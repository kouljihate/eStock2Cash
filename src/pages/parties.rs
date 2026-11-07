//! Suppliers and customers (CRUD + customer credit view).

use axum::extract::{Path, Query};
use axum::response::{IntoResponse, Response};
use futures_util::TryStreamExt;
use mongodb::bson::doc;
use std::collections::HashMap;

use crate::auth::Ctx;
use crate::error::AppError;
use crate::form::Pairs;
use crate::i18n::tr;
use crate::models::{Customer, Invoice, InvoiceStatus, Supplier};
use crate::services;
use crate::util::{esc, now_iso};
use crate::views;

// ---------------------------------------------------------------------------
// Suppliers
// ---------------------------------------------------------------------------

pub async fn suppliers(ctx: Ctx, Query(q): Query<HashMap<String, String>>) -> Result<Response, AppError> {
    let search = q.get("q").cloned().unwrap_or_default();
    let filter = if search.trim().is_empty() {
        doc! { "company_id": &ctx.company.id }
    } else {
        let re = mongodb::bson::Regex {
            pattern: crate::pages::regex_escape(search.trim()),
            options: "i".into(),
        };
        doc! {
            "company_id": &ctx.company.id,
            "$or": [
                { "name": { "$regex": re.clone() } },
                { "code": { "$regex": re.clone() } },
                { "email": { "$regex": re } },
            ]
        }
    };
    let mut cur = ctx
        .state
        .db
        .suppliers()
        .find(filter)
        .sort(doc! { "name": 1 })
        .await?;
    let mut items = Vec::new();
    while let Some(s) = cur.try_next().await? {
        items.push(s);
    }

    let l = ctx.lang;
    let rows = items
        .iter()
        .map(|s| {
            vec![
                format!(
                    "<a href=\"/suppliers/{}/edit\">{}</a><div class=\"muted small\">{}</div>",
                    esc(&s.id),
                    esc(&s.name),
                    esc(&s.code)
                ),
                esc(&s.contact),
                format!("{}{}", esc(&s.email), if s.phone.is_empty() { String::new() } else { format!("<div class=\"muted small\">{}</div>", esc(&s.phone)) }),
                views::status_badge(
                    l,
                    if s.active { "st_posted" } else { "st_cancelled" },
                ),
                format!(
                    "<div class=\"row-actions\">{}{}</div>",
                    views::icon_link(&format!("/purchase-orders/new?supplier={}", s.id), "po", &tr(l, "nav_purchase_orders")),
                    views::icon_post(&format!("/suppliers/{}/delete", s.id), "trash", &tr(l, "a_delete"))
                ),
            ]
        })
        .collect::<Vec<_>>();

    let body = format!(
        "{}{}",
        super::page_head(
            &ctx,
            "nav_suppliers",
            "",
            vec![
                super::search_form(l, "/suppliers", &search, ""),
                views::link_btn("/suppliers/new", &format!("+ {}", tr(l, "a_new"))),
            ]
        ),
        views::table_actions(
            l,
            &[
                &tr(l, "lbl_name"),
                &tr(l, "lbl_contact"),
                &tr(l, "lbl_email"),
                &tr(l, "lbl_status"),
                &tr(l, "lbl_actions"),
            ],
            rows,
            "msg_no_records",
        )
    );
    let title = views::Title::of(l, "nav_suppliers");
    Ok(views::respond(views::page(&ctx, &title, body)))
}

fn supplier_fields(l: crate::i18n::Lang, s: &Supplier) -> String {
    format!(
        "<div class=\"form-grid\">{}{}{}{}{}{}{}{}{}{}</div>",
        views::input(&tr(l, "lbl_code"), "code", &s.code, "text", true),
        views::input(&tr(l, "lbl_name"), "name", &s.name, "text", true),
        views::input(&tr(l, "lbl_legal_id"), "legal_id", &s.legal_id, "text", false),
        views::input(&tr(l, "lbl_tax_id"), "tax_id", &s.tax_id, "text", false),
        views::input(&tr(l, "lbl_contact"), "contact", &s.contact, "text", false),
        views::input(&tr(l, "lbl_email"), "email", &s.email, "email", false),
        views::input(&tr(l, "lbl_phone"), "phone", &s.phone, "text", false),
        views::input(&tr(l, "lbl_city"), "city", &s.city, "text", false),
        views::input(&tr(l, "lbl_country"), "country", &s.country, "text", false),
        views::checkbox(&tr(l, "lbl_active"), "active", s.active),
    )
}

pub async fn supplier_new(ctx: Ctx) -> Result<Response, AppError> {
    let l = ctx.lang;
    let empty = Supplier {
        active: true,
        ..Default::default()
    };
    let body = format!(
        "{}<form method=\"post\" action=\"/suppliers\" class=\"card form\">{}<div class=\"form-grid\">{}</div>{}</form>",
        super::page_head(&ctx, "nav_suppliers", "", vec![]),
        supplier_fields(l, &empty),
        views::textarea(&tr(l, "lbl_address"), "address", ""),
        views::form_actions(&tr(l, "a_save"), "/suppliers", &tr(l, "a_back")),
    );
    let title = views::Title::of(l, "nav_suppliers").join(l, "a_new");
    Ok(views::respond(views::page(&ctx, &title, body)))
}

fn supplier_from_pairs(pairs: &Pairs, company_id: &str) -> Supplier {
    let now = now_iso();
    Supplier {
        id: crate::util::new_id(),
        company_id: company_id.to_string(),
        code: pairs.get("code"),
        name: pairs.get("name"),
        legal_id: pairs.get("legal_id"),
        tax_id: pairs.get("tax_id"),
        email: pairs.get("email"),
        phone: pairs.get("phone"),
        address: pairs.get("address"),
        city: pairs.get("city"),
        country: pairs.get("country"),
        contact: pairs.get("contact"),
        notes: pairs.get("notes"),
        active: pairs.flag("active"),
        created_at: now.clone(),
        updated_at: now,
    }
}

pub async fn supplier_create(ctx: Ctx, pairs: Pairs) -> Result<Response, AppError> {
    let s = supplier_from_pairs(&pairs, &ctx.company.id);
    if s.name.trim().is_empty() || s.code.trim().is_empty() {
        return Ok(ctx.back("/suppliers/new", "msg_required", "err").into_response());
    }
    match ctx.state.db.suppliers().insert_one(&s).await {
        Ok(_) => Ok(ctx.back("/suppliers", "msg_saved", "ok").into_response()),
        Err(e) if e.to_string().contains("duplicate key") => {
            Ok(ctx.back("/suppliers/new", "msg_duplicate", "err").into_response())
        }
        Err(e) => Err(e.into()),
    }
}

pub async fn supplier_edit(ctx: Ctx, Path(id): Path<String>) -> Result<Response, AppError> {
    let l = ctx.lang;
    let s = services::get_by_id::<Supplier>(&ctx.state.db.suppliers(), &ctx.company.id, &id).await?;
    let body = format!(
        "{}<form method=\"post\" action=\"/suppliers/{}\" class=\"card form\">{}<div class=\"form-grid\">{}</div>{}</form>",
        super::page_head(&ctx, "nav_suppliers", &format!("{} · {}", s.code, s.name), vec![]),
        esc(&s.id),
        supplier_fields(l, &s),
        views::textarea(&tr(l, "lbl_address"), "address", &s.address),
        views::form_actions(&tr(l, "a_save"), "/suppliers", &tr(l, "a_back")),
    );
    let title = views::Title::of(l, "a_edit");
    Ok(views::respond(views::page(&ctx, &title, body)))
}

pub async fn supplier_update(ctx: Ctx, Path(id): Path<String>, pairs: Pairs) -> Result<Response, AppError> {
    let mut s = services::get_by_id::<Supplier>(&ctx.state.db.suppliers(), &ctx.company.id, &id).await?;
    s.code = pairs.get("code");
    s.name = pairs.get("name");
    s.legal_id = pairs.get("legal_id");
    s.tax_id = pairs.get("tax_id");
    s.email = pairs.get("email");
    s.phone = pairs.get("phone");
    s.address = pairs.get("address");
    s.city = pairs.get("city");
    s.country = pairs.get("country");
    s.contact = pairs.get("contact");
    s.notes = pairs.get("notes");
    s.active = pairs.flag("active");
    s.updated_at = now_iso();
    ctx.state
        .db
        .suppliers()
        .replace_one(doc! { "_id": &s.id, "company_id": &ctx.company.id }, &s)
        .await?;
    Ok(ctx.back("/suppliers", "msg_saved", "ok").into_response())
}

pub async fn supplier_delete(ctx: Ctx, Path(id): Path<String>) -> Result<Response, AppError> {
    ctx.state
        .db
        .suppliers()
        .delete_one(doc! { "_id": &id, "company_id": &ctx.company.id })
        .await?;
    Ok(ctx.back("/suppliers", "msg_deleted", "ok").into_response())
}

// ---------------------------------------------------------------------------
// Customers
// ---------------------------------------------------------------------------

pub async fn customers(ctx: Ctx, Query(q): Query<HashMap<String, String>>) -> Result<Response, AppError> {
    let search = q.get("q").cloned().unwrap_or_default();
    let filter = if search.trim().is_empty() {
        doc! { "company_id": &ctx.company.id }
    } else {
        let re = mongodb::bson::Regex {
            pattern: crate::pages::regex_escape(search.trim()),
            options: "i".into(),
        };
        doc! {
            "company_id": &ctx.company.id,
            "$or": [
                { "name": { "$regex": re.clone() } },
                { "code": { "$regex": re.clone() } },
                { "email": { "$regex": re } },
            ]
        }
    };
    let mut cur = ctx
        .state
        .db
        .customers()
        .find(filter)
        .sort(doc! { "name": 1 })
        .await?;
    let mut items = Vec::new();
    while let Some(c) = cur.try_next().await? {
        items.push(c);
    }

    let l = ctx.lang;
    let cur_code = ctx.company.currency.clone();
    let mut rows = Vec::new();
    for c in &items {
        let exposure = services::customer_exposure(&ctx.state.db, &ctx.company.id, &c.id).await?;
        let limit = if c.credit_limit > 0.0 {
            views::money_text(c.credit_limit, &cur_code)
        } else {
            tr(l, "m_unlimited")
        };
        rows.push(vec![
            format!(
                "<a href=\"/customers/{}\">{}</a><div class=\"muted small\">{}</div>",
                esc(&c.id),
                esc(&c.name),
                esc(&c.code)
            ),
            esc(&c.contact),
            esc(&c.email),
            format!("<span class=\"num\">{}</span>", esc(&limit)),
            format!(
                "<span class=\"num\">{}</span>",
                esc(&views::money_text(exposure, &cur_code))
            ),
            format!(
                "<div class=\"row-actions\">{}{}</div>",
                views::icon_link(&format!("/quotes/new?customer={}", c.id), "quote", &tr(l, "nav_quotes")),
                views::icon_post(&format!("/customers/{}/delete", c.id), "trash", &tr(l, "a_delete"))
            ),
        ]);
    }

    let body = format!(
        "{}{}",
        super::page_head(
            &ctx,
            "nav_customers",
            "",
            vec![
                super::search_form(l, "/customers", &search, ""),
                views::link_btn("/customers/new", &format!("+ {}", tr(l, "a_new"))),
            ]
        ),
        views::table_actions(
            l,
            &[
                &tr(l, "lbl_name"),
                &tr(l, "lbl_contact"),
                &tr(l, "lbl_email"),
                &tr(l, "lbl_credit_limit"),
                &tr(l, "lbl_exposure"),
                &tr(l, "lbl_actions"),
            ],
            rows,
            "msg_no_records",
        )
    );
    let title = views::Title::of(l, "nav_customers");
    Ok(views::respond(views::page(&ctx, &title, body)))
}

fn customer_fields(l: crate::i18n::Lang, c: &Customer) -> String {
    format!(
        "<div class=\"form-grid\">{}{}{}{}{}{}{}{}{}{}{}{}</div>",
        views::input(&tr(l, "lbl_code"), "code", &c.code, "text", true),
        views::input(&tr(l, "lbl_name"), "name", &c.name, "text", true),
        views::input(&tr(l, "lbl_legal_id"), "legal_id", &c.legal_id, "text", false),
        views::input(&tr(l, "lbl_tax_id"), "tax_id", &c.tax_id, "text", false),
        views::input(&tr(l, "lbl_contact"), "contact", &c.contact, "text", false),
        views::input(&tr(l, "lbl_email"), "email", &c.email, "email", false),
        views::input(&tr(l, "lbl_phone"), "phone", &c.phone, "text", false),
        views::input_step(&tr(l, "lbl_credit_limit"), "credit_limit", &format!("{:.2}", c.credit_limit), "0.01", false),
        views::input_step(&tr(l, "lbl_days"), "payment_terms", &c.payment_terms.to_string(), "1", false),
        views::input(&tr(l, "lbl_city"), "city", &c.city, "text", false),
        views::input(&tr(l, "lbl_country"), "country", &c.country, "text", false),
        views::checkbox(&tr(l, "lbl_active"), "active", c.active),
    )
}

pub async fn customer_new(ctx: Ctx) -> Result<Response, AppError> {
    let l = ctx.lang;
    let empty = Customer {
        active: true,
        payment_terms: 30,
        ..Default::default()
    };
    let body = format!(
        "{}<form method=\"post\" action=\"/customers\" class=\"card form\">{}<div class=\"form-grid\">{}</div>{}</form>",
        super::page_head(&ctx, "nav_customers", "", vec![]),
        customer_fields(l, &empty),
        views::textarea(&tr(l, "lbl_address"), "address", ""),
        views::form_actions(&tr(l, "a_save"), "/customers", &tr(l, "a_back")),
    );
    let title = views::Title::of(l, "nav_customers").join(l, "a_new");
    Ok(views::respond(views::page(&ctx, &title, body)))
}

fn customer_from_pairs(pairs: &Pairs, company_id: &str) -> Customer {
    let now = now_iso();
    Customer {
        id: crate::util::new_id(),
        company_id: company_id.to_string(),
        code: pairs.get("code"),
        name: pairs.get("name"),
        legal_id: pairs.get("legal_id"),
        tax_id: pairs.get("tax_id"),
        email: pairs.get("email"),
        phone: pairs.get("phone"),
        address: pairs.get("address"),
        city: pairs.get("city"),
        country: pairs.get("country"),
        contact: pairs.get("contact"),
        notes: pairs.get("notes"),
        credit_limit: pairs.num("credit_limit", 0.0),
        payment_terms: pairs.int("payment_terms", 30),
        active: pairs.flag("active"),
        created_at: now.clone(),
        updated_at: now,
    }
}

pub async fn customer_create(ctx: Ctx, pairs: Pairs) -> Result<Response, AppError> {
    let c = customer_from_pairs(&pairs, &ctx.company.id);
    if c.name.trim().is_empty() || c.code.trim().is_empty() {
        return Ok(ctx.back("/customers/new", "msg_required", "err").into_response());
    }
    match ctx.state.db.customers().insert_one(&c).await {
        Ok(_) => Ok(ctx.back("/customers", "msg_saved", "ok").into_response()),
        Err(e) if e.to_string().contains("duplicate key") => {
            Ok(ctx.back("/customers/new", "msg_duplicate", "err").into_response())
        }
        Err(e) => Err(e.into()),
    }
}

pub async fn customer_edit(ctx: Ctx, Path(id): Path<String>) -> Result<Response, AppError> {
    let l = ctx.lang;
    let c = services::get_by_id::<Customer>(&ctx.state.db.customers(), &ctx.company.id, &id).await?;
    let body = format!(
        "{}<form method=\"post\" action=\"/customers/{}\" class=\"card form\">{}<div class=\"form-grid\">{}</div>{}</form>",
        super::page_head(&ctx, "nav_customers", &format!("{} · {}", c.code, c.name), vec![]),
        esc(&c.id),
        customer_fields(l, &c),
        views::textarea(&tr(l, "lbl_address"), "address", &c.address),
        views::form_actions(&tr(l, "a_save"), "/customers", &tr(l, "a_back")),
    );
    let title = views::Title::of(l, "a_edit");
    Ok(views::respond(views::page(&ctx, &title, body)))
}

pub async fn customer_update(ctx: Ctx, Path(id): Path<String>, pairs: Pairs) -> Result<Response, AppError> {
    let mut c = services::get_by_id::<Customer>(&ctx.state.db.customers(), &ctx.company.id, &id).await?;
    c.code = pairs.get("code");
    c.name = pairs.get("name");
    c.legal_id = pairs.get("legal_id");
    c.tax_id = pairs.get("tax_id");
    c.email = pairs.get("email");
    c.phone = pairs.get("phone");
    c.address = pairs.get("address");
    c.city = pairs.get("city");
    c.country = pairs.get("country");
    c.contact = pairs.get("contact");
    c.notes = pairs.get("notes");
    c.credit_limit = pairs.num("credit_limit", c.credit_limit);
    c.payment_terms = pairs.int("payment_terms", c.payment_terms);
    c.active = pairs.flag("active");
    c.updated_at = now_iso();
    ctx.state
        .db
        .customers()
        .replace_one(doc! { "_id": &c.id, "company_id": &ctx.company.id }, &c)
        .await?;
    Ok(ctx.back("/customers", "msg_saved", "ok").into_response())
}

pub async fn customer_delete(ctx: Ctx, Path(id): Path<String>) -> Result<Response, AppError> {
    ctx.state
        .db
        .customers()
        .delete_one(doc! { "_id": &id, "company_id": &ctx.company.id })
        .await?;
    Ok(ctx.back("/customers", "msg_deleted", "ok").into_response())
}

/// Customer overview: credit position and invoice history.
pub async fn customer_detail(ctx: Ctx, Path(id): Path<String>) -> Result<Response, AppError> {
    let l = ctx.lang;
    let c = services::get_by_id::<Customer>(&ctx.state.db.customers(), &ctx.company.id, &id).await?;
    let credit = services::credit_position(&ctx.state.db, &ctx.company.id, &c).await?;
    let cur = ctx.company.currency.clone();

    let mut cur_inv = ctx
        .state
        .db
        .customer_invoices()
        .find(doc! { "company_id": &ctx.company.id, "party_id": &c.id })
        .sort(doc! { "invoice_date": -1 })
        .await?;
    let mut invoices: Vec<Invoice> = Vec::new();
    while let Some(inv) = cur_inv.try_next().await? {
        invoices.push(inv);
    }

    let rows = invoices
        .iter()
        .filter(|i| i.status != InvoiceStatus::Draft)
        .map(|inv| {
            vec![
                format!(
                    "<a href=\"/customer-invoices/{}\">{}</a>",
                    esc(&inv.id),
                    esc(&inv.number)
                ),
                esc(&inv.invoice_date),
                esc(&inv.due_date),
                views::status_badge(l, &inv.display_status().key()),
                format!(
                    "<span class=\"num\">{}</span>",
                    esc(&views::money_text(inv.outstanding(), &cur))
                ),
            ]
        })
        .collect::<Vec<_>>();

    let usage = if credit.unlimited() {
        tr(l, "m_unlimited")
    } else {
        format!(
            "{} / {}",
            views::money_text(credit.exposure, &cur),
            views::money_text(credit.limit, &cur)
        )
    };

    let meta = super::meta_grid(vec![
        (tr(l, "lbl_code"), esc(&c.code)),
        (tr(l, "lbl_email"), esc(&c.email)),
        (tr(l, "lbl_phone"), esc(&c.phone)),
        (tr(l, "lbl_contact"), esc(&c.contact)),
        (tr(l, "lbl_address"), esc(&format!("{} {}", c.address, c.city))),
        (tr(l, "lbl_credit_limit"), if credit.unlimited() { tr(l, "m_unlimited") } else { views::money_text(credit.limit, &cur) }),
        (tr(l, "lbl_exposure"), views::money_text(credit.exposure, &cur)),
        (tr(l, "m_credit_usage"), usage),
    ]);

    let body = format!(
        "{}{}{}{}",
        super::page_head(
            &ctx,
            "nav_customers",
            &c.name,
            vec![
                views::link_btn(&format!("/customers/{}/edit", c.id), &tr(l, "a_edit")),
                views::link_btn(&format!("/quotes/new?customer={}", c.id), &format!("+ {}", tr(l, "nav_quotes"))),
                views::link_btn(&format!("/customer-orders/new?customer={}", c.id), &format!("+ {}", tr(l, "nav_customer_orders"))),
            ]
        ),
        views::card(&tr(l, "lbl_customer"), meta),
        views::card(
            &tr(l, "nav_customer_invoices"),
            views::table(
                l,
                &[
                    &tr(l, "lbl_number"),
                    &tr(l, "lbl_date"),
                    &tr(l, "lbl_due_date"),
                    &tr(l, "lbl_status"),
                    &tr(l, "lbl_outstanding"),
                ],
                rows,
                "msg_no_records",
            )
        ),
        ""
    );
    let title = views::Title::raw(&c.name);
    Ok(views::respond(views::page(&ctx, &title, body)))
}
