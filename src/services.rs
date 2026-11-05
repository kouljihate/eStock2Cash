//! Business rules shared by the web handlers and the JSON API: stock posting,
//! invoice settlement, credit exposure and aging.

use futures_util::TryStreamExt;
use mongodb::Collection;
use mongodb::bson::{Document, doc};
use serde::de::DeserializeOwned;

use crate::error::AppError;
use crate::models::*;
use crate::state::DbHandle;
use crate::util::{now_iso, round2};

pub async fn get_by_id<T: DeserializeOwned + Send + Sync>(
    coll: &Collection<T>,
    company_id: &str,
    id: &str,
) -> Result<T, AppError> {
    coll.find_one(doc! { "_id": id, "company_id": company_id })
        .await?
        .ok_or_else(|| AppError::NotFound("Record not found".into()))
}

pub async fn list_recent<T: DeserializeOwned + Send + Sync>(
    coll: &Collection<T>,
    company_id: &str,
    limit: i64,
) -> Result<Vec<T>, AppError> {
    let mut cur = coll
        .find(doc! { "company_id": company_id })
        .sort(doc! { "created_at": -1 })
        .limit(limit)
        .await?;
    let mut out = Vec::new();
    while let Some(d) = cur.try_next().await? {
        out.push(d);
    }
    Ok(out)
}

pub async fn count(coll: &Collection<Document>, company_id: &str) -> Result<u64, AppError> {
    Ok(coll
        .count_documents(doc! { "company_id": company_id })
        .await?)
}

pub fn line_key(product_id: &str, description: &str) -> String {
    if product_id.is_empty() {
        format!("desc:{}", description.trim().to_lowercase())
    } else {
        format!("pid:{product_id}")
    }
}

/// Post a goods receipt: increase stock and record movements.
pub async fn post_goods_receipt(
    db: &DbHandle,
    gr: &mut GoodsReceipt,
    actor: &str,
) -> Result<(), AppError> {
    if gr.status == ReceiptStatus::Posted {
        return Err(AppError::BadRequest("This receipt is already posted.".into()));
    }
    let now = now_iso();
    for line in &gr.lines {
        if line.qty_received <= 0.0 || line.product_id.is_empty() {
            continue;
        }
        db.products()
            .update_one(
                doc! { "_id": &line.product_id, "company_id": &gr.company_id },
                doc! { "$inc": { "qty_on_hand": line.qty_received }, "$set": { "updated_at": &now } },
            )
            .await?;
        let mv = StockMove {
            id: crate::util::new_id(),
            company_id: gr.company_id.clone(),
            product_id: line.product_id.clone(),
            product_name: line.description.clone(),
            kind: MoveKind::In,
            qty: line.qty_received,
            unit_cost: line.unit_cost,
            source: "goods_receipt".into(),
            source_number: gr.number.clone(),
            note: String::new(),
            created_at: now.clone(),
            created_by: actor.to_string(),
        };
        db.stock_moves().insert_one(&mv).await?;
    }
    gr.status = ReceiptStatus::Posted;
    gr.posted_at = now.clone();
    db.goods_receipts()
        .update_one(
            doc! { "_id": &gr.id },
            doc! {
                "$set": {
                    "status": gr.status.as_str(),
                    "posted_at": &gr.posted_at,
                }
            },
        )
        .await?;
    recompute_po_status(db, &gr.company_id, &gr.po_id).await?;
    Ok(())
}

/// Recompute the received quantity of a PO from all its posted receipts.
pub async fn recompute_po_status(db: &DbHandle, company_id: &str, po_id: &str) -> Result<(), AppError> {
    if po_id.is_empty() {
        return Ok(());
    }
    let Ok(po) = get_by_id::<PurchaseOrder>(&db.purchase_orders(), company_id, po_id).await else {
        return Ok(());
    };
    if matches!(po.status, PoStatus::Cancelled) {
        return Ok(());
    }

    let mut cur = db
        .goods_receipts()
        .find(doc! {
            "company_id": company_id,
            "po_id": po_id,
            "status": ReceiptStatus::Posted.as_str(),
        })
        .await?;
    let mut received: std::collections::HashMap<String, f64> = std::collections::HashMap::new();
    let mut any = false;
    while let Some(gr) = cur.try_next().await? {
        for line in gr.lines {
            if line.qty_received > 0.0 {
                any = true;
                *received
                    .entry(line_key(&line.product_id, &line.description))
                    .or_insert(0.0) += line.qty_received;
            }
        }
    }

    let fully = po.lines.iter().all(|l| {
        let got = received
            .get(&line_key(&l.product_id, &l.description))
            .copied()
            .unwrap_or(0.0);
        got + 1e-9 >= l.qty
    });

    let status = if fully && !po.lines.is_empty() {
        PoStatus::Received
    } else if any {
        PoStatus::PartiallyReceived
    } else {
        return Ok(());
    };

    db.purchase_orders()
        .update_one(
            doc! { "_id": &po.id },
            doc! { "$set": { "status": status.as_str() } },
        )
        .await?;
    Ok(())
}

/// Post a delivery note: decrease stock, record movements, close the order.
pub async fn post_delivery_note(
    db: &DbHandle,
    dn: &mut DeliveryNote,
    actor: &str,
) -> Result<(), AppError> {
    if dn.status == DeliveryStatus::Posted {
        return Err(AppError::BadRequest("This delivery note is already posted.".into()));
    }
    let now = now_iso();
    for line in &dn.lines {
        if line.qty <= 0.0 {
            continue;
        }
        if !line.product_id.is_empty() {
            db.products()
                .update_one(
                    doc! { "_id": &line.product_id, "company_id": &dn.company_id },
                    doc! { "$inc": { "qty_on_hand": -line.qty }, "$set": { "updated_at": &now } },
                )
                .await?;
        }
        let mv = StockMove {
            id: crate::util::new_id(),
            company_id: dn.company_id.clone(),
            product_id: line.product_id.clone(),
            product_name: line.description.clone(),
            kind: MoveKind::Out,
            qty: line.qty,
            unit_cost: 0.0,
            source: "delivery_note".into(),
            source_number: dn.number.clone(),
            note: String::new(),
            created_at: now.clone(),
            created_by: actor.to_string(),
        };
        db.stock_moves().insert_one(&mv).await?;
    }
    dn.status = DeliveryStatus::Posted;
    dn.posted_at = now.clone();
    db.delivery_notes()
        .update_one(
            doc! { "_id": &dn.id },
            doc! { "$set": { "status": dn.status.as_str(), "posted_at": &dn.posted_at } },
        )
        .await?;

    if !dn.order_id.is_empty() {
        db.sales_orders()
            .update_one(
                doc! { "_id": &dn.order_id, "company_id": &dn.company_id },
                doc! { "$set": { "status": OrderStatus::Delivered.as_str(), "delivery_id": &dn.id } },
            )
            .await?;
    }
    Ok(())
}

/// Direct (POS) ticket: sell stock out of inventory immediately.
pub async fn post_ticket(db: &DbHandle, ticket: &Ticket) -> Result<(), AppError> {
    for item in &ticket.items {
        if item.product_id.is_empty() || item.qty <= 0.0 {
            continue;
        }
        db.products()
            .update_one(
                doc! { "_id": &item.product_id, "company_id": &ticket.company_id },
                doc! { "$inc": { "qty_on_hand": -item.qty }, "$set": { "updated_at": &ticket.created_at } },
            )
            .await?;
        let mv = StockMove {
            id: crate::util::new_id(),
            company_id: ticket.company_id.clone(),
            product_id: item.product_id.clone(),
            product_name: item.name.clone(),
            kind: MoveKind::Out,
            qty: item.qty,
            unit_cost: item.price,
            source: "ticket".into(),
            source_number: ticket.number.clone(),
            note: String::new(),
            created_at: ticket.created_at.clone(),
            created_by: ticket.created_by.clone(),
        };
        db.stock_moves().insert_one(&mv).await?;
    }
    Ok(())
}

/// Which collection holds this invoice kind.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InvoiceKind {
    Supplier,
    Customer,
}

impl InvoiceKind {
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "supplier" => Some(InvoiceKind::Supplier),
            "customer" => Some(InvoiceKind::Customer),
            _ => None,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            InvoiceKind::Supplier => "supplier",
            InvoiceKind::Customer => "customer",
        }
    }

    pub fn direction(&self) -> PayDirection {
        match self {
            InvoiceKind::Supplier => PayDirection::Out,
            InvoiceKind::Customer => PayDirection::In,
        }
    }
}

/// Recompute paid status from the paid amount.
pub fn settle_invoice(inv: &mut Invoice, paid: f64) {
    let paid = round2(paid.max(0.0).min(inv.totals.total));
    inv.paid_amount = paid;
    inv.status = if paid >= inv.totals.total - 0.005 {
        InvoiceStatus::Paid
    } else if paid > 0.005 {
        InvoiceStatus::PartiallyPaid
    } else if inv.status == InvoiceStatus::Draft {
        InvoiceStatus::Draft
    } else {
        InvoiceStatus::Open
    };
}

pub async fn persist_invoice(
    db: &DbHandle,
    kind: InvoiceKind,
    inv: &Invoice,
) -> Result<(), AppError> {
    let coll = match kind {
        InvoiceKind::Supplier => db.supplier_invoices(),
        InvoiceKind::Customer => db.customer_invoices(),
    };
    coll.update_one(
        doc! { "_id": &inv.id, "company_id": &inv.company_id },
        doc! {
            "$set": {
                "status": inv.status.as_str(),
                "paid_amount": inv.paid_amount,
                "totals": mongodb::bson::to_bson(&inv.totals)?,
            }
        },
    )
    .await?;
    Ok(())
}

pub async fn invoice_by_id(
    db: &DbHandle,
    kind: InvoiceKind,
    company_id: &str,
    id: &str,
) -> Result<Invoice, AppError> {
    match kind {
        InvoiceKind::Supplier => {
            get_by_id::<Invoice>(&db.supplier_invoices(), company_id, id).await
        }
        InvoiceKind::Customer => {
            get_by_id::<Invoice>(&db.customer_invoices(), company_id, id).await
        }
    }
}

/// Outstanding receivables for one customer across open invoices.
pub async fn customer_exposure(
    db: &DbHandle,
    company_id: &str,
    customer_id: &str,
) -> Result<f64, AppError> {
    let mut cur = db
        .customer_invoices()
        .find(doc! {
            "company_id": company_id,
            "party_id": customer_id,
        })
        .await?;
    let mut total = 0.0;
    while let Some(inv) = cur.try_next().await? {
        if inv.status.is_payable() {
            total += inv.outstanding();
        }
    }
    Ok(round2(total))
}

/// Credit position for a customer (limit 0 = unlimited).
pub struct CreditPosition {
    pub limit: f64,
    pub exposure: f64,
}

impl CreditPosition {
    pub fn available(&self) -> f64 {
        if self.limit <= 0.0 {
            f64::INFINITY
        } else {
            round2((self.limit - self.exposure).max(0.0))
        }
    }

    pub fn unlimited(&self) -> bool {
        self.limit <= 0.0
    }

    pub fn allows(&self, extra: f64) -> bool {
        self.unlimited() || self.exposure + extra <= self.limit + 0.005
    }
}

pub async fn credit_position(
    db: &DbHandle,
    company_id: &str,
    customer: &Customer,
) -> Result<CreditPosition, AppError> {
    let exposure = customer_exposure(db, company_id, &customer.id).await?;
    Ok(CreditPosition {
        limit: customer.credit_limit,
        exposure,
    })
}

#[derive(Debug, Default, Clone)]
pub struct Aging {
    pub current: f64,
    pub d30: f64,
    pub d60: f64,
    pub d90: f64,
    pub total: f64,
}

/// Aging of customer receivables by days past due.
pub async fn aging_report(db: &DbHandle, company_id: &str) -> Result<Aging, AppError> {
    let mut cur = db
        .customer_invoices()
        .find(doc! { "company_id": company_id })
        .await?;
    let mut aging = Aging::default();
    while let Some(inv) = cur.try_next().await? {
        if !inv.status.is_payable() {
            continue;
        }
        let out = inv.outstanding();
        if out <= 0.0 {
            continue;
        }
        let overdue = crate::util::days_until(&inv.due_date);
        if overdue >= 0 {
            aging.current += out;
        } else if overdue >= -30 {
            aging.d30 += out;
        } else if overdue >= -60 {
            aging.d60 += out;
        } else {
            aging.d90 += out;
        }
        aging.total += out;
    }
    aging.current = round2(aging.current);
    aging.d30 = round2(aging.d30);
    aging.d60 = round2(aging.d60);
    aging.d90 = round2(aging.d90);
    aging.total = round2(aging.total);
    Ok(aging)
}

pub async fn payables_total(db: &DbHandle, company_id: &str) -> Result<f64, AppError> {
    let mut cur = db
        .supplier_invoices()
        .find(doc! { "company_id": company_id })
        .await?;
    let mut total = 0.0;
    while let Some(inv) = cur.try_next().await? {
        if inv.status.is_payable() {
            total += inv.outstanding();
        }
    }
    Ok(round2(total))
}

pub struct DashboardStats {
    pub suppliers: u64,
    pub customers: u64,
    pub products: u64,
    pub low_stock: Vec<Product>,
    pub stock_value: f64,
    pub receivables: f64,
    pub payables: f64,
    pub open_pos: u64,
    pub open_quotes: u64,
    pub aging: Aging,
}

pub async fn dashboard_stats(db: &DbHandle, company_id: &str) -> Result<DashboardStats, AppError> {
    let suppliers = count(&db.db.collection(COLL_SUPPLIERS), company_id).await?;
    let customers = count(&db.db.collection(COLL_CUSTOMERS), company_id).await?;
    let products = count(&db.db.collection(COLL_PRODUCTS), company_id).await?;

    let mut cur = db.products().find(doc! { "company_id": company_id }).await?;
    let mut low_stock = Vec::new();
    let mut stock_value = 0.0;
    while let Some(p) = cur.try_next().await? {
        stock_value += p.qty_on_hand.max(0.0) * p.cost;
        if p.is_low() {
            low_stock.push(p);
        }
    }
    low_stock.sort_by(|a, b| a.qty_on_hand.partial_cmp(&b.qty_on_hand).unwrap_or(std::cmp::Ordering::Equal));
    low_stock.truncate(8);

    let aging = aging_report(db, company_id).await?;
    let payables = payables_total(db, company_id).await?;

    let open_pos = db
        .purchase_orders()
        .count_documents(doc! {
            "company_id": company_id,
            "status": { "$in": [PoStatus::Draft.as_str(), PoStatus::Sent.as_str(), PoStatus::PartiallyReceived.as_str()] }
        })
        .await?;

    let open_quotes = db
        .quotes()
        .count_documents(doc! {
            "company_id": company_id,
            "status": { "$in": [QuoteStatus::Draft.as_str(), QuoteStatus::Sent.as_str()] }
        })
        .await?;

    Ok(DashboardStats {
        suppliers,
        customers,
        products,
        low_stock,
        stock_value: round2(stock_value),
        receivables: aging.total,
        payables,
        open_pos,
        open_quotes,
        aging,
    })
}
