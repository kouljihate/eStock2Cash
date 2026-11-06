//! Dashboard: pipeline overview, stock alerts, aging and recent documents.

use axum::response::Response;

use crate::auth::Ctx;
use crate::error::AppError;
use crate::i18n::tr;
use crate::services;
use crate::util::{esc, qty};
use crate::views;

pub async fn index(ctx: Ctx) -> Result<Response, AppError> {
    let db = &ctx.state.db;
    let company_id = ctx.company.id.clone();
    let stats = services::dashboard_stats(db, &company_id).await?;

    let recent_pos = services::list_recent::<crate::models::PurchaseOrder>(
        &db.purchase_orders(),
        &company_id,
        5,
    )
    .await?;
    let recent_invoices = services::list_recent::<crate::models::Invoice>(
        &db.customer_invoices(),
        &company_id,
        5,
    )
    .await?;
    let recent_quotes =
        services::list_recent::<crate::models::Quote>(&db.quotes(), &company_id, 4).await?;

    let l = ctx.lang;
    let cur = ctx.company.currency.clone();

    let cards = views::stats_grid(vec![
        views::stat(&tr(l, "nav_suppliers"), &stats.suppliers.to_string(), &tr(l, "nav_purchasing")),
        views::stat(&tr(l, "nav_customers"), &stats.customers.to_string(), &tr(l, "nav_sales")),
        views::stat(&tr(l, "nav_products"), &stats.products.to_string(), &tr(l, "lbl_stock")),
        views::stat(
            &tr(l, "m_low_stock"),
            &stats.low_stock.len().to_string(),
            &tr(l, "lbl_reorder"),
        ),
        views::stat(
            &tr(l, "m_receivables"),
            &views::money_text(stats.receivables, &cur),
            &tr(l, "m_aging"),
        ),
        views::stat(
            &tr(l, "m_payables"),
            &views::money_text(stats.payables, &cur),
            &tr(l, "nav_supplier_invoices"),
        ),
        views::stat(
            &tr(l, "m_stock_value"),
            &views::money_text(stats.stock_value, &cur),
            &tr(l, "nav_products"),
        ),
        views::stat(
            &tr(l, "m_open_documents"),
            &format!("{} POs · {} quotes", stats.open_pos, stats.open_quotes),
            &tr(l, "m_quick_actions"),
        ),
    ]);

    let chain = views::doc_chain(vec![
        (tr(l, "nav_suppliers"), "/suppliers".into(), true),
        (tr(l, "nav_purchase_orders"), "/purchase-orders".into(), true),
        (tr(l, "nav_goods_receipts"), "/goods-receipts".into(), true),
        (tr(l, "nav_supplier_invoices"), "/supplier-invoices".into(), true),
        (tr(l, "nav_payments"), "/payments".into(), true),
    ]);
    let chain2 = views::doc_chain(vec![
        (tr(l, "nav_quotes"), "/quotes".into(), true),
        (tr(l, "nav_customer_orders"), "/customer-orders".into(), true),
        (tr(l, "nav_delivery_notes"), "/delivery-notes".into(), true),
        (tr(l, "nav_customer_invoices"), "/customer-invoices".into(), true),
    ]);

    let low_rows = stats
        .low_stock
        .iter()
        .map(|p| {
            vec![
                format!(
                    "<a href=\"/products/{}/edit\">{}</a>",
                    esc(&p.id),
                    esc(&p.name)
                ),
                format!("<span class=\"muted\">{}</span>", esc(&p.sku)),
                format!("<span class=\"num\">{}</span>", qty(p.qty_on_hand)),
                format!("<span class=\"num\">{}</span>", qty(p.reorder_level)),
            ]
        })
        .collect::<Vec<_>>();
    let low_stock = views::card(
        &tr(l, "m_low_stock"),
        views::table(
            l,
            &[
                &tr(l, "lbl_product"),
                &tr(l, "lbl_sku"),
                &tr(l, "lbl_stock"),
                &tr(l, "lbl_reorder"),
            ],
            low_rows,
            "msg_no_records",
        ),
    );

    let po_rows = recent_pos
        .iter()
        .map(|po| {
            vec![
                format!("<a href=\"/purchase-orders/{}\">{}</a>", esc(&po.id), esc(&po.number)),
                esc(&po.supplier_name),
                format!("<span class=\"muted\">{}</span>", esc(&po.order_date)),
                views::status_badge(l, &po.status.key()),
                format!(
                    "<span class=\"num\">{}</span>",
                    esc(&views::money_text(po.totals.total, &cur))
                ),
            ]
        })
        .collect::<Vec<_>>();
    let pos = views::card(
        &tr(l, "nav_purchase_orders"),
        views::table(
            l,
            &[
                &tr(l, "lbl_number"),
                &tr(l, "lbl_supplier"),
                &tr(l, "lbl_date"),
                &tr(l, "lbl_status"),
                &tr(l, "lbl_total"),
            ],
            po_rows,
            "msg_no_records",
        ),
    );

    let inv_rows = recent_invoices
        .iter()
        .map(|inv| {
            vec![
                format!(
                    "<a href=\"/customer-invoices/{}\">{}</a>",
                    esc(&inv.id),
                    esc(&inv.number)
                ),
                esc(&inv.party_name),
                views::status_badge(l, &inv.display_status().key()),
                format!(
                    "<span class=\"num\">{}</span>",
                    esc(&views::money_text(inv.outstanding(), &cur))
                ),
            ]
        })
        .collect::<Vec<_>>();
    let invoices = views::card(
        &tr(l, "nav_customer_invoices"),
        views::table(
            l,
            &[
                &tr(l, "lbl_number"),
                &tr(l, "lbl_customer"),
                &tr(l, "lbl_status"),
                &tr(l, "lbl_outstanding"),
            ],
            inv_rows,
            "msg_no_records",
        ),
    );

    let quote_rows = recent_quotes
        .iter()
        .map(|q| {
            vec![
                format!("<a href=\"/quotes/{}\">{}</a>", esc(&q.id), esc(&q.number)),
                esc(&q.customer_name),
                views::status_badge(l, &q.status.key()),
                format!(
                    "<span class=\"num\">{}</span>",
                    esc(&views::money_text(q.totals.total, &cur))
                ),
            ]
        })
        .collect::<Vec<_>>();
    let quotes = views::card(
        &tr(l, "nav_quotes"),
        views::table(
            l,
            &[
                &tr(l, "lbl_number"),
                &tr(l, "lbl_customer"),
                &tr(l, "lbl_status"),
                &tr(l, "lbl_total"),
            ],
            quote_rows,
            "msg_no_records",
        ),
    );

    let aging_rows = vec![
        (
            tr(l, "m_current"),
            stats.aging.current,
            views::status_tone("st_open"),
        ),
        (tr(l, "lbl_days") + " 1-30", stats.aging.d30, views::status_tone("st_sent")),
        (tr(l, "lbl_days") + " 31-60", stats.aging.d60, views::status_tone("st_partially_paid")),
        (tr(l, "lbl_days") + " 61+", stats.aging.d90, views::status_tone("st_overdue")),
    ];
    let aging_body = aging_rows
        .into_iter()
        .map(|(label, value, tone)| {
            format!(
                "<div class=\"aging-row\"><span class=\"dot {}\"></span><span>{}</span><b class=\"num\">{}</b></div>",
                tone,
                esc(&label),
                esc(&views::money_text(value, &cur))
            )
        })
        .collect::<String>();
    let aging = views::card(
        &tr(l, "m_aging"),
        format!(
            "{aging_body}<div class=\"aging-row total\"><span>{}</span><b class=\"num\">{}</b></div>",
            esc(&tr(l, "lbl_total")),
            esc(&views::money_text(stats.aging.total, &cur))
        ),
    );

    let quick = views::card(
        &tr(l, "m_quick_actions"),
        format!(
            "<div class=\"quick\">{}{}{}{}{}{}</div>",
            views::link_btn("/purchase-orders/new", &tr(l, "nav_purchase_orders")),
            views::link_btn("/quotes/new", &tr(l, "nav_quotes")),
            views::link_btn("/customer-invoices/new", &tr(l, "nav_customer_invoices")),
            views::link_btn("/tickets/new", &tr(l, "m_direct_purchase")),
            views::link_btn("/suppliers/new", &tr(l, "nav_suppliers")),
            views::link_btn("/products/new", &tr(l, "nav_products")),
        ),
    );

    let pipeline = format!(
        "<div class=\"pipeline\"><div class=\"pipe-block\"><h3>{}</h3>{chain}</div><div class=\"pipe-block\"><h3>{}</h3>{chain2}</div></div>",
        esc(&tr(l, "nav_purchasing")),
        esc(&tr(l, "nav_sales"))
    );

    let body = format!(
        "{cards}{pipeline}<div class=\"grid-2\">{low_stock}{aging}</div><div class=\"grid-2\">{pos}{invoices}</div><div class=\"grid-2\">{quotes}{quick}</div>"
    );

    let title = views::Title::of(l, "nav_dashboard");
    Ok(views::respond(views::page(&ctx, &title, body)))
}

