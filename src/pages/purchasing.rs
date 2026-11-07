//! Procure-to-Pay: purchase orders, goods receipts and supplier invoices.

use axum::extract::{Path, Query};
use axum::response::{IntoResponse, Response};
use futures_util::TryStreamExt;
use mongodb::bson::doc;
use std::collections::HashMap;

use crate::auth::Ctx;
use crate::error::AppError;
use crate::events::new_event;
use crate::form::{Pairs, parse_doc_lines, product_index};
use crate::i18n::tr;
use crate::models::*;
use crate::pdf::{PartyBlock, PdfLabels, invoice_pdf};
use crate::services;
use crate::util::{add_days, esc, now_iso, qty, round2, today};
use crate::views;

// ---------------------------------------------------------------------------
// Purchase orders
// ---------------------------------------------------------------------------

pub async fn po_list(ctx: Ctx, Query(q): Query<HashMap<String, String>>) -> Result<Response, AppError> {
    let search = q.get("q").cloned().unwrap_or_default();
    let mut cur = ctx
        .state
        .db
        .purchase_orders()
        .find(super::list_filter(&ctx.company.id, &search, &["number", "supplier_name"]))
        .sort(doc! { "created_at": -1 })
        .limit(300)
        .await?;
    let l = ctx.lang;
    let cur_code = ctx.company.currency.clone();
    let mut rows = Vec::new();
    while let Some(po) = cur.try_next().await? {
        rows.push(vec![
            format!(
                "<a href=\"/purchase-orders/{}\">{}</a>",
                esc(&po.id),
                esc(&po.number)
            ),
            esc(&po.supplier_name),
            esc(&po.order_date),
            views::status_badge(l, &po.status.key()),
            format!(
                "<span class=\"num\">{}</span>",
                esc(&views::money_text(po.totals.total, &cur_code))
            ),
            format!(
                "<div class=\"row-actions\">{}{}</div>",
                views::icon_link(&format!("/purchase-orders/{}", po.id), "eye", &tr(l, "a_view")),
                if matches!(po.status, PoStatus::Sent | PoStatus::PartiallyReceived) {
                    views::icon_link(
                        &format!("/goods-receipts/new?po={}", po.id),
                        "receipt",
                        &tr(l, "a_receive")
                    )
                } else {
                    String::new()
                }
            ),
        ]);
    }

    let body = format!(
        "{}{}",
        super::page_head(
            &ctx,
            "nav_purchase_orders",
            "",
            vec![
                super::search_form(l, "/purchase-orders", &search, ""),
                views::link_btn("/purchase-orders/new", &format!("+ {}", tr(l, "a_new"))),
            ]
        ),
        views::table_actions(
            l,
            &[
                &tr(l, "lbl_number"),
                &tr(l, "lbl_supplier"),
                &tr(l, "lbl_date"),
                &tr(l, "lbl_status"),
                &tr(l, "lbl_total"),
                &tr(l, "lbl_actions"),
            ],
            rows,
            "msg_no_records",
        )
    );
    let title = views::Title::of(l, "nav_purchase_orders");
    Ok(views::respond(views::page(&ctx, &title, body)))
}

async fn supplier_options(ctx: &Ctx, selected: &str) -> Result<Vec<(String, String, bool)>, AppError> {
    let mut cur = ctx
        .state
        .db
        .suppliers()
        .find(doc! { "company_id": &ctx.company.id, "active": true })
        .sort(doc! { "name": 1 })
        .await?;
    let mut out = Vec::new();
    while let Some(s) = cur.try_next().await? {
        out.push((s.id.clone(), s.name.clone(), s.id == selected));
    }
    Ok(out)
}

pub async fn po_new(ctx: Ctx, Query(q): Query<HashMap<String, String>>) -> Result<Response, AppError> {
    let l = ctx.lang;
    let supplier_id = q.get("supplier").cloned().unwrap_or_default();
    let products = product_index(&ctx.state.db, &ctx.company.id).await?;
    let list: Vec<Product> = products.values().cloned().collect();
    let mut list = list;
    list.sort_by(|a, b| a.name.cmp(&b.name));
    let suppliers = supplier_options(&ctx, &supplier_id).await?;

    if suppliers.is_empty() {
        return Ok(views::respond(views::page(&ctx, &views::Title::of(l, "nav_purchase_orders"),
            format!(
                "{}{}",
                super::page_head(&ctx, "nav_purchase_orders", "", vec![]),
                views::notice(l, "msg_no_records", "warn")
            ),
        )));
    }

    let body = format!(
        "{}<form method=\"post\" action=\"/purchase-orders\" class=\"card form\">\
            <div class=\"form-grid\">{}{}{}{}</div>{}{}</form>",
        super::page_head(&ctx, "nav_purchase_orders", "", vec![]),
        views::select(&tr(l, "lbl_supplier"), "supplier_id", &suppliers, true),
        views::input(&tr(l, "lbl_order_date"), "order_date", &today(), "date", true),
        views::input(&tr(l, "lbl_expected_date"), "expected_date", &add_days(&today(), 7), "date", false),
        views::input_step(&tr(l, "lbl_tax_rate"), "default_tax", "0", "0.01", false),
        format!(
            "<h3 class=\"section-title\">{}</h3>{}",
            tr(l, "lbl_lines"),
            super::lines_editor(&ctx, &list, &[], 0.0, true)
        ),
        format!(
            "<div class=\"form-grid\">{}</div>{}",
            views::textarea(&tr(l, "lbl_notes"), "notes", ""),
            views::form_actions(&tr(l, "a_save"), "/purchase-orders", &tr(l, "a_back"))
        )
    );
    let title = views::Title::of(l, "nav_purchase_orders").join(l, "a_new");
    Ok(views::respond(views::page(&ctx, &title, body)))
}

pub async fn po_create(ctx: Ctx, pairs: Pairs) -> Result<Response, AppError> {
    let supplier_id = pairs.get("supplier_id");
    let supplier = services::get_by_id::<Supplier>(&ctx.state.db.suppliers(), &ctx.company.id, &supplier_id)
        .await
        .map_err(|_| AppError::BadRequest("Select a supplier".into()))?;
    let products = product_index(&ctx.state.db, &ctx.company.id).await?;
    let default_tax = pairs.num("default_tax", 0.0);
    let lines = parse_doc_lines(&pairs, &products, true, default_tax);
    if lines.is_empty() {
        return Ok(ctx.back("/purchase-orders/new", "msg_line_required", "err").into_response());
    }
    let totals = Totals::compute(&lines);
    let number = crate::db::next_number(&ctx.state.db.db, &ctx.company.id, "PO").await?;
    let po = PurchaseOrder {
        id: crate::util::new_id(),
        company_id: ctx.company.id.clone(),
        number: number.clone(),
        supplier_id: supplier.id.clone(),
        supplier_name: supplier.name.clone(),
        order_date: crate::util::if_empty(&pairs.get("order_date"), &today()),
        expected_date: pairs.get("expected_date"),
        status: PoStatus::Draft,
        lines,
        totals,
        notes: pairs.get("notes"),
        created_at: now_iso(),
        created_by: ctx.claims.un.clone(),
    };
    ctx.state.db.purchase_orders().insert_one(&po).await?;
    ctx.state
        .bus
        .publish(
            "po.created",
            &new_event(
                "purchase_order.created",
                &po.company_id,
                &po.id,
                &po.number,
                po.status.as_str(),
                po.totals.total,
                &ctx.claims.un,
            ),
        )
        .await;
    Ok(axum::response::Redirect::to(&format!("/purchase-orders/{}?m=msg_saved&k=ok", po.id))
        .into_response())
}

pub async fn po_detail(ctx: Ctx, Path(id): Path<String>) -> Result<Response, AppError> {
    let l = ctx.lang;
    let cur_code = ctx.company.currency.clone();
    let po = services::get_by_id::<PurchaseOrder>(&ctx.state.db.purchase_orders(), &ctx.company.id, &id).await?;

    let mut cur = ctx
        .state
        .db
        .goods_receipts()
        .find(doc! { "company_id": &ctx.company.id, "po_id": &po.id })
        .sort(doc! { "created_at": -1 })
        .await?;
    let mut grs = Vec::new();
    while let Some(g) = cur.try_next().await? {
        grs.push(g);
    }
    let mut inv_cur = ctx
        .state
        .db
        .supplier_invoices()
        .find(doc! { "company_id": &ctx.company.id, "source_id": &po.id })
        .await?;
    let mut invoices = Vec::new();
    while let Some(i) = inv_cur.try_next().await? {
        invoices.push(i);
    }

    let mut actions = Vec::new();
    match po.status {
        PoStatus::Draft => {
            actions.push(views::post_btn(
                &format!("/purchase-orders/{}/action", po.id),
                &tr(l, "a_send"),
                "btn-primary",
            ));
        }
        PoStatus::Sent | PoStatus::PartiallyReceived => {
            actions.push(views::link_btn(
                &format!("/goods-receipts/new?po={}", po.id),
                &tr(l, "a_receive"),
            ));
            actions.push(views::link_btn(
                &format!("/supplier-invoices/new?po={}", po.id),
                &tr(l, "a_invoice"),
            ));
        }
        _ => {}
    }
    if !matches!(po.status, PoStatus::Received | PoStatus::Cancelled) {
        actions.push(views::post_btn(
            &format!("/purchase-orders/{}/cancel", po.id),
            &tr(l, "a_cancel"),
            "btn-danger",
        ));
    }

    let gr_rows = grs
        .iter()
        .map(|g| {
            vec![
                format!("<a href=\"/goods-receipts/{}\">{}</a>", esc(&g.id), esc(&g.number)),
                esc(&g.receipt_date),
                views::status_badge(l, &g.status.key()),
            ]
        })
        .collect::<Vec<_>>();
    let inv_rows = invoices
        .iter()
        .map(|i| {
            vec![
                format!(
                    "<a href=\"/supplier-invoices/{}\">{}</a>",
                    esc(&i.id),
                    esc(&i.number)
                ),
                views::status_badge(l, &i.display_status().key()),
                format!("<span class=\"num\">{}</span>", esc(&views::money_text(i.outstanding(), &cur_code))),
            ]
        })
        .collect::<Vec<_>>();

    let meta = super::meta_grid(vec![
        (tr(l, "lbl_supplier"), esc(&po.supplier_name)),
        (tr(l, "lbl_order_date"), esc(&po.order_date)),
        (tr(l, "lbl_expected_date"), esc(&po.expected_date)),
        (tr(l, "lbl_status"), views::status_badge(l, &po.status.key())),
        (tr(l, "lbl_created_by"), esc(&po.created_by)),
        (tr(l, "lbl_created_at"), esc(&po.created_at)),
    ]);

    let body = format!(
        "{}{}{}{}{}{}",
        super::page_head(&ctx, "nav_purchase_orders", &po.number, actions),
        views::card(&tr(l, "nav_purchase_orders"), meta),
        views::card(&tr(l, "lbl_lines"), super::lines_view(&ctx, &po.lines, &cur_code)),
        views::card(
            &tr(l, "m_totals"),
            super::totals_block(&ctx, &po.totals, None, &cur_code)
        ),
        views::card(
            &tr(l, "nav_goods_receipts"),
            views::table(
                l,
                &[&tr(l, "lbl_number"), &tr(l, "lbl_date"), &tr(l, "lbl_status")],
                gr_rows,
                "msg_no_records"
            )
        ),
        views::card(
            &tr(l, "nav_supplier_invoices"),
            views::table(
                l,
                &[&tr(l, "lbl_number"), &tr(l, "lbl_status"), &tr(l, "lbl_outstanding")],
                inv_rows,
                "msg_no_records"
            )
        )
    );
    let title = views::Title::raw(&po.number);
    Ok(views::respond(views::page(&ctx, &title, body)))
}

pub async fn po_send(ctx: Ctx, Path(id): Path<String>) -> Result<Response, AppError> {
    let mut po = services::get_by_id::<PurchaseOrder>(&ctx.state.db.purchase_orders(), &ctx.company.id, &id).await?;
    if po.status != PoStatus::Draft {
        return Ok(ctx.back(&format!("/purchase-orders/{id}"), "msg_cannot_post_twice", "err").into_response());
    }
    po.status = PoStatus::Sent;
    ctx.state
        .db
        .purchase_orders()
        .update_one(
            doc! { "_id": &po.id, "company_id": &ctx.company.id },
            doc! { "$set": { "status": po.status.as_str() } },
        )
        .await?;
    ctx.state
        .bus
        .publish(
            "po.sent",
            &new_event(
                "purchase_order.sent",
                &po.company_id,
                &po.id,
                &po.number,
                po.status.as_str(),
                po.totals.total,
                &ctx.claims.un,
            ),
        )
        .await;
    Ok(ctx.back(&format!("/purchase-orders/{id}"), "msg_sent", "ok").into_response())
}

pub async fn po_cancel(ctx: Ctx, Path(id): Path<String>) -> Result<Response, AppError> {
    let po = services::get_by_id::<PurchaseOrder>(&ctx.state.db.purchase_orders(), &ctx.company.id, &id).await?;
    ctx.state
        .db
        .purchase_orders()
        .update_one(
            doc! { "_id": &po.id, "company_id": &ctx.company.id },
            doc! { "$set": { "status": PoStatus::Cancelled.as_str() } },
        )
        .await?;
    ctx.state
        .bus
        .publish(
            "po.cancelled",
            &new_event(
                "purchase_order.cancelled",
                &po.company_id,
                &po.id,
                &po.number,
                PoStatus::Cancelled.as_str(),
                po.totals.total,
                &ctx.claims.un,
            ),
        )
        .await;
    Ok(ctx.back("/purchase-orders", "msg_saved", "ok").into_response())
}

// ---------------------------------------------------------------------------
// Goods receipts
// ---------------------------------------------------------------------------

pub async fn gr_list(ctx: Ctx, Query(q): Query<HashMap<String, String>>) -> Result<Response, AppError> {
    let search = q.get("q").cloned().unwrap_or_default();
    let mut cur = ctx
        .state
        .db
        .goods_receipts()
        .find(super::list_filter(&ctx.company.id, &search, &["number", "po_number", "supplier_name"]))
        .sort(doc! { "created_at": -1 })
        .limit(300)
        .await?;
    let l = ctx.lang;
    let mut rows = Vec::new();
    while let Some(g) = cur.try_next().await? {
        rows.push(vec![
            format!(
                "<a href=\"/goods-receipts/{}\">{}</a>",
                esc(&g.id),
                esc(&g.number)
            ),
            esc(&g.po_number),
            esc(&g.supplier_name),
            esc(&g.receipt_date),
            views::status_badge(l, &g.status.key()),
        ]);
    }
    let body = format!(
        "{}{}",
        super::page_head(
            &ctx,
            "nav_goods_receipts",
            "",
            vec![
                super::search_form(l, "/goods-receipts", &search, ""),
                views::link_btn("/goods-receipts/new", &format!("+ {}", tr(l, "a_new"))),
            ]
        ),
        views::table(
            l,
            &[
                &tr(l, "lbl_number"),
                &tr(l, "nav_purchase_orders"),
                &tr(l, "lbl_supplier"),
                &tr(l, "lbl_date"),
                &tr(l, "lbl_status"),
            ],
            rows,
            "msg_no_records",
        )
    );
    let title = views::Title::of(l, "nav_goods_receipts");
    Ok(views::respond(views::page(&ctx, &title, body)))
}

pub async fn gr_new(ctx: Ctx, Query(q): Query<HashMap<String, String>>) -> Result<Response, AppError> {
    let l = ctx.lang;
    let po_id = q.get("po").cloned().unwrap_or_default();
    let products = product_index(&ctx.state.db, &ctx.company.id).await?;

    // Prefill from the purchase order with the still-open quantities.
    let (mut po_number, mut supplier_id, mut supplier_name) =
        (String::new(), String::new(), String::new());
    let mut rows = String::new();
    let mut tax = 0.0;

    if !po_id.is_empty() {
        let po = services::get_by_id::<PurchaseOrder>(&ctx.state.db.purchase_orders(), &ctx.company.id, &po_id).await?;
        po_number = po.number.clone();
        supplier_id = po.supplier_id.clone();
        supplier_name = po.supplier_name.clone();

        let mut received: HashMap<String, f64> = HashMap::new();
        let mut cur = ctx
            .state
            .db
            .goods_receipts()
            .find(doc! { "company_id": &ctx.company.id, "po_id": &po.id, "status": ReceiptStatus::Posted.as_str() })
            .await?;
        while let Some(g) = cur.try_next().await? {
            for line in g.lines {
                *received
                    .entry(services::line_key(&line.product_id, &line.description))
                    .or_insert(0.0) += line.qty_received;
            }
        }

        for line in &po.lines {
            let got = received
                .get(&services::line_key(&line.product_id, &line.description))
                .copied()
                .unwrap_or(0.0);
            let remaining = round2((line.qty - got).max(0.0));
            let opts = product_select(&products, &line.product_id);
            rows.push_str(&format!(
                "<tr class=\"line-row\"><td><select name=\"item_product\">{opts}</select></td>\
                 <td><input name=\"item_desc\" value=\"{desc}\"/><input type=\"hidden\" name=\"item_ordered\" value=\"{ordered}\"/></td>\
                 <td><input name=\"item_qty\" class=\"num\" type=\"number\" step=\"0.01\" value=\"{q}\"/></td>\
                 <td><input name=\"item_cost\" class=\"num\" type=\"number\" step=\"0.01\" value=\"{cost}\"/></td>\
                 <td><button type=\"button\" class=\"btn-icon remove-line\">×</button></td></tr>",
                desc = esc(&line.description),
                ordered = qty(line.qty),
                q = qty(remaining),
                cost = format!("{:.2}", line.unit_price),
            ));
        }
        tax = 0.0;
    } else {
        rows.push_str(&format!(
            "<tr class=\"line-row\"><td><select name=\"item_product\">{}</select></td>\
             <td><input name=\"item_desc\" value=\"\"/><input type=\"hidden\" name=\"item_ordered\" value=\"0\"/></td>\
             <td><input name=\"item_qty\" class=\"num\" type=\"number\" step=\"0.01\" value=\"1\"/></td>\
             <td><input name=\"item_cost\" class=\"num\" type=\"number\" step=\"0.01\" value=\"0\"/></td>\
             <td><button type=\"button\" class=\"btn-icon remove-line\">×</button></td></tr>",
            product_select(&products, "")
        ));
    }

    let template = format!(
        "<tr class=\"line-row\"><td><select name=\"item_product\">{}</select></td>\
         <td><input name=\"item_desc\" value=\"\"/><input type=\"hidden\" name=\"item_ordered\" value=\"0\"/></td>\
         <td><input name=\"item_qty\" class=\"num\" type=\"number\" step=\"0.01\" value=\"1\"/></td>\
         <td><input name=\"item_cost\" class=\"num\" type=\"number\" step=\"0.01\" value=\"0\"/></td>\
         <td><button type=\"button\" class=\"btn-icon remove-line\">×</button></td></tr>",
        product_select(&products, "")
    );

    let po_options = {
        let mut cur = ctx
            .state
            .db
            .purchase_orders()
            .find(doc! { "company_id": &ctx.company.id, "status": { "$in": [PoStatus::Sent.as_str(), PoStatus::PartiallyReceived.as_str()] } })
            .sort(doc! { "created_at": -1 })
            .await?;
        let mut out = vec![(String::new(), tr(l, "lbl_none"), po_id.is_empty())];
        while let Some(p) = cur.try_next().await? {
            out.push((p.id.clone(), format!("{} · {}", p.number, p.supplier_name), p.id == po_id));
        }
        out
    };

    let body = format!(
        "{}<form method=\"post\" action=\"/goods-receipts\" class=\"card form\">\
           <div class=\"form-grid\">{}{}{}{}</div>\
           <div class=\"lines-wrap\"><table class=\"lines-table\"><thead><tr>\
             <th>{}</th><th>{}</th><th class=\"num\">{}</th><th class=\"num\">{}</th><th></th>\
           </tr></thead><tbody id=\"lines-body\">{rows}</tbody></table>\
           <template id=\"line-tpl\">{template}</template>\
           <div class=\"lines-foot\"><button type=\"button\" class=\"btn btn-sm\" id=\"add-line\">+ {}</button></div></div>\
           {}{}</form>",
        super::page_head(&ctx, "nav_goods_receipts", &po_number, vec![]),
        views::select(&tr(l, "nav_purchase_orders"), "po_id", &po_options, false),
        views::input(&tr(l, "lbl_receipt_date"), "receipt_date", &today(), "date", true),
        views::input(&tr(l, "lbl_supplier"), "supplier_name", &supplier_name, "text", false),
        views::input_step(&tr(l, "lbl_tax_rate"), "default_tax", &qty(tax), "0.01", false),
        tr(l, "lbl_product"),
        tr(l, "lbl_description"),
        tr(l, "lbl_qty_received"),
        tr(l, "lbl_cost"),
        tr(l, "a_add_line"),
        views::textarea(&tr(l, "lbl_notes"), "notes", ""),
        views::form_actions(&tr(l, "a_save"), "/goods-receipts", &tr(l, "a_back")),
    );
    let _ = supplier_id;
    let title = views::Title::of(l, "nav_goods_receipts").join(l, "a_new");
    Ok(views::respond(views::page(&ctx, &title, body)))
}

fn product_select(products: &HashMap<String, Product>, selected: &str) -> String {
    let mut out = String::from("<option value=\"\">—</option>");
    for p in products.values() {
        out.push_str(&format!(
            "<option value=\"{}\"{}>{}</option>",
            esc(&p.id),
            if p.id == selected { " selected" } else { "" },
            esc(&format!("{} ({})", p.name, p.sku))
        ));
    }
    out
}

pub async fn gr_create(ctx: Ctx, pairs: Pairs) -> Result<Response, AppError> {
    let po_id = pairs.get("po_id");
    let (po_number, supplier_id, supplier_name) = if po_id.is_empty() {
        (String::new(), String::new(), pairs.get("supplier_name"))
    } else {
        let po = services::get_by_id::<PurchaseOrder>(&ctx.state.db.purchase_orders(), &ctx.company.id, &po_id).await?;
        (po.number.clone(), po.supplier_id.clone(), po.supplier_name.clone())
    };

    let products = product_index(&ctx.state.db, &ctx.company.id).await?;
    let ids = pairs.all("item_product");
    let descs = pairs.all("item_desc");
    let qtys = pairs.all("item_qty");
    let ordered = pairs.all("item_ordered");
    let costs = pairs.all("item_cost");

    let mut lines = Vec::new();
    for i in 0..ids.len().max(descs.len()) {
        let product_id = ids.get(i).cloned().unwrap_or_default();
        let mut description = descs.get(i).cloned().unwrap_or_default();
        let received = qtys
            .get(i)
            .cloned()
            .unwrap_or_default()
            .replace(',', ".")
            .parse::<f64>()
            .unwrap_or(0.0);
        let qty_ordered = ordered
            .get(i)
            .cloned()
            .unwrap_or_default()
            .parse::<f64>()
            .unwrap_or(0.0);
        let unit_cost = costs
            .get(i)
            .cloned()
            .unwrap_or_default()
            .replace(',', ".")
            .parse::<f64>()
            .unwrap_or(0.0);
        if description.trim().is_empty() && product_id.is_empty() {
            continue;
        }
        if description.trim().is_empty() {
            if let Some(p) = products.get(&product_id) {
                description = p.name.clone();
            }
        }
        if received <= 0.0 {
            continue;
        }
        lines.push(ReceiptLine {
            product_id,
            description,
            qty_ordered,
            qty_received: received,
            unit_cost,
        });
    }
    if lines.is_empty() {
        return Ok(ctx.back("/goods-receipts/new", "msg_line_required", "err").into_response());
    }

    let number = crate::db::next_number(&ctx.state.db.db, &ctx.company.id, "GR").await?;
    let mut gr = GoodsReceipt {
        id: crate::util::new_id(),
        company_id: ctx.company.id.clone(),
        number,
        po_id,
        po_number,
        supplier_id,
        supplier_name,
        receipt_date: crate::util::if_empty(&pairs.get("receipt_date"), &today()),
        status: ReceiptStatus::Draft,
        lines,
        notes: pairs.get("notes"),
        created_at: now_iso(),
        created_by: ctx.claims.un.clone(),
        posted_at: String::new(),
    };
    ctx.state.db.goods_receipts().insert_one(&gr).await?;

    // Receiving immediately moves the stock; the detail page shows the outcome.
    services::post_goods_receipt(&ctx.state.db, &mut gr, &ctx.claims.un).await?;
    ctx.state
        .bus
        .publish(
            "gr.posted",
            &new_event(
                "goods_receipt.posted",
                &gr.company_id,
                &gr.id,
                &gr.number,
                gr.status.as_str(),
                0.0,
                &ctx.claims.un,
            ),
        )
        .await;

    Ok(axum::response::Redirect::to(&format!("/goods-receipts/{}?m=msg_posted&k=ok", gr.id))
        .into_response())
}

pub async fn gr_detail(ctx: Ctx, Path(id): Path<String>) -> Result<Response, AppError> {
    let l = ctx.lang;
    let gr = services::get_by_id::<GoodsReceipt>(&ctx.state.db.goods_receipts(), &ctx.company.id, &id).await?;
    let cur_code = ctx.company.currency.clone();

    let rows = gr
        .lines
        .iter()
        .map(|line| {
            vec![
                esc(&line.description),
                format!("<span class=\"num\">{}</span>", qty(line.qty_ordered)),
                format!("<span class=\"num\">{}</span>", qty(line.qty_received)),
                format!("<span class=\"num\">{}</span>", esc(&views::money_text(line.unit_cost, &cur_code))),
            ]
        })
        .collect::<Vec<_>>();

    let mut actions = Vec::new();
    if gr.status == ReceiptStatus::Draft {
        actions.push(views::post_btn(
            &format!("/goods-receipts/{}/post", gr.id),
            &tr(l, "a_post"),
            "btn-primary",
        ));
    }
    if !gr.po_id.is_empty() {
        actions.push(views::link_btn(
            &format!("/purchase-orders/{}", gr.po_id),
            &tr(l, "nav_purchase_orders"),
        ));
        actions.push(views::link_btn(
            &format!("/supplier-invoices/new?gr={}", gr.id),
            &tr(l, "a_invoice"),
        ));
    }

    let meta = super::meta_grid(vec![
        (tr(l, "nav_purchase_orders"), esc(&gr.po_number)),
        (tr(l, "lbl_supplier"), esc(&gr.supplier_name)),
        (tr(l, "lbl_receipt_date"), esc(&gr.receipt_date)),
        (tr(l, "lbl_status"), views::status_badge(l, &gr.status.key())),
        (tr(l, "lbl_created_by"), esc(&gr.created_by)),
    ]);

    let body = format!(
        "{}{}{}{}",
        super::page_head(&ctx, "nav_goods_receipts", &gr.number, actions),
        views::card(&tr(l, "nav_goods_receipts"), meta),
        views::card(
            &tr(l, "lbl_lines"),
            views::table(
                l,
                &[
                    &tr(l, "lbl_description"),
                    &tr(l, "lbl_qty_ordered"),
                    &tr(l, "lbl_qty_received"),
                    &tr(l, "lbl_cost"),
                ],
                rows,
                "msg_no_records"
            )
        ),
        views::card(&tr(l, "lbl_notes"), format!("<p>{}</p>", esc(&gr.notes)))
    );
    let title = views::Title::raw(&gr.number);
    Ok(views::respond(views::page(&ctx, &title, body)))
}

pub async fn gr_post(ctx: Ctx, Path(id): Path<String>) -> Result<Response, AppError> {
    let mut gr = services::get_by_id::<GoodsReceipt>(&ctx.state.db.goods_receipts(), &ctx.company.id, &id).await?;
    if gr.status == ReceiptStatus::Posted {
        return Ok(ctx.back(&format!("/goods-receipts/{id}"), "msg_cannot_post_twice", "err").into_response());
    }
    services::post_goods_receipt(&ctx.state.db, &mut gr, &ctx.claims.un).await?;
    ctx.state
        .bus
        .publish(
            "gr.posted",
            &new_event(
                "goods_receipt.posted",
                &gr.company_id,
                &gr.id,
                &gr.number,
                gr.status.as_str(),
                0.0,
                &ctx.claims.un,
            ),
        )
        .await;
    Ok(ctx.back(&format!("/goods-receipts/{id}"), "msg_posted", "ok").into_response())
}

// ---------------------------------------------------------------------------
// Supplier invoices
// ---------------------------------------------------------------------------

pub async fn si_list(ctx: Ctx, Query(q): Query<HashMap<String, String>>) -> Result<Response, AppError> {
    let search = q.get("q").cloned().unwrap_or_default();
    let mut cur = ctx
        .state
        .db
        .supplier_invoices()
        .find(super::list_filter(&ctx.company.id, &search, &["number", "party_name"]))
        .sort(doc! { "created_at": -1 })
        .limit(300)
        .await?;
    let l = ctx.lang;
    let cur_code = ctx.company.currency.clone();
    let mut rows = Vec::new();
    while let Some(inv) = cur.try_next().await? {
        rows.push(vec![
            format!(
                "<a href=\"/supplier-invoices/{}\">{}</a>",
                esc(&inv.id),
                esc(&inv.number)
            ),
            esc(&inv.party_name),
            esc(&inv.invoice_date),
            esc(&inv.due_date),
            views::status_badge(l, &inv.display_status().key()),
            format!(
                "<span class=\"num\">{}</span>",
                esc(&views::money_text(inv.outstanding(), &cur_code))
            ),
            format!(
                "<div class=\"row-actions\">{}{}</div>",
                views::icon_link(&format!("/supplier-invoices/{}/pdf", inv.id), "file", &tr(l, "a_pdf")),
                if inv.display_status().is_payable() {
                    views::icon_link(&format!("/payments/new?invoice={}&kind=supplier", inv.id), "wallet", &tr(l, "a_pay"))
                } else {
                    String::new()
                }
            ),
        ]);
    }
    let body = format!(
        "{}{}",
        super::page_head(
            &ctx,
            "nav_supplier_invoices",
            "",
            vec![
                super::search_form(l, "/supplier-invoices", &search, ""),
                views::link_btn("/supplier-invoices/new", &format!("+ {}", tr(l, "a_new"))),
            ]
        ),
        views::table_actions(
            l,
            &[
                &tr(l, "lbl_number"),
                &tr(l, "lbl_supplier"),
                &tr(l, "lbl_date"),
                &tr(l, "lbl_due_date"),
                &tr(l, "lbl_status"),
                &tr(l, "lbl_outstanding"),
                &tr(l, "lbl_actions"),
            ],
            rows,
            "msg_no_records",
        )
    );
    let title = views::Title::of(l, "nav_supplier_invoices");
    Ok(views::respond(views::page(&ctx, &title, body)))
}

pub async fn si_new(ctx: Ctx, Query(q): Query<HashMap<String, String>>) -> Result<Response, AppError> {
    let l = ctx.lang;
    let po_id = q.get("po").cloned().unwrap_or_default();
    let gr_id = q.get("gr").cloned().unwrap_or_default();
    let products = product_index(&ctx.state.db, &ctx.company.id).await?;
    let mut list: Vec<Product> = products.values().cloned().collect();
    list.sort_by(|a, b| a.name.cmp(&b.name));

    let mut lines: Vec<DocLine> = Vec::new();
    let mut supplier_id = String::new();
    let mut source_id = String::new();
    let mut source_number = String::new();
    let due = add_days(&today(), 30);

    if !po_id.is_empty() {
        let po = services::get_by_id::<PurchaseOrder>(&ctx.state.db.purchase_orders(), &ctx.company.id, &po_id).await?;
        supplier_id = po.supplier_id.clone();
        source_id = po.id.clone();
        source_number = po.number.clone();
        lines = po.lines.clone();
    } else if !gr_id.is_empty() {
        let gr = services::get_by_id::<GoodsReceipt>(&ctx.state.db.goods_receipts(), &ctx.company.id, &gr_id).await?;
        supplier_id = gr.supplier_id.clone();
        source_id = gr.id.clone();
        source_number = gr.number.clone();
        lines = gr
            .lines
            .iter()
            .map(|line| DocLine {
                product_id: line.product_id.clone(),
                description: line.description.clone(),
                qty: line.qty_received,
                unit_price: line.unit_cost,
                tax_rate: 0.0,
            })
            .collect();
    }

    let suppliers = supplier_options(&ctx, &supplier_id).await?;

    let body = format!(
        "{}<form method=\"post\" action=\"/supplier-invoices\" class=\"card form\">\
           <div class=\"form-grid\">{}{}{}{}{}{}</div>\
           <h3 class=\"section-title\">{}</h3>{}\
           {}{}</form>",
        super::page_head(&ctx, "nav_supplier_invoices", &source_number, vec![]),
        views::select(&tr(l, "lbl_supplier"), "supplier_id", &suppliers, true),
        views::input(&tr(l, "lbl_number"), "party_invoice_no", "", "text", false),
        views::input(&tr(l, "lbl_invoice_date"), "invoice_date", &today(), "date", true),
        views::input(&tr(l, "lbl_due_date"), "due_date", &due, "date", true),
        views::input_step(&tr(l, "lbl_tax_rate"), "default_tax", "0", "0.01", false),
        format!("<input type=\"hidden\" name=\"source_id\" value=\"{}\"/>", esc(&source_id)),
        tr(l, "lbl_lines"),
        super::lines_editor(&ctx, &list, &lines, 0.0, true),
        views::textarea(&tr(l, "lbl_notes"), "notes", ""),
        views::form_actions(&tr(l, "a_save"), "/supplier-invoices", &tr(l, "a_back")),
    );
    let title = views::Title::of(l, "nav_supplier_invoices").join(l, "a_new");
    Ok(views::respond(views::page(&ctx, &title, body)))
}

pub async fn si_create(ctx: Ctx, pairs: Pairs) -> Result<Response, AppError> {
    let supplier_id = pairs.get("supplier_id");
    let supplier = services::get_by_id::<Supplier>(&ctx.state.db.suppliers(), &ctx.company.id, &supplier_id)
        .await
        .map_err(|_| AppError::BadRequest("Select a supplier".into()))?;
    let products = product_index(&ctx.state.db, &ctx.company.id).await?;
    let lines = parse_doc_lines(&pairs, &products, true, pairs.num("default_tax", 0.0));
    if lines.is_empty() {
        return Ok(ctx.back("/supplier-invoices/new", "msg_line_required", "err").into_response());
    }
    let totals = Totals::compute(&lines);
    let number = crate::db::next_number(&ctx.state.db.db, &ctx.company.id, "SI").await?;
    let source_id = pairs.get("source_id");
    let source_number = if source_id.is_empty() {
        String::new()
    } else if let Some(po) = ctx
        .state
        .db
        .purchase_orders()
        .find_one(doc! { "_id": &source_id, "company_id": &ctx.company.id })
        .await?
    {
        po.number
    } else {
        ctx.state
            .db
            .goods_receipts()
            .find_one(doc! { "_id": &source_id, "company_id": &ctx.company.id })
            .await?
            .map(|g| g.number)
            .unwrap_or_default()
    };
    let inv = Invoice {
        id: crate::util::new_id(),
        company_id: ctx.company.id.clone(),
        number: number.clone(),
        party_id: supplier.id.clone(),
        party_name: supplier.name.clone(),
        source_id,
        source_number,
        invoice_date: crate::util::if_empty(&pairs.get("invoice_date"), &today()),
        due_date: crate::util::if_empty(&pairs.get("due_date"), &add_days(&today(), 30)),
        lines,
        totals,
        status: InvoiceStatus::Draft,
        paid_amount: 0.0,
        notes: pairs.get("notes"),
        created_at: now_iso(),
        created_by: ctx.claims.un.clone(),
    };
    ctx.state.db.supplier_invoices().insert_one(&inv).await?;
    ctx.state
        .bus
        .publish(
            "supplier_invoice.created",
            &new_event(
                "supplier_invoice.created",
                &inv.company_id,
                &inv.id,
                &inv.number,
                inv.status.as_str(),
                inv.totals.total,
                &ctx.claims.un,
            ),
        )
        .await;
    Ok(axum::response::Redirect::to(&format!("/supplier-invoices/{}?m=msg_saved&k=ok", inv.id))
        .into_response())
}

pub async fn si_detail(ctx: Ctx, Path(id): Path<String>) -> Result<Response, AppError> {
    let l = ctx.lang;
    let inv = services::get_by_id::<Invoice>(&ctx.state.db.supplier_invoices(), &ctx.company.id, &id).await?;
    let cur_code = ctx.company.currency.clone();
    let mut actions = vec![views::link_btn(
        &format!("/supplier-invoices/{}/pdf", inv.id),
        &tr(l, "a_pdf"),
    )];
    if inv.status == InvoiceStatus::Draft {
        actions.push(views::post_btn(
            &format!("/supplier-invoices/{}/issue", inv.id),
            &tr(l, "a_issue"),
            "btn-primary",
        ));
    }
    if inv.display_status().is_payable() {
        actions.push(views::link_btn(
            &format!("/payments/new?invoice={}&kind=supplier", inv.id),
            &tr(l, "a_pay"),
        ));
    }
    if !inv.status.is_settled() {
        actions.push(views::post_btn(
            &format!("/supplier-invoices/{}/cancel", inv.id),
            &tr(l, "a_cancel"),
            "btn-danger",
        ));
    }

    let meta = super::meta_grid(vec![
        (tr(l, "lbl_supplier"), esc(&inv.party_name)),
        (tr(l, "lbl_number"), esc(&inv.number)),
        (tr(l, "lbl_invoice_date"), esc(&inv.invoice_date)),
        (tr(l, "lbl_due_date"), esc(&inv.due_date)),
        (tr(l, "lbl_status"), views::status_badge(l, &inv.display_status().key())),
        (tr(l, "lbl_source"), esc(&inv.source_number)),
    ]);

    let body = format!(
        "{}{}{}{}",
        super::page_head(&ctx, "nav_supplier_invoices", &inv.number, actions),
        views::card(&tr(l, "nav_supplier_invoices"), meta),
        views::card(&tr(l, "lbl_lines"), super::lines_view(&ctx, &inv.lines, &cur_code)),
        views::card(
            &tr(l, "m_totals"),
            super::totals_block(&ctx, &inv.totals, Some(inv.paid_amount), &cur_code)
        )
    );
    let title = views::Title::raw(&inv.number);
    Ok(views::respond(views::page(&ctx, &title, body)))
}

pub async fn si_issue(ctx: Ctx, Path(id): Path<String>) -> Result<Response, AppError> {
    let mut inv = services::get_by_id::<Invoice>(&ctx.state.db.supplier_invoices(), &ctx.company.id, &id).await?;
    if inv.status != InvoiceStatus::Draft {
        return Ok(ctx.back(&format!("/supplier-invoices/{id}"), "msg_cannot_post_twice", "err").into_response());
    }
    inv.status = InvoiceStatus::Open;
    services::persist_invoice(&ctx.state.db, services::InvoiceKind::Supplier, &inv).await?;
    ctx.state
        .bus
        .publish(
            "supplier_invoice.issued",
            &new_event(
                "supplier_invoice.issued",
                &inv.company_id,
                &inv.id,
                &inv.number,
                inv.status.as_str(),
                inv.totals.total,
                &ctx.claims.un,
            ),
        )
        .await;
    Ok(ctx.back(&format!("/supplier-invoices/{id}"), "msg_issued", "ok").into_response())
}

pub async fn si_cancel(ctx: Ctx, Path(id): Path<String>) -> Result<Response, AppError> {
    let mut inv = services::get_by_id::<Invoice>(&ctx.state.db.supplier_invoices(), &ctx.company.id, &id).await?;
    inv.status = InvoiceStatus::Cancelled;
    services::persist_invoice(&ctx.state.db, services::InvoiceKind::Supplier, &inv).await?;
    Ok(ctx.back("/supplier-invoices", "msg_saved", "ok").into_response())
}

pub async fn si_pdf(ctx: Ctx, Path(id): Path<String>) -> Result<Response, AppError> {
    let inv = services::get_by_id::<Invoice>(&ctx.state.db.supplier_invoices(), &ctx.company.id, &id).await?;
    let supplier = ctx
        .state
        .db
        .suppliers()
        .find_one(doc! { "_id": &inv.party_id, "company_id": &ctx.company.id })
        .await?;
    let party = PartyBlock {
        name: inv.party_name.clone(),
        lines: supplier
            .map(|s| {
                vec![s.address, s.city, s.phone, s.email]
                    .into_iter()
                    .filter(|v| !v.trim().is_empty())
                    .collect()
            })
            .unwrap_or_default(),
    };
    let labels = PdfLabels::new(ctx.lang);
    let bytes = invoice_pdf(&inv, &ctx.company, &party, &labels);
    Ok(views::pdf_response(
        bytes,
        &format!("{}.pdf", inv.number.replace(['/', ' '], "-")),
    ))
}
