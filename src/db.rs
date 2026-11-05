//! MongoDB connection, indexes, numbering sequences and first-run seeding.

use chrono::Utc;
use mongodb::bson::doc;
use mongodb::options::{IndexOptions, ReturnDocument};
use mongodb::{Client, Database, IndexModel};

use crate::auth;
use crate::config::Config;
use crate::error::AppError;
use crate::models::*;
use crate::state::DbHandle;
use crate::util::now_iso;

pub async fn connect(uri: &str, db_name: &str) -> Result<Client, AppError> {
    let client = Client::with_uri_str(uri)
        .await
        .map_err(|e| AppError::Internal(format!("cannot reach MongoDB: {e}")))?;
    // Fail fast at boot instead of on the first request.
    client
        .database(db_name)
        .run_command(doc! { "ping": 1 })
        .await
        .map_err(|e| AppError::Internal(format!("MongoDB ping failed: {e}")))?;
    Ok(client)
}

/// Next document number, e.g. `PO-2026-0007`. The sequence is per company+prefix.
pub async fn next_number(db: &Database, company_id: &str, prefix: &str) -> Result<String, AppError> {
    let key = format!("{company_id}:{prefix}");
    let counters = db.collection::<Counter>(COLL_COUNTERS);
    let updated = counters
        .find_one_and_update(doc! { "_id": &key }, doc! { "$inc": { "seq": 1_i64 } })
        .upsert(true)
        .return_document(ReturnDocument::After)
        .await?;
    let seq = updated.map(|c| c.seq).unwrap_or(1);
    Ok(format!(
        "{prefix}-{}-{:04}",
        Utc::now().format("%Y"),
        seq
    ))
}

fn unique_index(keys: mongodb::bson::Document) -> IndexModel {
    IndexModel::builder()
        .keys(keys)
        .options(IndexOptions::builder().unique(true).build())
        .build()
}

fn plain_index(keys: mongodb::bson::Document) -> IndexModel {
    IndexModel::builder().keys(keys).build()
}

pub async fn ensure_indexes(db: &Database) -> Result<(), AppError> {
    let users = db.collection::<User>(COLL_USERS);
    users
        .create_index(unique_index(doc! { "username": 1 }))
        .await?;

    let suppliers = db.collection::<Supplier>(COLL_SUPPLIERS);
    suppliers
        .create_index(plain_index(doc! { "company_id": 1, "name": 1 }))
        .await?;
    suppliers
        .create_index(unique_index(doc! { "company_id": 1, "code": 1 }))
        .await?;

    let customers = db.collection::<Customer>(COLL_CUSTOMERS);
    customers
        .create_index(plain_index(doc! { "company_id": 1, "name": 1 }))
        .await?;
    customers
        .create_index(unique_index(doc! { "company_id": 1, "code": 1 }))
        .await?;

    let products = db.collection::<Product>(COLL_PRODUCTS);
    products
        .create_index(plain_index(doc! { "company_id": 1, "name": 1 }))
        .await?;
    products
        .create_index(unique_index(doc! { "company_id": 1, "sku": 1 }))
        .await?;

    for (name, coll) in [
        (COLL_PURCHASE_ORDERS, "purchase_orders"),
        (COLL_SUPPLIER_INVOICES, "supplier_invoices"),
    ] {
        let c = db.collection::<mongodb::bson::Document>(coll);
        c.create_index(unique_index(doc! { "company_id": 1, "number": 1 }))
            .await?;
        let _ = name;
    }

    for coll in [
        COLL_GOODS_RECEIPTS,
        COLL_CUSTOMER_INVOICES,
        COLL_QUOTES,
        COLL_SALES_ORDERS,
        COLL_DELIVERY_NOTES,
        COLL_TICKETS,
        COLL_PAYMENTS,
    ] {
        let c = db.collection::<mongodb::bson::Document>(coll);
        c.create_index(unique_index(doc! { "company_id": 1, "number": 1 }))
            .await?;
    }

    let invoices = db.collection::<mongodb::bson::Document>(COLL_CUSTOMER_INVOICES);
    invoices
        .create_index(plain_index(doc! { "company_id": 1, "party_id": 1, "status": 1 }))
        .await?;

    let moves = db.collection::<StockMove>(COLL_STOCK_MOVES);
    moves
        .create_index(plain_index(doc! { "company_id": 1, "product_id": 1, "created_at": -1 }))
        .await?;

    let events = db.collection::<EventLog>(COLL_EVENT_LOG);
    events
        .create_index(plain_index(doc! { "received_at": -1 }))
        .await?;

    Ok(())
}

/// Create the (single) company and its local administrator on an empty database.
pub async fn seed(db: &Database, cfg: &Config) -> Result<(), AppError> {
    let companies = db.collection::<Company>(COLL_COMPANIES);
    let users = db.collection::<User>(COLL_USERS);
    let now = now_iso();

    let company = match companies.find_one(doc! {}).await? {
        Some(c) => c,
        None => {
            let company = Company {
                id: crate::util::new_id(),
                name: cfg.company_name.clone(),
                legal_name: cfg.company_name.clone(),
                tax_id: String::new(),
                currency: "USD".into(),
                address: String::new(),
                phone: String::new(),
                email: String::new(),
                logo: String::new(),
                template: InvoiceTemplate::default(),
                created_at: now.clone(),
                updated_at: now.clone(),
            };
            companies.insert_one(&company).await?;
            tracing::info!(company = %company.name, "seeded default company");
            company
        }
    };

    let existing = users.find_one(doc! { "username": &cfg.admin_username }).await?;
    if existing.is_none() {
        let (salt, hash) = auth::hash_password(&cfg.admin_password)?;
        let admin = User {
            id: crate::util::new_id(),
            company_id: company.id.clone(),
            username: cfg.admin_username.clone(),
            full_name: "Local Administrator".into(),
            password_hash: hash,
            password_salt: salt,
            role: Role::LocalAdmin,
            locale: cfg.default_locale.clone(),
            active: true,
            created_at: now.clone(),
            updated_at: now.clone(),
        };
        users.insert_one(&admin).await?;
        tracing::info!(username = %cfg.admin_username, "seeded local administrator");
    } else if existing.as_ref().map(|u| u.company_id.is_empty()).unwrap_or(false) {
        let id = existing.unwrap().id;
        users
            .update_one(
                doc! { "_id": &id },
                doc! { "$set": { "company_id": &company.id } },
            )
            .await?;
    }

    Ok(())
}

pub async fn handle(client: Client, db_name: &str) -> DbHandle {
    DbHandle {
        db: client.database(db_name),
    }
}
