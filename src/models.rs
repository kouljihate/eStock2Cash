//! Domain models. Documents are stored as plain JSON-friendly structs: ids are
//! hex strings and dates are ISO strings so the same structs serialise cleanly
//! for both MongoDB and the mobile JSON API.

use serde::{Deserialize, Serialize};

pub const COLL_COMPANIES: &str = "companies";
pub const COLL_USERS: &str = "users";
pub const COLL_SUPPLIERS: &str = "suppliers";
pub const COLL_CUSTOMERS: &str = "customers";
pub const COLL_PRODUCTS: &str = "products";
pub const COLL_STOCK_MOVES: &str = "stock_moves";
pub const COLL_PURCHASE_ORDERS: &str = "purchase_orders";
pub const COLL_GOODS_RECEIPTS: &str = "goods_receipts";
pub const COLL_SUPPLIER_INVOICES: &str = "supplier_invoices";
pub const COLL_CUSTOMER_INVOICES: &str = "customer_invoices";
pub const COLL_PAYMENTS: &str = "payments";
pub const COLL_QUOTES: &str = "quotes";
pub const COLL_SALES_ORDERS: &str = "sales_orders";
pub const COLL_DELIVERY_NOTES: &str = "delivery_notes";
pub const COLL_TICKETS: &str = "tickets";
pub const COLL_COUNTERS: &str = "counters";
pub const COLL_EVENT_LOG: &str = "event_log";

macro_rules! status_enum {
    ($name:ident { $($variant:ident => $lit:literal),* $(,)? }) => {
        #[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug)]
        pub enum $name {
            $(#[serde(rename = $lit)] $variant),*
        }

        #[allow(dead_code)]
        impl $name {
            pub fn as_str(&self) -> &'static str {
                match self { $(Self::$variant => $lit),* }
            }

            /// i18n key, e.g. `st_draft`.
            pub fn key(&self) -> String {
                format!("st_{}", self.as_str())
            }

            pub fn parse(s: &str) -> Option<Self> {
                match s { $($lit => Some(Self::$variant),)* _ => None }
            }

            pub fn is(&self, other: Self) -> bool { *self == other }
        }
    };
}

status_enum!(PoStatus { Draft => "draft", Sent => "sent", PartiallyReceived => "partially_received", Received => "received", Cancelled => "cancelled" });
status_enum!(ReceiptStatus { Draft => "draft", Posted => "posted", Cancelled => "cancelled" });
status_enum!(InvoiceStatus { Draft => "draft", Open => "open", PartiallyPaid => "partially_paid", Paid => "paid", Overdue => "overdue", Cancelled => "cancelled" });
status_enum!(QuoteStatus { Draft => "draft", Sent => "sent", Accepted => "accepted", Rejected => "rejected", Expired => "expired", Converted => "converted" });
status_enum!(OrderStatus { Draft => "draft", Confirmed => "confirmed", Delivered => "delivered", Invoiced => "invoiced", Cancelled => "cancelled" });
status_enum!(DeliveryStatus { Draft => "draft", Posted => "posted", Cancelled => "cancelled" });
status_enum!(Role { LocalAdmin => "local_admin" });
status_enum!(PayDirection { Out => "out", In => "in" });
status_enum!(MoveKind { In => "in", Out => "out", Adjust => "adjust" });

impl InvoiceStatus {
    /// Status shown to the user: an unpaid invoice past its due date is overdue.
    pub fn effective(self, due_date: &str, outstanding: f64) -> Self {
        match self {
            InvoiceStatus::Open | InvoiceStatus::PartiallyPaid => {
                if outstanding > 0.005 && crate::util::days_until(due_date) < 0 {
                    InvoiceStatus::Overdue
                } else {
                    self
                }
            }
            other => other,
        }
    }

    pub fn is_settled(self) -> bool {
        matches!(self, InvoiceStatus::Paid | InvoiceStatus::Cancelled)
    }

    pub fn is_payable(self) -> bool {
        matches!(
            self,
            InvoiceStatus::Open | InvoiceStatus::PartiallyPaid | InvoiceStatus::Overdue
        )
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct InvoiceTemplate {
    pub primary: String,
    pub accent: String,
    pub font: String,
    pub header: String,
    pub footer: String,
    pub terms: String,
    pub show_logo: bool,
    pub show_tax: bool,
}

impl Default for InvoiceTemplate {
    fn default() -> Self {
        Self {
            primary: "#1f6feb".into(),
            accent: "#0f172a".into(),
            font: "Comfortaa".into(),
            header: String::new(),
            footer: String::new(),
            terms: "Payment due within 30 days.".into(),
            show_logo: true,
            show_tax: true,
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Company {
    #[serde(rename = "_id")]
    pub id: String,
    pub name: String,
    pub legal_name: String,
    pub tax_id: String,
    pub currency: String,
    pub address: String,
    pub phone: String,
    pub email: String,
    /// Base64 JPEG (optional) used on PDF documents.
    pub logo: String,
    pub template: InvoiceTemplate,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct User {
    #[serde(rename = "_id")]
    pub id: String,
    pub company_id: String,
    pub username: String,
    pub full_name: String,
    pub password_hash: String,
    pub password_salt: String,
    pub role: Role,
    pub locale: String,
    pub active: bool,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct Supplier {
    #[serde(rename = "_id")]
    pub id: String,
    pub company_id: String,
    pub code: String,
    pub name: String,
    pub legal_id: String,
    pub tax_id: String,
    pub email: String,
    pub phone: String,
    pub address: String,
    pub city: String,
    pub country: String,
    pub contact: String,
    pub notes: String,
    pub active: bool,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct Customer {
    #[serde(rename = "_id")]
    pub id: String,
    pub company_id: String,
    pub code: String,
    pub name: String,
    pub legal_id: String,
    pub tax_id: String,
    pub email: String,
    pub phone: String,
    pub address: String,
    pub city: String,
    pub country: String,
    pub contact: String,
    pub notes: String,
    /// 0 = unlimited.
    pub credit_limit: f64,
    /// Payment terms in days (used to compute invoice due dates).
    pub payment_terms: i64,
    pub active: bool,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct Product {
    #[serde(rename = "_id")]
    pub id: String,
    pub company_id: String,
    pub sku: String,
    pub name: String,
    pub unit: String,
    /// How the unit is packed and bought: "Carton of 12", "Sachet 500 g".
    #[serde(default)]
    pub package: String,
    pub category: String,
    pub cost: f64,
    pub price: f64,
    pub qty_on_hand: f64,
    pub reorder_level: f64,
    pub active: bool,
    pub created_at: String,
    pub updated_at: String,
}

impl Product {
    pub fn is_low(&self) -> bool {
        self.active && self.qty_on_hand <= self.reorder_level
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct StockMove {
    #[serde(rename = "_id")]
    pub id: String,
    pub company_id: String,
    pub product_id: String,
    pub product_name: String,
    pub kind: MoveKind,
    pub qty: f64,
    pub unit_cost: f64,
    /// e.g. `goods_receipt`, `delivery_note`, `ticket`, `adjustment`.
    pub source: String,
    pub source_number: String,
    pub note: String,
    pub created_at: String,
    pub created_by: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct DocLine {
    #[serde(default)]
    pub product_id: String,
    pub description: String,
    pub qty: f64,
    pub unit_price: f64,
    pub tax_rate: f64,
}

impl DocLine {
    pub fn net(&self) -> f64 {
        self.qty * self.unit_price
    }

    pub fn tax(&self) -> f64 {
        self.net() * self.tax_rate / 100.0
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct Totals {
    pub subtotal: f64,
    pub tax: f64,
    pub total: f64,
}

impl Totals {
    pub fn compute(lines: &[DocLine]) -> Self {
        let subtotal: f64 = lines.iter().map(|l| l.net()).sum();
        let tax: f64 = lines.iter().map(|l| l.tax()).sum();
        Self {
            subtotal: crate::util::round2(subtotal),
            tax: crate::util::round2(tax),
            total: crate::util::round2(subtotal + tax),
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct PurchaseOrder {
    #[serde(rename = "_id")]
    pub id: String,
    pub company_id: String,
    pub number: String,
    pub supplier_id: String,
    pub supplier_name: String,
    pub order_date: String,
    pub expected_date: String,
    pub status: PoStatus,
    pub lines: Vec<DocLine>,
    pub totals: Totals,
    pub notes: String,
    pub created_at: String,
    pub created_by: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct ReceiptLine {
    #[serde(default)]
    pub product_id: String,
    pub description: String,
    pub qty_ordered: f64,
    pub qty_received: f64,
    pub unit_cost: f64,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct GoodsReceipt {
    #[serde(rename = "_id")]
    pub id: String,
    pub company_id: String,
    pub number: String,
    pub po_id: String,
    pub po_number: String,
    pub supplier_id: String,
    pub supplier_name: String,
    pub receipt_date: String,
    pub status: ReceiptStatus,
    pub lines: Vec<ReceiptLine>,
    pub notes: String,
    pub created_at: String,
    pub created_by: String,
    pub posted_at: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Invoice {
    #[serde(rename = "_id")]
    pub id: String,
    pub company_id: String,
    pub number: String,
    /// Supplier id or customer id depending on the collection.
    pub party_id: String,
    pub party_name: String,
    /// Purchase order / goods receipt / sales order id this invoice bills.
    pub source_id: String,
    pub source_number: String,
    pub invoice_date: String,
    pub due_date: String,
    pub lines: Vec<DocLine>,
    pub totals: Totals,
    pub status: InvoiceStatus,
    pub paid_amount: f64,
    pub notes: String,
    pub created_at: String,
    pub created_by: String,
}

impl Invoice {
    pub fn outstanding(&self) -> f64 {
        crate::util::round2((self.totals.total - self.paid_amount).max(0.0))
    }

    pub fn display_status(&self) -> InvoiceStatus {
        self.status.effective(&self.due_date, self.outstanding())
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Payment {
    #[serde(rename = "_id")]
    pub id: String,
    pub company_id: String,
    pub number: String,
    pub direction: PayDirection,
    pub party_id: String,
    pub party_name: String,
    pub invoice_id: String,
    pub invoice_number: String,
    pub date: String,
    pub amount: f64,
    pub method: String,
    pub reference: String,
    pub notes: String,
    pub created_at: String,
    pub created_by: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Quote {
    #[serde(rename = "_id")]
    pub id: String,
    pub company_id: String,
    pub number: String,
    pub customer_id: String,
    pub customer_name: String,
    pub quote_date: String,
    pub valid_until: String,
    pub status: QuoteStatus,
    pub lines: Vec<DocLine>,
    pub totals: Totals,
    pub notes: String,
    pub converted_order_id: String,
    pub converted_invoice_id: String,
    pub created_at: String,
    pub created_by: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct SalesOrder {
    #[serde(rename = "_id")]
    pub id: String,
    pub company_id: String,
    pub number: String,
    pub customer_id: String,
    pub customer_name: String,
    pub quote_id: String,
    pub order_date: String,
    pub status: OrderStatus,
    pub lines: Vec<DocLine>,
    pub totals: Totals,
    pub notes: String,
    pub delivery_id: String,
    pub invoice_id: String,
    pub created_at: String,
    pub created_by: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct DeliveryLine {
    #[serde(default)]
    pub product_id: String,
    pub description: String,
    pub qty: f64,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct DeliveryNote {
    #[serde(rename = "_id")]
    pub id: String,
    pub company_id: String,
    pub number: String,
    pub order_id: String,
    pub order_number: String,
    pub customer_id: String,
    pub customer_name: String,
    pub delivery_date: String,
    pub status: DeliveryStatus,
    pub lines: Vec<DeliveryLine>,
    pub notes: String,
    pub created_at: String,
    pub created_by: String,
    pub posted_at: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct TicketItem {
    #[serde(default)]
    pub product_id: String,
    pub name: String,
    pub qty: f64,
    pub price: f64,
}

impl TicketItem {
    pub fn line_total(&self) -> f64 {
        crate::util::round2(self.qty * self.price)
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Ticket {
    #[serde(rename = "_id")]
    pub id: String,
    pub company_id: String,
    pub number: String,
    pub date: String,
    pub items: Vec<TicketItem>,
    pub subtotal: f64,
    pub tax: f64,
    pub total: f64,
    pub paid: f64,
    pub change: f64,
    pub cashier: String,
    /// The customer the cash was taken from, when the till was told who it is.
    /// Empty for a walk-in sale, and for the tickets written before the registry.
    #[serde(default)]
    pub customer_id: String,
    #[serde(default)]
    pub customer_name: String,
    pub created_at: String,
    pub created_by: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct EventLog {
    #[serde(rename = "_id")]
    pub id: String,
    pub routing_key: String,
    pub event_type: String,
    pub payload: serde_json::Value,
    pub received_at: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Counter {
    #[serde(rename = "_id")]
    pub id: String,
    pub seq: i64,
}

/// Payload published to RabbitMQ and mirrored into `event_log`.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct DomainEvent {
    pub event_type: String,
    pub company_id: String,
    pub document_id: String,
    pub document_number: String,
    pub status: String,
    pub amount: f64,
    pub actor: String,
    pub at: String,
}
