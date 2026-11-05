//! Shared application state and MongoDB handles.

use std::sync::Arc;

use mongodb::Database;
use mongodb::bson::doc;

use crate::config::Config;
use crate::error::AppError;
use crate::events::Bus;
use crate::models::*;

#[derive(Clone)]
pub struct DbHandle {
    pub db: Database,
}

impl DbHandle {
    pub fn coll<T: Send + Sync>(&self, name: &str) -> mongodb::Collection<T> {
        self.db.collection::<T>(name)
    }

    pub fn companies(&self) -> mongodb::Collection<Company> {
        self.coll(COLL_COMPANIES)
    }
    pub fn users(&self) -> mongodb::Collection<User> {
        self.coll(COLL_USERS)
    }
    pub fn suppliers(&self) -> mongodb::Collection<Supplier> {
        self.coll(COLL_SUPPLIERS)
    }
    pub fn customers(&self) -> mongodb::Collection<Customer> {
        self.coll(COLL_CUSTOMERS)
    }
    pub fn products(&self) -> mongodb::Collection<Product> {
        self.coll(COLL_PRODUCTS)
    }
    pub fn stock_moves(&self) -> mongodb::Collection<StockMove> {
        self.coll(COLL_STOCK_MOVES)
    }
    pub fn purchase_orders(&self) -> mongodb::Collection<PurchaseOrder> {
        self.coll(COLL_PURCHASE_ORDERS)
    }
    pub fn goods_receipts(&self) -> mongodb::Collection<GoodsReceipt> {
        self.coll(COLL_GOODS_RECEIPTS)
    }
    pub fn supplier_invoices(&self) -> mongodb::Collection<Invoice> {
        self.coll(COLL_SUPPLIER_INVOICES)
    }
    pub fn customer_invoices(&self) -> mongodb::Collection<Invoice> {
        self.coll(COLL_CUSTOMER_INVOICES)
    }
    pub fn payments(&self) -> mongodb::Collection<Payment> {
        self.coll(COLL_PAYMENTS)
    }
    pub fn quotes(&self) -> mongodb::Collection<Quote> {
        self.coll(COLL_QUOTES)
    }
    pub fn sales_orders(&self) -> mongodb::Collection<SalesOrder> {
        self.coll(COLL_SALES_ORDERS)
    }
    pub fn delivery_notes(&self) -> mongodb::Collection<DeliveryNote> {
        self.coll(COLL_DELIVERY_NOTES)
    }
    pub fn tickets(&self) -> mongodb::Collection<Ticket> {
        self.coll(COLL_TICKETS)
    }
    pub fn event_log(&self) -> mongodb::Collection<EventLog> {
        self.coll(COLL_EVENT_LOG)
    }

    pub async fn company(&self, id: &str) -> Result<Company, AppError> {
        self.companies()
            .find_one(doc! { "_id": id })
            .await?
            .ok_or_else(|| AppError::NotFound("Company not found".into()))
    }

    pub async fn user(&self, id: &str) -> Result<User, AppError> {
        self.users()
            .find_one(doc! { "_id": id })
            .await?
            .ok_or_else(|| AppError::NotFound("User not found".into()))
    }
}

#[derive(Clone)]
pub struct AppState {
    pub db: DbHandle,
    pub cfg: Arc<Config>,
    pub bus: Bus,
}

impl AppState {
    pub fn new(db: DbHandle, cfg: Arc<Config>, bus: Bus) -> Self {
        Self { db, cfg, bus }
    }
}
