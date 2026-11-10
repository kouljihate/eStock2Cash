//! Shared application state and MongoDB handles.

use std::sync::{Arc, RwLock};

use mongodb::Database;
use mongodb::bson::doc;

use crate::config::Config;
use crate::error::AppError;
use crate::events::Bus;
use crate::models::*;

/// The database every request talks to. The handle is shared and swappable: the
/// settings page can point a running app at a database server on the network (or
/// back at the local one) without a restart, and the next query uses it.
#[derive(Clone)]
pub struct DbHandle {
    inner: Arc<RwLock<Database>>,
}

impl DbHandle {
    pub fn new(db: Database) -> Self {
        Self {
            inner: Arc::new(RwLock::new(db)),
        }
    }

    /// A clone of the current database handle. Cheap: the driver pools the sockets.
    pub fn db(&self) -> Database {
        self.inner
            .read()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .clone()
    }

    /// Point the whole running app at another database.
    pub fn swap(&self, db: Database) {
        *self
            .inner
            .write()
            .unwrap_or_else(|poisoned| poisoned.into_inner()) = db;
    }

    pub fn coll<T: Send + Sync>(&self, name: &str) -> mongodb::Collection<T> {
        self.db().collection::<T>(name)
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
    /// The database choice in force right now. `cfg.db` is the value at boot; this is
    /// what the settings page reads, so a saved change is visible without a restart.
    pub db_settings: Arc<RwLock<crate::config::DbSettings>>,
}

impl AppState {
    pub fn new(db: DbHandle, cfg: Arc<Config>, bus: Bus) -> Self {
        let settings = cfg.db.clone();
        Self {
            db,
            cfg,
            bus,
            db_settings: Arc::new(RwLock::new(settings)),
        }
    }
}
