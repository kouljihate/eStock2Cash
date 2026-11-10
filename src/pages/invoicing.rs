//! Customer invoices, payments (both directions) and the credit workspace.

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
use crate::services::{self, InvoiceKind};
use crate::util::{add_days, esc, now_iso, today};
use crate::views;

/// Create an invoice document (used by manual creation and by quote validation).
pub async fn new_invoice_record(
    ctx: &Ctx,
    customer: &Customer,
    lines: Vec<DocLine>,
    source_id: String,
    source_number: String,
    status: InvoiceStatus,
    notes: String,
) -> Result<Invoice, AppError> {
    let totals = Totals::compute(&lines);
    let number = crate::db::next_number(&ctx.state.db.db(), &ctx.company.id, "CI").await?;
    let inv = Invoice {
        id: crate::util::new_id(),
        company_id: ctx.company.id.clone(),
        number,
        party_id: customer.id.clone(),
        party_name: customer.name.clone(),
        source_id,
        source_number,
        invoice_date: today(),
        due_date: add_days(&today(), customer.payment_terms.max(0)),
        lines,
        totals,
        status,
        paid_amount: 0.0,
        notes,
        created_at: now_iso(),
        created_by: ctx.claims.un.clone(),
    };
    ctx.state.db.customer_invoices().insert_one(&inv).await?;
    Ok(inv)
}

pub async fn ci_list(ctx: Ctx, Query(q): Query<HashMap<String, String>>) -> Result<Response, AppError> {
    let search = q.get("q").cloned().unwrap_or_default();
    let mut cur = ctx
        .state
        .db
        .customer_invoices()
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
                "<a href=\"/customer-invoices/{}\">{}</a>",
                esc(&inv.id),
                esc(&inv.number)
            ),
            esc(&inv.party_name),
            esc(&inv.invoice_date),
            esc(&inv.due_date),
            views::status_badge(l, &inv.display_status().key()),
            format!(
                "<span class=\"num\">{}</span>",
                esc(&views::money_text(inv.totals.total, &cur_code))
            ),
            format!(
                "<span class=\"num\">{}</span>",
                esc(&views::money_text(inv.outstanding(), &cur_code))
            ),
            format!(
                "<div class=\"row-actions\">{}{}</div>",
                views::icon_link(&format!("/customer-invoices/{}/pdf", inv.id), "file", &tr(l, "a_pdf")),
                if inv.display_status().is_payable() {
                    views::icon_link(&format!("/payments/new?invoice={}&kind=customer", inv.id), "wallet", &tr(l, "a_pay"))
                } else {
                    String::new()
                }
            ),
        ]);
    }
    let tools = format!(
        "{}{}",
        super::search_form(l, "/customer-invoices", &search, ""),
        views::link_btn("/customer-invoices/new", &format!("+ {}", tr(l, "a_new"))),
    );
    let body = views::list_table_actions(
        l,
        &[
            &tr(l, "lbl_number"),
            &tr(l, "lbl_customer"),
            &tr(l, "lbl_date"),
            &tr(l, "lbl_due_date"),
            &tr(l, "lbl_status"),
            &tr(l, "lbl_total"),
            &tr(l, "lbl_outstanding"),
            &tr(l, "lbl_actions"),
        ],
        rows,
        "msg_no_records",
        &tools,
    );
    let title = views::Title::of(l, "nav_customer_invoices");
    Ok(views::respond(views::page(&ctx, &title, body)))
}

pub async fn ci_new(ctx: Ctx, Query(q): Query<HashMap<String, String>>) -> Result<Response, AppError> {
    let l = ctx.lang;
    let order_id = q.get("order").cloned().unwrap_or_default();
    let customer_id = q.get("customer").cloned().unwrap_or_default();
    let products = product_index(&ctx.state.db, &ctx.company.id).await?;
    let mut list: Vec<Product> = products.values().cloned().collect();
    list.sort_by(|a, b| a.name.cmp(&b.name));

    let mut lines: Vec<DocLine> = Vec::new();
    let mut source_id = String::new();
    let mut source_number = String::new();
    let mut selected_customer = customer_id;
    if !order_id.is_empty() {
        let order = services::get_by_id::<SalesOrder>(&ctx.state.db.sales_orders(), &ctx.company.id, &order_id).await?;
        lines = order.lines.clone();
        selected_customer = order.customer_id.clone();
        source_id = order.id.clone();
        source_number = order.number.clone();
    }
    let customers = {
        let mut cur = ctx
            .state
            .db
            .customers()
            .find(doc! { "company_id": &ctx.company.id, "active": true })
            .sort(doc! { "name": 1 })
            .await?;
        let mut out = Vec::new();
        while let Some(c) = cur.try_next().await? {
            out.push((c.id.clone(), c.name.clone(), c.id == selected_customer));
        }
        out
    };

    let body = format!(
        "{}<form method=\"post\" action=\"/customer-invoices\" class=\"card form\">\
           <div class=\"form-grid\">{}{}{}{}<input type=\"hidden\" name=\"source_id\" value=\"{}\"/></div>\
           <h3 class=\"section-title\">{}</h3>{}\
           {}{}</form>",
        super::page_head(&ctx, "nav_customer_invoices", &source_number, vec![]),
        views::select(&tr(l, "lbl_customer"), "customer_id", &customers, true),
        views::input(&tr(l, "lbl_invoice_date"), "invoice_date", &today(), "date", true),
        views::input(&tr(l, "lbl_due_date"), "due_date", &add_days(&today(), 30), "date", true),
        views::input_step(&tr(l, "lbl_tax_rate"), "default_tax", "0", "0.01", false),
        esc(&source_id),
        tr(l, "lbl_lines"),
        super::lines_editor(&ctx, &list, &lines, 0.0, false),
        views::textarea(&tr(l, "lbl_notes"), "notes", ""),
        views::form_actions(&tr(l, "a_save"), "/customer-invoices", &tr(l, "a_back")),
    );
    let title = views::Title::of(l, "nav_customer_invoices").join(l, "a_new");
    Ok(views::respond(views::page(&ctx, &title, body)))
}

pub async fn ci_create(ctx: Ctx, pairs: Pairs) -> Result<Response, AppError> {
    let customer_id = pairs.get("customer_id");
    let customer = services::get_by_id::<Customer>(&ctx.state.db.customers(), &ctx.company.id, &customer_id)
        .await
        .map_err(|_| AppError::BadRequest("Select a customer".into()))?;
    let products = product_index(&ctx.state.db, &ctx.company.id).await?;
    let lines = parse_doc_lines(&pairs, &products, false, pairs.num("default_tax", 0.0));
    if lines.is_empty() {
        return Ok(ctx.back("/customer-invoices/new", "msg_line_required", "err").into_response());
    }
    let totals = Totals::compute(&lines);
    let credit = services::credit_position(&ctx.state.db, &ctx.company.id, &customer).await?;
    if !credit.allows(totals.total) {
        return Ok(ctx.back("/customer-invoices/new", "msg_credit_exceeded", "err").into_response());
    }

    let source_id = pairs.get("source_id");
    let source_number = if source_id.is_empty() {
        String::new()
    } else {
        ctx.state
            .db
            .sales_orders()
            .find_one(doc! { "_id": &source_id })
            .await?
            .map(|o| o.number)
            .unwrap_or_default()
    };

    let number = crate::db::next_number(&ctx.state.db.db(), &ctx.company.id, "CI").await?;
    let inv = Invoice {
        id: crate::util::new_id(),
        company_id: ctx.company.id.clone(),
        number,
        party_id: customer.id.clone(),
        party_name: customer.name.clone(),
        source_id: source_id.clone(),
        source_number,
        invoice_date: crate::util::if_empty(&pairs.get("invoice_date"), &today()),
        due_date: crate::util::if_empty(&pairs.get("due_date"), &add_days(&today(), 30)),
        lines,
        totals,
        status: InvoiceStatus::Open,
        paid_amount: 0.0,
        notes: pairs.get("notes"),
        created_at: now_iso(),
        created_by: ctx.claims.un.clone(),
    };
    ctx.state.db.customer_invoices().insert_one(&inv).await?;
    if !source_id.is_empty() {
        ctx.state
            .db
            .sales_orders()
            .update_one(
                doc! { "_id": &source_id, "company_id": &ctx.company.id },
                doc! { "$set": { "status": OrderStatus::Invoiced.as_str(), "invoice_id": &inv.id } },
            )
            .await?;
    }
    ctx.state
        .bus
        .publish(
            "customer_invoice.issued",
            &new_event(
                "customer_invoice.issued",
                &inv.company_id,
                &inv.id,
                &inv.number,
                inv.status.as_str(),
                inv.totals.total,
                &ctx.claims.un,
            ),
        )
        .await;
    Ok(axum::response::Redirect::to(&format!("/customer-invoices/{}?m=msg_saved&k=ok", inv.id))
        .into_response())
}

pub async fn ci_detail(ctx: Ctx, Path(id): Path<String>) -> Result<Response, AppError> {
    let l = ctx.lang;
    let inv = services::get_by_id::<Invoice>(&ctx.state.db.customer_invoices(), &ctx.company.id, &id).await?;
    let cur_code = ctx.company.currency.clone();

    let mut cur = ctx
        .state
        .db
        .payments()
        .find(doc! { "company_id": &ctx.company.id, "invoice_id": &inv.id })
        .sort(doc! { "created_at": -1 })
        .await?;
    let mut pay_rows = Vec::new();
    while let Some(p) = cur.try_next().await? {
        pay_rows.push(vec![
            esc(&p.number),
            esc(&p.date),
            esc(&p.method),
            format!(
                "<span class=\"num\">{}</span>",
                esc(&views::money_text(p.amount, &cur_code))
            ),
        ]);
    }

    let mut actions = vec![views::link_btn(
        &format!("/customer-invoices/{}/pdf", inv.id),
        &tr(l, "a_pdf"),
    )];
    if inv.status == InvoiceStatus::Draft {
        actions.push(views::post_btn(
            &format!("/customer-invoices/{}/issue", inv.id),
            &tr(l, "a_issue"),
            "btn-primary",
        ));
    }
    if inv.display_status().is_payable() {
        actions.push(views::link_btn(
            &format!("/payments/new?invoice={}&kind=customer", inv.id),
            &tr(l, "a_pay"),
        ));
    }
    if !inv.status.is_settled() {
        actions.push(views::post_btn(
            &format!("/customer-invoices/{}/cancel", inv.id),
            &tr(l, "a_cancel"),
            "btn-danger",
        ));
    }

    let meta = super::meta_grid(vec![
        (tr(l, "lbl_customer"), esc(&inv.party_name)),
        (tr(l, "lbl_invoice_date"), esc(&inv.invoice_date)),
        (tr(l, "lbl_due_date"), esc(&inv.due_date)),
        (tr(l, "lbl_status"), views::status_badge(l, &inv.display_status().key())),
        (tr(l, "lbl_source"), esc(&inv.source_number)),
        (tr(l, "lbl_created_by"), esc(&inv.created_by)),
    ]);

    let body = format!(
        "{}{}{}{}{}",
        super::page_head(&ctx, "nav_customer_invoices", &inv.number, actions),
        views::card(&tr(l, "nav_customer_invoices"), meta),
        views::card(&tr(l, "lbl_lines"), super::lines_view(&ctx, &inv.lines, &cur_code)),
        views::card(
            &tr(l, "m_totals"),
            super::totals_block(&ctx, &inv.totals, Some(inv.paid_amount), &cur_code)
        ),
        views::card(
            &tr(l, "nav_payments"),
            views::table(
                l,
                &[
                    &tr(l, "lbl_number"),
                    &tr(l, "lbl_date"),
                    &tr(l, "lbl_method"),
                    &tr(l, "lbl_amount"),
                ],
                pay_rows,
                "msg_no_records"
            )
        )
    );
    let title = views::Title::raw(&inv.number);
    Ok(views::respond(views::page(&ctx, &title, body)))
}

pub async fn ci_issue(ctx: Ctx, Path(id): Path<String>) -> Result<Response, AppError> {
    let mut inv = services::get_by_id::<Invoice>(&ctx.state.db.customer_invoices(), &ctx.company.id, &id).await?;
    if inv.status != InvoiceStatus::Draft {
        return Ok(ctx.back(&format!("/customer-invoices/{id}"), "msg_cannot_post_twice", "err").into_response());
    }
    let customer = services::get_by_id::<Customer>(&ctx.state.db.customers(), &ctx.company.id, &inv.party_id).await?;
    let credit = services::credit_position(&ctx.state.db, &ctx.company.id, &customer).await?;
    if !credit.allows(inv.totals.total) {
        return Ok(ctx.back(&format!("/customer-invoices/{id}"), "msg_credit_exceeded", "err").into_response());
    }
    inv.status = InvoiceStatus::Open;
    services::persist_invoice(&ctx.state.db, InvoiceKind::Customer, &inv).await?;
    ctx.state
        .bus
        .publish(
            "customer_invoice.issued",
            &new_event(
                "customer_invoice.issued",
                &inv.company_id,
                &inv.id,
                &inv.number,
                inv.status.as_str(),
                inv.totals.total,
                &ctx.claims.un,
            ),
        )
        .await;
    Ok(ctx.back(&format!("/customer-invoices/{id}"), "msg_issued", "ok").into_response())
}

pub async fn ci_cancel(ctx: Ctx, Path(id): Path<String>) -> Result<Response, AppError> {
    let mut inv = services::get_by_id::<Invoice>(&ctx.state.db.customer_invoices(), &ctx.company.id, &id).await?;
    inv.status = InvoiceStatus::Cancelled;
    services::persist_invoice(&ctx.state.db, InvoiceKind::Customer, &inv).await?;
    Ok(ctx.back("/customer-invoices", "msg_saved", "ok").into_response())
}

pub async fn ci_pdf(ctx: Ctx, Path(id): Path<String>) -> Result<Response, AppError> {
    let inv = services::get_by_id::<Invoice>(&ctx.state.db.customer_invoices(), &ctx.company.id, &id).await?;
    let customer = ctx
        .state
        .db
        .customers()
        .find_one(doc! { "_id": &inv.party_id, "company_id": &ctx.company.id })
        .await?;
    let party = PartyBlock {
        name: inv.party_name.clone(),
        lines: customer
            .map(|c| {
                vec![c.address, c.city, c.phone, c.email]
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

// ---------------------------------------------------------------------------
// Payments
// ---------------------------------------------------------------------------

pub async fn payment_list(ctx: Ctx, Query(q): Query<HashMap<String, String>>) -> Result<Response, AppError> {
    let search = q.get("q").cloned().unwrap_or_default();
    let mut cur = ctx
        .state
        .db
        .payments()
        .find(super::list_filter(&ctx.company.id, &search, &["number", "party_name", "invoice_number"]))
        .sort(doc! { "created_at": -1 })
        .limit(300)
        .await?;
    let l = ctx.lang;
    let cur_code = ctx.company.currency.clone();
    let mut rows = Vec::new();
    while let Some(p) = cur.try_next().await? {
        rows.push(vec![
            esc(&p.number),
            format!(
                "<span class=\"badge {}\">{}</span>",
                if p.direction == PayDirection::In { "green" } else { "amber" },
                if p.direction == PayDirection::In {
                    tr(l, "nav_customer_invoices")
                } else {
                    tr(l, "nav_supplier_invoices")
                }
            ),
            esc(&p.party_name),
            esc(&p.invoice_number),
            esc(&p.date),
            esc(&p.method),
            format!(
                "<span class=\"num\">{}</span>",
                esc(&views::money_text(p.amount, &cur_code))
            ),
        ]);
    }
    let tools = format!(
        "{}{}",
        super::search_form(l, "/payments", &search, ""),
        views::link_btn("/payments/new", &format!("+ {}", tr(l, "a_new"))),
    );
    let body = views::list_table(
        l,
        &[
            &tr(l, "lbl_number"),
            &tr(l, "lbl_document"),
            &tr(l, "lbl_name"),
            &tr(l, "lbl_reference"),
            &tr(l, "lbl_date"),
            &tr(l, "lbl_method"),
            &tr(l, "lbl_amount"),
        ],
        rows,
        "msg_no_records",
        &tools,
    );
    let title = views::Title::of(l, "nav_payments");
    Ok(views::respond(views::page(&ctx, &title, body)))
}

pub async fn payment_new(ctx: Ctx, Query(q): Query<HashMap<String, String>>) -> Result<Response, AppError> {
    let l = ctx.lang;
    let invoice_id = q.get("invoice").cloned().unwrap_or_default();
    let kind_str = q.get("kind").cloned().unwrap_or_else(|| "customer".into());
    let kind = InvoiceKind::parse(&kind_str).unwrap_or(InvoiceKind::Customer);

    let mut party = String::new();
    let mut number = String::new();
    let mut outstanding = 0.0;
    if !invoice_id.is_empty() {
        if let Ok(inv) = services::invoice_by_id(&ctx.state.db, kind, &ctx.company.id, &invoice_id).await {
            party = inv.party_name.clone();
            number = inv.number.clone();
            outstanding = inv.outstanding();
        }
    }

    // Pickable open invoices when opened without one.
    let mut candidates = Vec::new();
    for (label, k) in [("customer", InvoiceKind::Customer), ("supplier", InvoiceKind::Supplier)] {
        for inv in open_invoices(&ctx, k).await? {
            candidates.push((
                format!("{label}:{}", inv.id),
                format!(
                    "{} · {} · {}",
                    inv.number,
                    inv.party_name,
                    views::money_text(inv.outstanding(), &ctx.company.currency)
                ),
                inv.id == invoice_id && k == kind,
            ));
        }
    }
    let options = {
        let mut o = vec![(String::new(), tr(l, "lbl_none"), invoice_id.is_empty())];
        o.extend(candidates);
        o
    };

    let methods = ["cash", "bank_transfer", "cheque", "card", "other"]
        .iter()
        .map(|m| (m.to_string(), m.replace('_', " "), *m == "cash"))
        .collect::<Vec<_>>();

    let body = format!(
        "{}<form method=\"post\" action=\"/payments\" class=\"card form\">\
           <div class=\"form-grid\">{}{}{}{}{}{}{}</div>{}</form>",
        super::page_head(
            &ctx,
            "nav_payments",
            &format!("{number} {party}"),
            vec![]
        ),
        views::select(&tr(l, "lbl_document"), "invoice_ref", &options, true),
        views::input_step(&tr(l, "lbl_amount"), "amount", &format!("{outstanding:.2}"), "0.01", true),
        views::input(&tr(l, "lbl_date"), "date", &today(), "date", true),
        views::select(&tr(l, "lbl_method"), "method", &methods, false),
        views::input(&tr(l, "lbl_reference"), "reference", "", "text", false),
        views::input(&tr(l, "lbl_outstanding"), "outstanding", &format!("{outstanding:.2}"), "text", false),
        format!("<input type=\"hidden\" name=\"kind\" value=\"{}\"/>", kind.as_str()),
        views::form_actions(&tr(l, "a_save"), "/payments", &tr(l, "a_back")),
    );
    let title = views::Title::of(l, "nav_payments");
    Ok(views::respond(views::page(&ctx, &title, body)))
}

async fn open_invoices(ctx: &Ctx, kind: InvoiceKind) -> Result<Vec<Invoice>, AppError> {
    let coll = match kind {
        InvoiceKind::Customer => ctx.state.db.customer_invoices(),
        InvoiceKind::Supplier => ctx.state.db.supplier_invoices(),
    };
    let mut cur = coll
        .find(doc! {
            "company_id": &ctx.company.id,
            "status": { "$in": [InvoiceStatus::Open.as_str(), InvoiceStatus::PartiallyPaid.as_str(), InvoiceStatus::Overdue.as_str()] }
        })
        .sort(doc! { "invoice_date": 1 })
        .limit(100)
        .await?;
    let mut out = Vec::new();
    while let Some(inv) = cur.try_next().await? {
        if inv.outstanding() > 0.005 {
            out.push(inv);
        }
    }
    Ok(out)
}

pub async fn payment_create(ctx: Ctx, pairs: Pairs) -> Result<Response, AppError> {
    let reference = pairs.get("invoice_ref");
    let (kind, invoice_id) = match reference.split_once(':') {
        Some((k, id)) => (
            InvoiceKind::parse(k).unwrap_or(InvoiceKind::Customer),
            id.to_string(),
        ),
        None => (
            InvoiceKind::parse(&pairs.get("kind")).unwrap_or(InvoiceKind::Customer),
            reference.clone(),
        ),
    };
    if invoice_id.is_empty() {
        return Ok(ctx.back("/payments/new", "msg_required", "err").into_response());
    }

    let mut inv = services::invoice_by_id(&ctx.state.db, kind, &ctx.company.id, &invoice_id).await?;
    if !inv.display_status().is_payable() {
        return Ok(ctx.back("/payments/new", "msg_cannot_post_twice", "err").into_response());
    }
    let amount = pairs.num("amount", 0.0);
    if amount <= 0.0 {
        return Ok(ctx.back("/payments/new", "msg_invalid_amount", "err").into_response());
    }
    if amount > inv.outstanding() + 0.005 {
        return Ok(ctx.back("/payments/new", "msg_amount_too_high", "err").into_response());
    }

    let number = crate::db::next_number(&ctx.state.db.db(), &ctx.company.id, "PAY").await?;
    let payment = Payment {
        id: crate::util::new_id(),
        company_id: ctx.company.id.clone(),
        number,
        direction: kind.direction(),
        party_id: inv.party_id.clone(),
        party_name: inv.party_name.clone(),
        invoice_id: inv.id.clone(),
        invoice_number: inv.number.clone(),
        date: crate::util::if_empty(&pairs.get("date"), &today()),
        amount: crate::util::round2(amount),
        method: crate::util::if_empty(&pairs.get("method"), "cash"),
        reference: pairs.get("reference"),
        notes: pairs.get("notes"),
        created_at: now_iso(),
        created_by: ctx.claims.un.clone(),
    };
    ctx.state.db.payments().insert_one(&payment).await?;

    let new_paid = inv.paid_amount + payment.amount;
    services::settle_invoice(&mut inv, new_paid);
    services::persist_invoice(&ctx.state.db, kind, &inv).await?;

    ctx.state
        .bus
        .publish(
            "payment.recorded",
            &new_event(
                "payment.recorded",
                &payment.company_id,
                &payment.id,
                &payment.number,
                inv.status.as_str(),
                payment.amount,
                &ctx.claims.un,
            ),
        )
        .await;

    let back = match kind {
        InvoiceKind::Customer => format!("/customer-invoices/{}", inv.id),
        InvoiceKind::Supplier => format!("/supplier-invoices/{}", inv.id),
    };
    Ok(axum::response::Redirect::to(&format!("{back}?m=msg_saved&k=ok")).into_response())
}

// ---------------------------------------------------------------------------
// Credit
// ---------------------------------------------------------------------------

pub async fn credit(
    ctx: Ctx,
    Query(q): Query<HashMap<String, String>>,
) -> Result<Response, AppError> {
    let l = ctx.lang;
    let search = q.get("q").cloned().unwrap_or_default();
    let cur = ctx.company.currency.clone();
    let aging = services::aging_report(&ctx.state.db, &ctx.company.id).await?;

    let mut cur_c = ctx
        .state
        .db
        .customers()
        .find(doc! { "company_id": &ctx.company.id })
        .sort(doc! { "name": 1 })
        .await?;
    let mut rows = Vec::new();
    while let Some(c) = cur_c.try_next().await? {
        if !super::row_matches(&search, &[&c.name, &c.code]) {
            continue;
        }
        let pos = services::credit_position(&ctx.state.db, &ctx.company.id, &c).await?;
        let limit_cell = if pos.unlimited() {
            tr(l, "m_unlimited")
        } else {
            views::money_text(pos.limit, &cur)
        };
        let pct = if pos.unlimited() {
            0.0
        } else if pos.limit > 0.0 {
            (pos.exposure / pos.limit * 100.0).min(100.0)
        } else {
            0.0
        };
        let bar = format!(
            "<div class=\"bar\"><span style=\"width:{pct:.0}%\"></span></div>"
        );
        rows.push(vec![
            format!(
                "<a href=\"/customers/{}\">{}</a>",
                esc(&c.id),
                esc(&c.name)
            ),
            format!("<span class=\"num\">{}</span>", esc(&limit_cell)),
            format!(
                "<span class=\"num\">{}</span>",
                esc(&views::money_text(pos.exposure, &cur))
            ),
            format!(
                "<span class=\"num\">{}</span>",
                if pos.unlimited() {
                    tr(l, "m_unlimited")
                } else {
                    esc(&views::money_text(pos.available(), &cur))
                }
            ),
            bar,
        ]);
    }

    let aging_body = format!(
        "<div class=\"aging-grid\">
            <div><span>{}</span><b>{}</b></div>
            <div><span>1-30 {}</span><b>{}</b></div>
            <div><span>31-60 {}</span><b>{}</b></div>
            <div><span>61+ {}</span><b>{}</b></div>
         </div>",
        tr(l, "m_current"),
        esc(&views::money_text(aging.current, &cur)),
        tr(l, "m_days_old"),
        esc(&views::money_text(aging.d30, &cur)),
        tr(l, "m_days_old"),
        esc(&views::money_text(aging.d60, &cur)),
        tr(l, "m_days_old"),
        esc(&views::money_text(aging.d90, &cur)),
    );

    let tools = super::search_form(l, "/credit", &search, "");
    let body = format!(
        "{}{}",
        views::card(
            &tr(l, "m_aging"),
            format!(
                "{aging_body}<div class=\"tot-row grand\"><span>{}</span><b>{}</b></div>",
                tr(l, "lbl_total"),
                esc(&views::money_text(aging.total, &cur))
            )
        ),
        views::card(
            &tr(l, "m_credit_usage"),
            views::list_table(
                l,
                &[
                    &tr(l, "lbl_customer"),
                    &tr(l, "lbl_credit_limit"),
                    &tr(l, "lbl_exposure"),
                    &tr(l, "lbl_balance"),
                    "",
                ],
                rows,
                "msg_no_records",
                &tools,
            )
        )
    );
    let title = views::Title::of(l, "nav_credit");
    Ok(views::respond(views::page(&ctx, &title, body)))
}
