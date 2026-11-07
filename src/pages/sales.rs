//! Quote-to-Cash: quotes, customer orders and delivery notes.

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
use crate::services;
use crate::util::{add_days, esc, qty, today};
use crate::views;

async fn customer_options(ctx: &Ctx, selected: &str) -> Result<Vec<(String, String, bool)>, AppError> {
    let mut cur = ctx
        .state
        .db
        .customers()
        .find(doc! { "company_id": &ctx.company.id, "active": true })
        .sort(doc! { "name": 1 })
        .await?;
    let mut out = Vec::new();
    while let Some(c) = cur.try_next().await? {
        out.push((c.id.clone(), c.name.clone(), c.id == selected));
    }
    Ok(out)
}

async fn product_list(ctx: &Ctx) -> Result<Vec<Product>, AppError> {
    let products = product_index(&ctx.state.db, &ctx.company.id).await?;
    let mut list: Vec<Product> = products.values().cloned().collect();
    list.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(list)
}

fn lines_template(ctx: &Ctx, list: &[Product], lines: &[DocLine], tax: f64) -> String {
    super::lines_editor(ctx, list, lines, tax, false)
}

// ---------------------------------------------------------------------------
// Quotes
// ---------------------------------------------------------------------------

pub async fn quote_list(ctx: Ctx, Query(q): Query<HashMap<String, String>>) -> Result<Response, AppError> {
    let search = q.get("q").cloned().unwrap_or_default();
    let mut cur = ctx
        .state
        .db
        .quotes()
        .find(super::list_filter(&ctx.company.id, &search, &["number", "customer_name"]))
        .sort(doc! { "created_at": -1 })
        .limit(300)
        .await?;
    let l = ctx.lang;
    let cur_code = ctx.company.currency.clone();
    let mut rows = Vec::new();
    while let Some(q) = cur.try_next().await? {
        rows.push(vec![
            format!("<a href=\"/quotes/{}\">{}</a>", esc(&q.id), esc(&q.number)),
            esc(&q.customer_name),
            esc(&q.quote_date),
            esc(&q.valid_until),
            views::status_badge(l, &q.status.key()),
            format!(
                "<span class=\"num\">{}</span>",
                esc(&views::money_text(q.totals.total, &cur_code))
            ),
        ]);
    }
    let body = format!(
        "{}{}",
        super::page_head(
            &ctx,
            "nav_quotes",
            "",
            vec![
                super::search_form(l, "/quotes", &search, ""),
                views::link_btn("/quotes/new", &format!("+ {}", tr(l, "a_new"))),
            ]
        ),
        views::table(
            l,
            &[
                &tr(l, "lbl_number"),
                &tr(l, "lbl_customer"),
                &tr(l, "lbl_date"),
                &tr(l, "lbl_valid_until"),
                &tr(l, "lbl_status"),
                &tr(l, "lbl_total"),
            ],
            rows,
            "msg_no_records",
        )
    );
    let title = views::Title::of(l, "nav_quotes");
    Ok(views::respond(views::page(&ctx, &title, body)))
}

pub async fn quote_new(ctx: Ctx, Query(q): Query<HashMap<String, String>>) -> Result<Response, AppError> {
    let l = ctx.lang;
    let customer_id = q.get("customer").cloned().unwrap_or_default();
    let customers = customer_options(&ctx, &customer_id).await?;
    let list = product_list(&ctx).await?;

    let body = format!(
        "{}<form method=\"post\" action=\"/quotes\" class=\"card form\">\
           <div class=\"form-grid\">{}{}{}{}</div>\
           <h3 class=\"section-title\">{}</h3>{}\
           {}{}</form>",
        super::page_head(&ctx, "nav_quotes", "", vec![]),
        views::select(&tr(l, "lbl_customer"), "customer_id", &customers, true),
        views::input(&tr(l, "lbl_date"), "quote_date", &today(), "date", true),
        views::input(&tr(l, "lbl_valid_until"), "valid_until", &add_days(&today(), 15), "date", false),
        views::input_step(&tr(l, "lbl_tax_rate"), "default_tax", "0", "0.01", false),
        tr(l, "lbl_lines"),
        lines_template(&ctx, &list, &[], 0.0),
        views::textarea(&tr(l, "lbl_notes"), "notes", ""),
        views::form_actions(&tr(l, "a_save"), "/quotes", &tr(l, "a_back")),
    );
    let title = views::Title::of(l, "nav_quotes").join(l, "a_new");
    Ok(views::respond(views::page(&ctx, &title, body)))
}

pub async fn quote_create(ctx: Ctx, pairs: Pairs) -> Result<Response, AppError> {
    let customer_id = pairs.get("customer_id");
    let customer = services::get_by_id::<Customer>(&ctx.state.db.customers(), &ctx.company.id, &customer_id)
        .await
        .map_err(|_| AppError::BadRequest("Select a customer".into()))?;
    let products = product_index(&ctx.state.db, &ctx.company.id).await?;
    let lines = parse_doc_lines(&pairs, &products, false, pairs.num("default_tax", 0.0));
    if lines.is_empty() {
        return Ok(ctx.back("/quotes/new", "msg_line_required", "err").into_response());
    }
    let totals = Totals::compute(&lines);
    let number = crate::db::next_number(&ctx.state.db.db, &ctx.company.id, "QUO").await?;
    let quote = Quote {
        id: crate::util::new_id(),
        company_id: ctx.company.id.clone(),
        number: number.clone(),
        customer_id: customer.id.clone(),
        customer_name: customer.name.clone(),
        quote_date: crate::util::if_empty(&pairs.get("quote_date"), &today()),
        valid_until: crate::util::if_empty(&pairs.get("valid_until"), &add_days(&today(), 15)),
        status: QuoteStatus::Draft,
        lines,
        totals,
        notes: pairs.get("notes"),
        converted_order_id: String::new(),
        converted_invoice_id: String::new(),
        created_at: crate::util::now_iso(),
        created_by: ctx.claims.un.clone(),
    };
    ctx.state.db.quotes().insert_one(&quote).await?;
    ctx.state
        .bus
        .publish(
            "quote.created",
            &new_event(
                "quote.created",
                &quote.company_id,
                &quote.id,
                &quote.number,
                quote.status.as_str(),
                quote.totals.total,
                &ctx.claims.un,
            ),
        )
        .await;
    Ok(axum::response::Redirect::to(&format!("/quotes/{}?m=msg_saved&k=ok", quote.id))
        .into_response())
}

pub async fn quote_detail(ctx: Ctx, Path(id): Path<String>) -> Result<Response, AppError> {
    let l = ctx.lang;
    let quote = services::get_by_id::<Quote>(&ctx.state.db.quotes(), &ctx.company.id, &id).await?;
    let cur_code = ctx.company.currency.clone();

    let mut actions = Vec::new();
    if quote.status == QuoteStatus::Draft {
        actions.push(views::post_btn(
            &format!("/quotes/{}/send", quote.id),
            &tr(l, "a_send"),
            "btn-primary",
        ));
    }
    if matches!(quote.status, QuoteStatus::Draft | QuoteStatus::Sent) {
        actions.push(views::post_btn(
            &format!("/quotes/{}/accept", quote.id),
            &tr(l, "a_accept"),
            "btn-primary",
        ));
        actions.push(views::post_btn(
            &format!("/quotes/{}/reject", quote.id),
            &tr(l, "a_cancel"),
            "btn-danger",
        ));
    }
    if !quote.converted_order_id.is_empty() {
        actions.push(views::link_btn(
            &format!("/customer-orders/{}", quote.converted_order_id),
            &tr(l, "nav_customer_orders"),
        ));
    }
    if !quote.converted_invoice_id.is_empty() {
        actions.push(views::link_btn(
            &format!("/customer-invoices/{}", quote.converted_invoice_id),
            &tr(l, "nav_customer_invoices"),
        ));
    }

    let meta = super::meta_grid(vec![
        (tr(l, "lbl_customer"), esc(&quote.customer_name)),
        (tr(l, "lbl_date"), esc(&quote.quote_date)),
        (tr(l, "lbl_valid_until"), esc(&quote.valid_until)),
        (tr(l, "lbl_status"), views::status_badge(l, &quote.status.key())),
        (tr(l, "lbl_created_by"), esc(&quote.created_by)),
    ]);

    let body = format!(
        "{}{}{}{}",
        super::page_head(&ctx, "nav_quotes", &quote.number, actions),
        views::card(&tr(l, "nav_quotes"), meta),
        views::card(&tr(l, "lbl_lines"), super::lines_view(&ctx, &quote.lines, &cur_code)),
        views::card(
            &tr(l, "m_totals"),
            super::totals_block(&ctx, &quote.totals, None, &cur_code)
        )
    );
    let title = views::Title::raw(&quote.number);
    Ok(views::respond(views::page(&ctx, &title, body)))
}

pub async fn quote_send(ctx: Ctx, Path(id): Path<String>) -> Result<Response, AppError> {
    let quote = services::get_by_id::<Quote>(&ctx.state.db.quotes(), &ctx.company.id, &id).await?;
    if quote.status != QuoteStatus::Draft {
        return Ok(ctx.back(&format!("/quotes/{id}"), "msg_cannot_post_twice", "err").into_response());
    }
    ctx.state
        .db
        .quotes()
        .update_one(
            doc! { "_id": &quote.id },
            doc! { "$set": { "status": QuoteStatus::Sent.as_str() } },
        )
        .await?;
    ctx.state
        .bus
        .publish(
            "quote.sent",
            &new_event(
                "quote.sent",
                &quote.company_id,
                &quote.id,
                &quote.number,
                QuoteStatus::Sent.as_str(),
                quote.totals.total,
                &ctx.claims.un,
            ),
        )
        .await;
    Ok(ctx.back(&format!("/quotes/{id}"), "msg_sent", "ok").into_response())
}

/// Validate a quote: create the confirmed order *and* the open invoice.
pub async fn quote_accept(ctx: Ctx, Path(id): Path<String>) -> Result<Response, AppError> {
    let mut quote = services::get_by_id::<Quote>(&ctx.state.db.quotes(), &ctx.company.id, &id).await?;
    if !quote.converted_order_id.is_empty() {
        return Ok(ctx.back(&format!("/quotes/{id}"), "msg_already_converted", "err").into_response());
    }
    let customer = services::get_by_id::<Customer>(&ctx.state.db.customers(), &ctx.company.id, &quote.customer_id).await?;
    let credit = services::credit_position(&ctx.state.db, &ctx.company.id, &customer).await?;
    if !credit.allows(quote.totals.total) {
        return Ok(ctx.back(&format!("/quotes/{id}"), "msg_credit_exceeded", "err").into_response());
    }

    let order_number = crate::db::next_number(&ctx.state.db.db, &ctx.company.id, "SO").await?;
    let order = SalesOrder {
        id: crate::util::new_id(),
        company_id: ctx.company.id.clone(),
        number: order_number,
        customer_id: quote.customer_id.clone(),
        customer_name: quote.customer_name.clone(),
        quote_id: quote.id.clone(),
        order_date: today(),
        status: OrderStatus::Confirmed,
        lines: quote.lines.clone(),
        totals: quote.totals.clone(),
        notes: quote.notes.clone(),
        delivery_id: String::new(),
        invoice_id: String::new(),
        created_at: crate::util::now_iso(),
        created_by: ctx.claims.un.clone(),
    };
    ctx.state.db.sales_orders().insert_one(&order).await?;

    let invoice = super::invoicing::new_invoice_record(
        &ctx,
        &customer,
        quote.lines.clone(),
        order.id.clone(),
        order.number.clone(),
        InvoiceStatus::Open,
        quote.notes.clone(),
    )
    .await?;

    quote.status = QuoteStatus::Converted;
    quote.converted_order_id = order.id.clone();
    quote.converted_invoice_id = invoice.id.clone();
    ctx.state
        .db
        .quotes()
        .update_one(
            doc! { "_id": &quote.id },
            doc! {
                "$set": {
                    "status": quote.status.as_str(),
                    "converted_order_id": &order.id,
                    "converted_invoice_id": &invoice.id,
                }
            },
        )
        .await?;

    ctx.state
        .db
        .sales_orders()
        .update_one(
            doc! { "_id": &order.id },
            doc! { "$set": { "invoice_id": &invoice.id } },
        )
        .await?;

    ctx.state
        .bus
        .publish(
            "quote.accepted",
            &new_event(
                "quote.accepted",
                &quote.company_id,
                &quote.id,
                &quote.number,
                quote.status.as_str(),
                quote.totals.total,
                &ctx.claims.un,
            ),
        )
        .await;
    ctx.state
        .bus
        .publish(
            "customer_invoice.issued",
            &new_event(
                "customer_invoice.issued",
                &invoice.company_id,
                &invoice.id,
                &invoice.number,
                invoice.status.as_str(),
                invoice.totals.total,
                &ctx.claims.un,
            ),
        )
        .await;

    Ok(ctx.back(&format!("/quotes/{id}"), "msg_quote_converted", "ok").into_response())
}

pub async fn quote_reject(ctx: Ctx, Path(id): Path<String>) -> Result<Response, AppError> {
    let quote = services::get_by_id::<Quote>(&ctx.state.db.quotes(), &ctx.company.id, &id).await?;
    ctx.state
        .db
        .quotes()
        .update_one(
            doc! { "_id": &quote.id },
            doc! { "$set": { "status": QuoteStatus::Rejected.as_str() } },
        )
        .await?;
    Ok(ctx.back(&format!("/quotes/{id}"), "msg_saved", "ok").into_response())
}

// ---------------------------------------------------------------------------
// Customer orders
// ---------------------------------------------------------------------------

pub async fn order_list(ctx: Ctx, Query(q): Query<HashMap<String, String>>) -> Result<Response, AppError> {
    let search = q.get("q").cloned().unwrap_or_default();
    let mut cur = ctx
        .state
        .db
        .sales_orders()
        .find(super::list_filter(&ctx.company.id, &search, &["number", "customer_name"]))
        .sort(doc! { "created_at": -1 })
        .limit(300)
        .await?;
    let l = ctx.lang;
    let cur_code = ctx.company.currency.clone();
    let mut rows = Vec::new();
    while let Some(o) = cur.try_next().await? {
        rows.push(vec![
            format!(
                "<a href=\"/customer-orders/{}\">{}</a>",
                esc(&o.id),
                esc(&o.number)
            ),
            esc(&o.customer_name),
            esc(&o.order_date),
            views::status_badge(l, &o.status.key()),
            format!(
                "<span class=\"num\">{}</span>",
                esc(&views::money_text(o.totals.total, &cur_code))
            ),
            format!(
                "<div class=\"row-actions\">{}{}</div>",
                if matches!(o.status, OrderStatus::Confirmed) {
                    views::icon_link(&format!("/delivery-notes/new?order={}", o.id), "delivery", &tr(l, "a_deliver"))
                } else {
                    String::new()
                },
                if matches!(o.status, OrderStatus::Confirmed | OrderStatus::Delivered) && o.invoice_id.is_empty() {
                    views::icon_link(&format!("/customer-invoices/new?order={}", o.id), "bill", &tr(l, "a_invoice"))
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
            "nav_customer_orders",
            "",
            vec![
                super::search_form(l, "/customer-orders", &search, ""),
                views::link_btn("/customer-orders/new", &format!("+ {}", tr(l, "a_new"))),
            ]
        ),
        views::table_actions(
            l,
            &[
                &tr(l, "lbl_number"),
                &tr(l, "lbl_customer"),
                &tr(l, "lbl_date"),
                &tr(l, "lbl_status"),
                &tr(l, "lbl_total"),
                &tr(l, "lbl_actions"),
            ],
            rows,
            "msg_no_records",
        )
    );
    let title = views::Title::of(l, "nav_customer_orders");
    Ok(views::respond(views::page(&ctx, &title, body)))
}

pub async fn order_new(ctx: Ctx, Query(q): Query<HashMap<String, String>>) -> Result<Response, AppError> {
    let l = ctx.lang;
    let quote_id = q.get("quote").cloned().unwrap_or_default();
    let customer_id = q.get("customer").cloned().unwrap_or_default();
    let list = product_list(&ctx).await?;

    let mut lines: Vec<DocLine> = Vec::new();
    let mut selected_customer = customer_id;
    let mut note = String::new();
    if !quote_id.is_empty() {
        let quote = services::get_by_id::<Quote>(&ctx.state.db.quotes(), &ctx.company.id, &quote_id).await?;
        lines = quote.lines.clone();
        selected_customer = quote.customer_id.clone();
        note = quote.notes.clone();
    }
    let customers = customer_options(&ctx, &selected_customer).await?;

    let body = format!(
        "{}<form method=\"post\" action=\"/customer-orders\" class=\"card form\">\
           <div class=\"form-grid\">{}{}{}<input type=\"hidden\" name=\"quote_id\" value=\"{}\"/></div>\
           <h3 class=\"section-title\">{}</h3>{}\
           {}{}</form>",
        super::page_head(&ctx, "nav_customer_orders", "", vec![]),
        views::select(&tr(l, "lbl_customer"), "customer_id", &customers, true),
        views::input(&tr(l, "lbl_order_date"), "order_date", &today(), "date", true),
        views::input_step(&tr(l, "lbl_tax_rate"), "default_tax", "0", "0.01", false),
        esc(&quote_id),
        tr(l, "lbl_lines"),
        lines_template(&ctx, &list, &lines, 0.0),
        views::textarea(&tr(l, "lbl_notes"), "notes", &note),
        views::form_actions(&tr(l, "a_save"), "/customer-orders", &tr(l, "a_back")),
    );
    let title = views::Title::of(l, "nav_customer_orders").join(l, "a_new");
    Ok(views::respond(views::page(&ctx, &title, body)))
}

pub async fn order_create(ctx: Ctx, pairs: Pairs) -> Result<Response, AppError> {
    let customer_id = pairs.get("customer_id");
    let customer = services::get_by_id::<Customer>(&ctx.state.db.customers(), &ctx.company.id, &customer_id)
        .await
        .map_err(|_| AppError::BadRequest("Select a customer".into()))?;
    let products = product_index(&ctx.state.db, &ctx.company.id).await?;
    let lines = parse_doc_lines(&pairs, &products, false, pairs.num("default_tax", 0.0));
    if lines.is_empty() {
        return Ok(ctx.back("/customer-orders/new", "msg_line_required", "err").into_response());
    }
    let totals = Totals::compute(&lines);
    let credit = services::credit_position(&ctx.state.db, &ctx.company.id, &customer).await?;
    if !credit.allows(totals.total) {
        return Ok(ctx.back("/customer-orders/new", "msg_credit_exceeded", "err").into_response());
    }
    let number = crate::db::next_number(&ctx.state.db.db, &ctx.company.id, "SO").await?;
    let order = SalesOrder {
        id: crate::util::new_id(),
        company_id: ctx.company.id.clone(),
        number: number.clone(),
        customer_id: customer.id.clone(),
        customer_name: customer.name.clone(),
        quote_id: pairs.get("quote_id"),
        order_date: crate::util::if_empty(&pairs.get("order_date"), &today()),
        status: OrderStatus::Confirmed,
        lines,
        totals,
        notes: pairs.get("notes"),
        delivery_id: String::new(),
        invoice_id: String::new(),
        created_at: crate::util::now_iso(),
        created_by: ctx.claims.un.clone(),
    };
    ctx.state.db.sales_orders().insert_one(&order).await?;
    ctx.state
        .bus
        .publish(
            "order.confirmed",
            &new_event(
                "sales_order.confirmed",
                &order.company_id,
                &order.id,
                &order.number,
                order.status.as_str(),
                order.totals.total,
                &ctx.claims.un,
            ),
        )
        .await;
    Ok(axum::response::Redirect::to(&format!("/customer-orders/{}?m=msg_saved&k=ok", order.id))
        .into_response())
}

pub async fn order_detail(ctx: Ctx, Path(id): Path<String>) -> Result<Response, AppError> {
    let l = ctx.lang;
    let order = services::get_by_id::<SalesOrder>(&ctx.state.db.sales_orders(), &ctx.company.id, &id).await?;
    let cur_code = ctx.company.currency.clone();

    let mut actions: Vec<String> = Vec::new();
    if order.status == OrderStatus::Draft {
        actions.push(views::post_btn(
            &format!("/customer-orders/{}/confirm", order.id),
            &tr(l, "a_accept"),
            "btn-primary",
        ));
    }
    if order.status == OrderStatus::Confirmed {
        actions.push(views::link_btn(
            &format!("/delivery-notes/new?order={}", order.id),
            &tr(l, "a_deliver"),
        ));
    }
    if order.invoice_id.is_empty() && order.status != OrderStatus::Cancelled {
        actions.push(views::link_btn(
            &format!("/customer-invoices/new?order={}", order.id),
            &tr(l, "a_invoice"),
        ));
    }
    if !order.invoice_id.is_empty() {
        actions.push(views::link_btn(
            &format!("/customer-invoices/{}", order.invoice_id),
            &tr(l, "nav_customer_invoices"),
        ));
    }

    let meta = super::meta_grid(vec![
        (tr(l, "lbl_customer"), esc(&order.customer_name)),
        (tr(l, "lbl_date"), esc(&order.order_date)),
        (tr(l, "lbl_status"), views::status_badge(l, &order.status.key())),
        (tr(l, "lbl_created_by"), esc(&order.created_by)),
        (tr(l, "lbl_source"), esc(&order.quote_id)),
        (tr(l, "nav_delivery_notes"), esc(&order.delivery_id)),
    ]);

    let body = format!(
        "{}{}{}{}",
        super::page_head(&ctx, "nav_customer_orders", &order.number, actions),
        views::card(&tr(l, "nav_customer_orders"), meta),
        views::card(&tr(l, "lbl_lines"), super::lines_view(&ctx, &order.lines, &cur_code)),
        views::card(
            &tr(l, "m_totals"),
            super::totals_block(&ctx, &order.totals, None, &cur_code)
        )
    );
    let title = views::Title::raw(&order.number);
    Ok(views::respond(views::page(&ctx, &title, body)))
}

pub async fn order_confirm(ctx: Ctx, Path(id): Path<String>) -> Result<Response, AppError> {
    let order = services::get_by_id::<SalesOrder>(&ctx.state.db.sales_orders(), &ctx.company.id, &id).await?;
    let customer = services::get_by_id::<Customer>(&ctx.state.db.customers(), &ctx.company.id, &order.customer_id).await?;
    let credit = services::credit_position(&ctx.state.db, &ctx.company.id, &customer).await?;
    if !credit.allows(order.totals.total) {
        return Ok(ctx.back(&format!("/customer-orders/{id}"), "msg_credit_exceeded", "err").into_response());
    }
    ctx.state
        .db
        .sales_orders()
        .update_one(
            doc! { "_id": &order.id },
            doc! { "$set": { "status": OrderStatus::Confirmed.as_str() } },
        )
        .await?;
    ctx.state
        .bus
        .publish(
            "order.confirmed",
            &new_event(
                "sales_order.confirmed",
                &order.company_id,
                &order.id,
                &order.number,
                OrderStatus::Confirmed.as_str(),
                order.totals.total,
                &ctx.claims.un,
            ),
        )
        .await;
    Ok(ctx.back(&format!("/customer-orders/{id}"), "msg_saved", "ok").into_response())
}

// ---------------------------------------------------------------------------
// Delivery notes
// ---------------------------------------------------------------------------

pub async fn delivery_list(ctx: Ctx, Query(q): Query<HashMap<String, String>>) -> Result<Response, AppError> {
    let search = q.get("q").cloned().unwrap_or_default();
    let mut cur = ctx
        .state
        .db
        .delivery_notes()
        .find(super::list_filter(&ctx.company.id, &search, &["number", "customer_name", "order_number"]))
        .sort(doc! { "created_at": -1 })
        .limit(300)
        .await?;
    let l = ctx.lang;
    let mut rows = Vec::new();
    while let Some(d) = cur.try_next().await? {
        rows.push(vec![
            format!(
                "<a href=\"/delivery-notes/{}\">{}</a>",
                esc(&d.id),
                esc(&d.number)
            ),
            esc(&d.customer_name),
            esc(&d.order_number),
            esc(&d.delivery_date),
            views::status_badge(l, &d.status.key()),
        ]);
    }
    let body = format!(
        "{}{}",
        super::page_head(
            &ctx,
            "nav_delivery_notes",
            "",
            vec![
                super::search_form(l, "/delivery-notes", &search, ""),
                views::link_btn("/delivery-notes/new", &format!("+ {}", tr(l, "a_new"))),
            ]
        ),
        views::table(
            l,
            &[
                &tr(l, "lbl_number"),
                &tr(l, "lbl_customer"),
                &tr(l, "nav_customer_orders"),
                &tr(l, "lbl_date"),
                &tr(l, "lbl_status"),
            ],
            rows,
            "msg_no_records",
        )
    );
    let title = views::Title::of(l, "nav_delivery_notes");
    Ok(views::respond(views::page(&ctx, &title, body)))
}

pub async fn delivery_new(ctx: Ctx, Query(q): Query<HashMap<String, String>>) -> Result<Response, AppError> {
    let l = ctx.lang;
    let order_id = q.get("order").cloned().unwrap_or_default();
    let products = product_index(&ctx.state.db, &ctx.company.id).await?;

    let mut order_number = String::new();
    let mut customer_name = String::new();
    let mut rows = String::new();
    if !order_id.is_empty() {
        let order = services::get_by_id::<SalesOrder>(&ctx.state.db.sales_orders(), &ctx.company.id, &order_id).await?;
        order_number = order.number.clone();
        customer_name = order.customer_name.clone();
        for line in &order.lines {
            rows.push_str(&format!(
                "<tr class=\"line-row\"><td><select name=\"item_product\">{}</select></td>\
                 <td><input name=\"item_desc\" value=\"{}\"/></td>\
                 <td><input name=\"item_qty\" class=\"num\" type=\"number\" step=\"0.01\" value=\"{}\"/></td>\
                 <td><button type=\"button\" class=\"btn-icon remove-line\">×</button></td></tr>",
                super::catalog_select(&products, &line.product_id),
                esc(&line.description),
                qty(line.qty)
            ));
        }
    }
    if rows.is_empty() {
        rows.push_str(&format!(
            "<tr class=\"line-row\"><td><select name=\"item_product\">{}</select></td>\
             <td><input name=\"item_desc\" value=\"\"/></td>\
             <td><input name=\"item_qty\" class=\"num\" type=\"number\" step=\"0.01\" value=\"1\"/></td>\
             <td><button type=\"button\" class=\"btn-icon remove-line\">×</button></td></tr>",
            super::catalog_select(&products, "")
        ));
    }

    let orders = {
        let mut cur = ctx
            .state
            .db
            .sales_orders()
            .find(doc! { "company_id": &ctx.company.id, "status": { "$in": [OrderStatus::Confirmed.as_str(), OrderStatus::Delivered.as_str()] } })
            .sort(doc! { "created_at": -1 })
            .await?;
        let mut out = vec![(String::new(), tr(l, "lbl_none"), order_id.is_empty())];
        while let Some(o) = cur.try_next().await? {
            out.push((o.id.clone(), format!("{} · {}", o.number, o.customer_name), o.id == order_id));
        }
        out
    };

    let template = format!(
        "<tr class=\"line-row\"><td><select name=\"item_product\">{}</select></td>\
         <td><input name=\"item_desc\" value=\"\"/></td>\
         <td><input name=\"item_qty\" class=\"num\" type=\"number\" step=\"0.01\" value=\"1\"/></td>\
         <td><button type=\"button\" class=\"btn-icon remove-line\">×</button></td></tr>",
        super::catalog_select(&products, "")
    );

    let body = format!(
        "{}<form method=\"post\" action=\"/delivery-notes\" class=\"card form\">\
           <div class=\"form-grid\">{}{}{}</div>\
           <div class=\"lines-wrap\"><table class=\"lines-table\"><thead><tr>\
             <th>{}</th><th>{}</th><th class=\"num\">{}</th><th></th>\
           </tr></thead><tbody id=\"lines-body\">{rows}</tbody></table>\
           <template id=\"line-tpl\">{template}</template>\
           <div class=\"lines-foot\"><button type=\"button\" class=\"btn btn-sm\" id=\"add-line\">+ {}</button></div></div>\
           {}{}</form>",
        super::page_head(&ctx, "nav_delivery_notes", &format!("{order_number} {customer_name}"), vec![]),
        views::select(&tr(l, "nav_customer_orders"), "order_id", &orders, false),
        views::input(&tr(l, "lbl_delivery_date"), "delivery_date", &today(), "date", true),
        format!("<input type=\"hidden\" name=\"customer_name\" value=\"{}\"/>", esc(&customer_name)),
        tr(l, "lbl_product"),
        tr(l, "lbl_description"),
        tr(l, "lbl_quantity"),
        tr(l, "a_add_line"),
        views::textarea(&tr(l, "lbl_notes"), "notes", ""),
        views::form_actions(&tr(l, "a_save"), "/delivery-notes", &tr(l, "a_back")),
    );
    let title = views::Title::of(l, "nav_delivery_notes").join(l, "a_new");
    Ok(views::respond(views::page(&ctx, &title, body)))
}

pub async fn delivery_create(ctx: Ctx, pairs: Pairs) -> Result<Response, AppError> {
    let order_id = pairs.get("order_id");
    let (order_number, customer_id, customer_name) = if order_id.is_empty() {
        (
            String::new(),
            String::new(),
            pairs.get("customer_name"),
        )
    } else {
        let order = services::get_by_id::<SalesOrder>(&ctx.state.db.sales_orders(), &ctx.company.id, &order_id).await?;
        (order.number.clone(), order.customer_id.clone(), order.customer_name.clone())
    };

    let products = product_index(&ctx.state.db, &ctx.company.id).await?;
    let ids = pairs.all("item_product");
    let descs = pairs.all("item_desc");
    let qtys = pairs.all("item_qty");
    let mut lines = Vec::new();
    for i in 0..ids.len().max(descs.len()) {
        let product_id = ids.get(i).cloned().unwrap_or_default();
        let mut description = descs.get(i).cloned().unwrap_or_default();
        let q = qtys
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
        if q <= 0.0 {
            continue;
        }
        lines.push(DeliveryLine {
            product_id,
            description,
            qty: q,
        });
    }
    if lines.is_empty() {
        return Ok(ctx.back("/delivery-notes/new", "msg_line_required", "err").into_response());
    }

    let number = crate::db::next_number(&ctx.state.db.db, &ctx.company.id, "DN").await?;
    let dn = DeliveryNote {
        id: crate::util::new_id(),
        company_id: ctx.company.id.clone(),
        number,
        order_id,
        order_number,
        customer_id,
        customer_name,
        delivery_date: crate::util::if_empty(&pairs.get("delivery_date"), &today()),
        status: DeliveryStatus::Draft,
        lines,
        notes: pairs.get("notes"),
        created_at: crate::util::now_iso(),
        created_by: ctx.claims.un.clone(),
        posted_at: String::new(),
    };
    ctx.state.db.delivery_notes().insert_one(&dn).await?;
    Ok(axum::response::Redirect::to(&format!("/delivery-notes/{}?m=msg_saved&k=ok", dn.id))
        .into_response())
}

pub async fn delivery_detail(ctx: Ctx, Path(id): Path<String>) -> Result<Response, AppError> {
    let l = ctx.lang;
    let dn = services::get_by_id::<DeliveryNote>(&ctx.state.db.delivery_notes(), &ctx.company.id, &id).await?;
    let mut actions = Vec::new();
    if dn.status == DeliveryStatus::Draft {
        actions.push(views::post_btn(
            &format!("/delivery-notes/{}/post", dn.id),
            &tr(l, "a_post"),
            "btn-primary",
        ));
    }
    if !dn.order_id.is_empty() {
        actions.push(views::link_btn(
            &format!("/customer-orders/{}", dn.order_id),
            &tr(l, "nav_customer_orders"),
        ));
    }

    let rows = dn
        .lines
        .iter()
        .map(|line| {
            vec![
                esc(&line.description),
                format!("<span class=\"num\">{}</span>", qty(line.qty)),
            ]
        })
        .collect::<Vec<_>>();

    let meta = super::meta_grid(vec![
        (tr(l, "nav_customer_orders"), esc(&dn.order_number)),
        (tr(l, "lbl_customer"), esc(&dn.customer_name)),
        (tr(l, "lbl_delivery_date"), esc(&dn.delivery_date)),
        (tr(l, "lbl_status"), views::status_badge(l, &dn.status.key())),
        (tr(l, "lbl_created_by"), esc(&dn.created_by)),
    ]);

    let body = format!(
        "{}{}{}",
        super::page_head(&ctx, "nav_delivery_notes", &dn.number, actions),
        views::card(&tr(l, "nav_delivery_notes"), meta),
        views::card(
            &tr(l, "lbl_lines"),
            views::table(
                l,
                &[&tr(l, "lbl_description"), &tr(l, "lbl_quantity")],
                rows,
                "msg_no_records"
            )
        )
    );
    let title = views::Title::raw(&dn.number);
    Ok(views::respond(views::page(&ctx, &title, body)))
}

pub async fn delivery_post(ctx: Ctx, Path(id): Path<String>) -> Result<Response, AppError> {
    let mut dn = services::get_by_id::<DeliveryNote>(&ctx.state.db.delivery_notes(), &ctx.company.id, &id).await?;
    if dn.status == DeliveryStatus::Posted {
        return Ok(ctx.back(&format!("/delivery-notes/{id}"), "msg_cannot_post_twice", "err").into_response());
    }
    services::post_delivery_note(&ctx.state.db, &mut dn, &ctx.claims.un).await?;
    ctx.state
        .bus
        .publish(
            "delivery.posted",
            &new_event(
                "delivery_note.posted",
                &dn.company_id,
                &dn.id,
                &dn.number,
                dn.status.as_str(),
                0.0,
                &ctx.claims.un,
            ),
        )
        .await;
    Ok(ctx.back(&format!("/delivery-notes/{id}"), "msg_posted", "ok").into_response())
}
