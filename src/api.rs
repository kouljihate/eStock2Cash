//! JSON API for the Android client (`/api/v1`). Bearer JWT authentication;
//! every query is scoped to the token's company.

use axum::extract::{FromRequestParts, Path, Query, State};
use axum::http::request::Parts;
use axum::routing::{get, post};
use axum::{Json, Router};
use futures_util::TryStreamExt;
use mongodb::bson::doc;
use serde::Deserialize;
use serde_json::{Value, json};
use std::collections::HashMap;

use crate::auth::{self, Claims};
use crate::error::{ApiError, AppError};
use crate::models::*;
use crate::pdf::{PartyBlock, PdfLabels, invoice_pdf};
use crate::services;
use crate::state::AppState;
use crate::util::{now_iso, round2, today};

pub struct ApiAuth {
    pub claims: Claims,
    pub state: AppState,
}

impl FromRequestParts<AppState> for ApiAuth {
    type Rejection = ApiError;

    async fn from_request_parts(parts: &mut Parts, state: &AppState) -> Result<Self, Self::Rejection> {
        let claims = auth::claims_from_headers(&parts.headers, &state.cfg)
            .ok_or(ApiError(AppError::Unauthorized))?;
        Ok(ApiAuth {
            claims,
            state: state.clone(),
        })
    }
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/health", get(health))
        .route("/auth/login", post(login))
        .route("/me", get(me))
        .route("/dashboard", get(dashboard))
        .route("/suppliers", get(suppliers).post(create_supplier))
        .route("/customers", get(customers).post(create_customer))
        .route("/products", get(products).post(create_product))
        .route("/purchase-orders", get(purchase_orders))
        .route("/purchase-orders/{id}", get(purchase_order))
        .route("/supplier-invoices", get(supplier_invoices))
        .route("/customer-invoices", get(customer_invoices))
        .route("/customer-invoices/{id}", get(customer_invoice))
        .route("/customer-invoices/{id}/pdf", get(customer_invoice_pdf))
        .route("/quotes", get(quotes))
        .route("/customer-orders", get(customer_orders))
        .route("/payments", get(payments))
        .route("/tickets", get(tickets).post(create_ticket))
        .route("/credit", get(credit))
}

async fn health() -> Json<Value> {
    Json(json!({ "ok": true, "service": "stockinvoice", "version": env!("CARGO_PKG_VERSION") }))
}

#[derive(Deserialize)]
struct LoginRequest {
    username: String,
    password: String,
}

async fn login(
    State(state): State<AppState>,
    Json(body): Json<LoginRequest>,
) -> Result<Json<Value>, ApiError> {
    let user = state
        .db
        .users()
        .find_one(doc! { "username": &body.username })
        .await?
        .filter(|u| u.active)
        .ok_or(ApiError(AppError::Unauthorized))?;
    if !auth::verify_password(&body.password, &user.password_salt, &user.password_hash) {
        return Err(ApiError(AppError::Unauthorized));
    }
    let token = auth::issue_token(&user, &state.cfg).map_err(ApiError)?;
    let company = state.db.company(&user.company_id).await.map_err(ApiError)?;
    Ok(Json(json!({
        "ok": true,
        "token": token,
        "expires_in": state.cfg.jwt_expires,
        "user": {
            "id": user.id,
            "username": user.username,
            "full_name": user.full_name,
            "role": user.role.as_str(),
            "locale": user.locale,
        },
        "company": { "id": company.id, "name": company.name, "currency": company.currency },
    })))
}

async fn me(auth: ApiAuth) -> Result<Json<Value>, ApiError> {
    let user = auth.state.db.user(&auth.claims.sub).await.map_err(ApiError)?;
    let company = auth.state.db.company(&auth.claims.cid).await.map_err(ApiError)?;
    Ok(Json(json!({
        "ok": true,
        "user": user,
        "company": company,
    })))
}

async fn dashboard(auth: ApiAuth) -> Result<Json<Value>, ApiError> {
    let stats = services::dashboard_stats(&auth.state.db, &auth.claims.cid)
        .await
        .map_err(ApiError)?;
    Ok(Json(json!({
        "ok": true,
        "suppliers": stats.suppliers,
        "customers": stats.customers,
        "products": stats.products,
        "low_stock": stats.low_stock.len(),
        "stock_value": stats.stock_value,
        "receivables": stats.receivables,
        "payables": stats.payables,
        "open_purchase_orders": stats.open_pos,
        "open_quotes": stats.open_quotes,
        "aging": {
            "current": stats.aging.current,
            "d30": stats.aging.d30,
            "d60": stats.aging.d60,
            "d90": stats.aging.d90,
            "total": stats.aging.total,
        }
    })))
}

fn search_filter(company_id: &str, q: &Option<String>, fields: &[&str]) -> mongodb::bson::Document {
    match q {
        Some(term) if !term.trim().is_empty() => {
            let re = mongodb::bson::Regex {
                pattern: crate::pages::regex_escape(term.trim()),
                options: "i".into(),
            };
            let ors: Vec<mongodb::bson::Document> = fields
                .iter()
                .map(|f| doc! { (*f): { "$regex": re.clone() } })
                .collect();
            doc! { "company_id": company_id, "$or": ors }
        }
        _ => doc! { "company_id": company_id },
    }
}

async fn suppliers(
    auth: ApiAuth,
    Query(q): Query<HashMap<String, String>>,
) -> Result<Json<Value>, ApiError> {
    let filter = search_filter(&auth.claims.cid, &q.get("q").cloned(), &["name", "code", "email"]);
    let mut cur = auth
        .state
        .db
        .suppliers()
        .find(filter)
        .sort(doc! { "name": 1 })
        .limit(500)
        .await
        .map_err(|e| ApiError(e.into()))?;
    let mut items = Vec::new();
    while let Some(s) = cur.try_next().await.map_err(|e| ApiError(e.into()))? {
        items.push(s);
    }
    Ok(Json(json!({ "ok": true, "count": items.len(), "items": items })))
}

async fn customers(
    auth: ApiAuth,
    Query(q): Query<HashMap<String, String>>,
) -> Result<Json<Value>, ApiError> {
    let filter = search_filter(&auth.claims.cid, &q.get("q").cloned(), &["name", "code", "email"]);
    let mut cur = auth
        .state
        .db
        .customers()
        .find(filter)
        .sort(doc! { "name": 1 })
        .limit(500)
        .await
        .map_err(|e| ApiError(e.into()))?;
    let mut items = Vec::new();
    while let Some(c) = cur.try_next().await.map_err(|e| ApiError(e.into()))? {
        let exposure = services::customer_exposure(&auth.state.db, &auth.claims.cid, &c.id)
            .await
            .map_err(ApiError)?;
        items.push(json!({
            "id": c.id,
            "code": c.code,
            "name": c.name,
            "email": c.email,
            "phone": c.phone,
            "credit_limit": c.credit_limit,
            "exposure": exposure,
            "payment_terms": c.payment_terms,
            "active": c.active,
        }));
    }
    Ok(Json(json!({ "ok": true, "count": items.len(), "items": items })))
}

async fn products(
    auth: ApiAuth,
    Query(q): Query<HashMap<String, String>>,
) -> Result<Json<Value>, ApiError> {
    let filter = search_filter(&auth.claims.cid, &q.get("q").cloned(), &["name", "sku", "category"]);
    let mut cur = auth
        .state
        .db
        .products()
        .find(filter)
        .sort(doc! { "name": 1 })
        .limit(1000)
        .await
        .map_err(|e| ApiError(e.into()))?;
    let mut items = Vec::new();
    while let Some(p) = cur.try_next().await.map_err(|e| ApiError(e.into()))? {
        items.push(p);
    }
    Ok(Json(json!({ "ok": true, "count": items.len(), "items": items })))
}

#[derive(Deserialize)]
struct SupplierRequest {
    code: String,
    name: String,
    #[serde(default)]
    email: String,
    #[serde(default)]
    phone: String,
    #[serde(default)]
    address: String,
    #[serde(default)]
    city: String,
    #[serde(default)]
    tax_id: String,
}

async fn create_supplier(
    auth: ApiAuth,
    Json(body): Json<SupplierRequest>,
) -> Result<Json<Value>, ApiError> {
    if body.code.trim().is_empty() || body.name.trim().is_empty() {
        return Err(ApiError(AppError::BadRequest("code and name are required".into())));
    }
    let now = now_iso();
    let supplier = Supplier {
        id: crate::util::new_id(),
        company_id: auth.claims.cid.clone(),
        code: body.code,
        name: body.name,
        legal_id: String::new(),
        tax_id: body.tax_id,
        email: body.email,
        phone: body.phone,
        address: body.address,
        city: body.city,
        country: String::new(),
        contact: String::new(),
        notes: String::new(),
        active: true,
        created_at: now.clone(),
        updated_at: now,
    };
    auth.state
        .db
        .suppliers()
        .insert_one(&supplier)
        .await
        .map_err(|e| ApiError(e.into()))?;
    Ok(Json(json!({ "ok": true, "item": supplier })))
}

#[derive(Deserialize)]
struct CustomerRequest {
    code: String,
    name: String,
    #[serde(default)]
    email: String,
    #[serde(default)]
    phone: String,
    #[serde(default)]
    address: String,
    #[serde(default)]
    city: String,
    #[serde(default)]
    credit_limit: f64,
    #[serde(default = "default_terms")]
    payment_terms: i64,
}

fn default_terms() -> i64 {
    30
}

async fn create_customer(
    auth: ApiAuth,
    Json(body): Json<CustomerRequest>,
) -> Result<Json<Value>, ApiError> {
    if body.code.trim().is_empty() || body.name.trim().is_empty() {
        return Err(ApiError(AppError::BadRequest("code and name are required".into())));
    }
    let now = now_iso();
    let customer = Customer {
        id: crate::util::new_id(),
        company_id: auth.claims.cid.clone(),
        code: body.code,
        name: body.name,
        legal_id: String::new(),
        tax_id: String::new(),
        email: body.email,
        phone: body.phone,
        address: body.address,
        city: body.city,
        country: String::new(),
        contact: String::new(),
        notes: String::new(),
        credit_limit: body.credit_limit,
        payment_terms: body.payment_terms,
        active: true,
        created_at: now.clone(),
        updated_at: now,
    };
    auth.state
        .db
        .customers()
        .insert_one(&customer)
        .await
        .map_err(|e| ApiError(e.into()))?;
    Ok(Json(json!({ "ok": true, "item": customer })))
}

#[derive(Deserialize)]
struct ProductRequest {
    sku: String,
    name: String,
    #[serde(default)]
    unit: String,
    #[serde(default)]
    category: String,
    #[serde(default)]
    cost: f64,
    #[serde(default)]
    price: f64,
    #[serde(default)]
    qty_on_hand: f64,
    #[serde(default)]
    reorder_level: f64,
}

async fn create_product(
    auth: ApiAuth,
    Json(body): Json<ProductRequest>,
) -> Result<Json<Value>, ApiError> {
    if body.sku.trim().is_empty() || body.name.trim().is_empty() {
        return Err(ApiError(AppError::BadRequest("sku and name are required".into())));
    }
    let now = now_iso();
    let product = Product {
        id: crate::util::new_id(),
        company_id: auth.claims.cid.clone(),
        sku: body.sku,
        name: body.name,
        unit: if body.unit.is_empty() { "pcs".into() } else { body.unit },
        category: body.category,
        cost: body.cost,
        price: body.price,
        qty_on_hand: body.qty_on_hand,
        reorder_level: body.reorder_level,
        active: true,
        created_at: now.clone(),
        updated_at: now,
    };
    auth.state
        .db
        .products()
        .insert_one(&product)
        .await
        .map_err(|e| ApiError(e.into()))?;
    Ok(Json(json!({ "ok": true, "item": product })))
}

async fn purchase_orders(auth: ApiAuth) -> Result<Json<Value>, ApiError> {
    let items = services::list_recent::<PurchaseOrder>(
        &auth.state.db.purchase_orders(),
        &auth.claims.cid,
        200,
    )
    .await
    .map_err(ApiError)?;
    Ok(Json(json!({ "ok": true, "count": items.len(), "items": items })))
}

async fn purchase_order(auth: ApiAuth, Path(id): Path<String>) -> Result<Json<Value>, ApiError> {
    let po = services::get_by_id::<PurchaseOrder>(&auth.state.db.purchase_orders(), &auth.claims.cid, &id)
        .await
        .map_err(ApiError)?;
    Ok(Json(json!({ "ok": true, "item": po })))
}

async fn supplier_invoices(auth: ApiAuth) -> Result<Json<Value>, ApiError> {
    let items = services::list_recent::<Invoice>(
        &auth.state.db.supplier_invoices(),
        &auth.claims.cid,
        200,
    )
    .await
    .map_err(ApiError)?;
    Ok(Json(json!({ "ok": true, "count": items.len(), "items": items })))
}

async fn customer_invoices(auth: ApiAuth) -> Result<Json<Value>, ApiError> {
    let items = services::list_recent::<Invoice>(
        &auth.state.db.customer_invoices(),
        &auth.claims.cid,
        200,
    )
    .await
    .map_err(ApiError)?;
    Ok(Json(json!({ "ok": true, "count": items.len(), "items": items })))
}

async fn customer_invoice(auth: ApiAuth, Path(id): Path<String>) -> Result<Json<Value>, ApiError> {
    let inv = services::get_by_id::<Invoice>(&auth.state.db.customer_invoices(), &auth.claims.cid, &id)
        .await
        .map_err(ApiError)?;
    Ok(Json(json!({
        "ok": true,
        "item": inv,
        "outstanding": inv.outstanding(),
        "status": inv.display_status().as_str(),
    })))
}

async fn customer_invoice_pdf(
    auth: ApiAuth,
    Path(id): Path<String>,
) -> Result<axum::response::Response, ApiError> {
    let inv = services::get_by_id::<Invoice>(&auth.state.db.customer_invoices(), &auth.claims.cid, &id)
        .await
        .map_err(ApiError)?;
    let company = auth.state.db.company(&auth.claims.cid).await.map_err(ApiError)?;
    let customer = auth
        .state
        .db
        .customers()
        .find_one(doc! { "_id": &inv.party_id })
        .await
        .map_err(|e| ApiError(e.into()))?;
    let party = PartyBlock {
        name: inv.party_name.clone(),
        lines: customer
            .map(|c| vec![c.address, c.city, c.phone].into_iter().filter(|v| !v.trim().is_empty()).collect())
            .unwrap_or_default(),
    };
    let labels = PdfLabels::new(crate::i18n::Lang::En);
    let bytes = invoice_pdf(&inv, &company, &party, &labels);
    Ok(crate::views::pdf_response(
        bytes,
        &format!("{}.pdf", inv.number.replace(['/', ' '], "-")),
    ))
}

async fn quotes(auth: ApiAuth) -> Result<Json<Value>, ApiError> {
    let items = services::list_recent::<Quote>(&auth.state.db.quotes(), &auth.claims.cid, 200)
        .await
        .map_err(ApiError)?;
    Ok(Json(json!({ "ok": true, "count": items.len(), "items": items })))
}

async fn customer_orders(auth: ApiAuth) -> Result<Json<Value>, ApiError> {
    let items = services::list_recent::<SalesOrder>(&auth.state.db.sales_orders(), &auth.claims.cid, 200)
        .await
        .map_err(ApiError)?;
    Ok(Json(json!({ "ok": true, "count": items.len(), "items": items })))
}

async fn payments(auth: ApiAuth) -> Result<Json<Value>, ApiError> {
    let items = services::list_recent::<Payment>(&auth.state.db.payments(), &auth.claims.cid, 200)
        .await
        .map_err(ApiError)?;
    Ok(Json(json!({ "ok": true, "count": items.len(), "items": items })))
}

async fn tickets(auth: ApiAuth) -> Result<Json<Value>, ApiError> {
    let items = services::list_recent::<Ticket>(&auth.state.db.tickets(), &auth.claims.cid, 100)
        .await
        .map_err(ApiError)?;
    Ok(Json(json!({ "ok": true, "count": items.len(), "items": items })))
}

#[derive(Deserialize)]
struct TicketItemRequest {
    #[serde(default)]
    product_id: String,
    #[serde(default)]
    name: String,
    qty: f64,
    #[serde(default)]
    price: f64,
}

#[derive(Deserialize)]
struct TicketRequest {
    items: Vec<TicketItemRequest>,
    #[serde(default)]
    tax: f64,
    #[serde(default)]
    paid: f64,
    #[serde(default)]
    cashier: String,
}

async fn create_ticket(
    auth: ApiAuth,
    Json(body): Json<TicketRequest>,
) -> Result<Json<Value>, ApiError> {
    if body.items.is_empty() {
        return Err(ApiError(AppError::BadRequest("items are required".into())));
    }
    let mut items = Vec::new();
    for it in body.items {
        let mut name = it.name;
        let mut price = it.price;
        if !it.product_id.is_empty() {
            if let Ok(p) = services::get_by_id::<Product>(&auth.state.db.products(), &auth.claims.cid, &it.product_id).await {
                if name.trim().is_empty() {
                    name = p.name.clone();
                }
                if price <= 0.0 {
                    price = p.price;
                }
            }
        }
        if it.qty <= 0.0 {
            return Err(ApiError(AppError::BadRequest("quantity must be positive".into())));
        }
        items.push(TicketItem {
            product_id: it.product_id,
            name: if name.trim().is_empty() { "Item".into() } else { name },
            qty: it.qty,
            price: round2(price),
        });
    }
    let subtotal = round2(items.iter().map(|i| i.line_total()).sum::<f64>());
    let tax = round2(body.tax.max(0.0));
    let total = round2(subtotal + tax);
    let paid = round2(if body.paid <= 0.0 { total } else { body.paid });
    let number = crate::db::next_number(&auth.state.db.db, &auth.claims.cid, "TK")
        .await
        .map_err(ApiError)?;
    let ticket = Ticket {
        id: crate::util::new_id(),
        company_id: auth.claims.cid.clone(),
        number,
        date: today(),
        items,
        subtotal,
        tax,
        total,
        paid,
        change: round2((paid - total).max(0.0)),
        cashier: if body.cashier.is_empty() { auth.claims.un.clone() } else { body.cashier },
        customer_id: String::new(),
        customer_name: String::new(),
        created_at: now_iso(),
        created_by: auth.claims.un.clone(),
    };
    auth.state
        .db
        .tickets()
        .insert_one(&ticket)
        .await
        .map_err(|e| ApiError(e.into()))?;
    services::post_ticket(&auth.state.db, &ticket).await.map_err(ApiError)?;
    auth.state
        .bus
        .publish(
            "ticket.created",
            &crate::events::new_event(
                "ticket.created",
                &ticket.company_id,
                &ticket.id,
                &ticket.number,
                "paid",
                ticket.total,
                &auth.claims.un,
            ),
        )
        .await;
    Ok(Json(json!({ "ok": true, "item": ticket })))
}

async fn credit(auth: ApiAuth) -> Result<Json<Value>, ApiError> {
    let mut cur = auth
        .state
        .db
        .customers()
        .find(doc! { "company_id": &auth.claims.cid })
        .sort(doc! { "name": 1 })
        .await
        .map_err(|e| ApiError(e.into()))?;
    let mut items = Vec::new();
    while let Some(c) = cur.try_next().await.map_err(|e| ApiError(e.into()))? {
        let pos = services::credit_position(&auth.state.db, &auth.claims.cid, &c).await.map_err(ApiError)?;
        items.push(json!({
            "id": c.id,
            "name": c.name,
            "limit": pos.limit,
            "exposure": pos.exposure,
            "available": if pos.unlimited() { Value::Null } else { json!(pos.available()) },
        }));
    }
    let aging = services::aging_report(&auth.state.db, &auth.claims.cid)
        .await
        .map_err(ApiError)?;
    Ok(Json(json!({
        "ok": true,
        "customers": items,
        "aging": {
            "current": aging.current,
            "d30": aging.d30,
            "d60": aging.d60,
            "d90": aging.d90,
            "total": aging.total,
        }
    })))
}
