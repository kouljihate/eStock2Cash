//! Direct purchase tickets (POS) with printable receipt and ESC/POS output.

use axum::extract::Path;
use axum::response::{IntoResponse, Response};
use futures_util::TryStreamExt;
use mongodb::bson::doc;

use crate::auth::Ctx;
use crate::error::AppError;
use crate::events::new_event;
use crate::form::{Pairs, parse_ticket_items, product_index};
use crate::i18n::tr;
use crate::models::Ticket;
use crate::pdf::{PdfLabels, ticket_pdf};
use crate::services;
use crate::util::{esc, money, now_iso, qty, round2, today};
use crate::views;

pub async fn list(ctx: Ctx) -> Result<Response, AppError> {
    let mut cur = ctx
        .state
        .db
        .tickets()
        .find(doc! { "company_id": &ctx.company.id })
        .sort(doc! { "created_at": -1 })
        .limit(200)
        .await?;
    let l = ctx.lang;
    let cur_code = ctx.company.currency.clone();
    let mut rows = Vec::new();
    while let Some(t) = cur.try_next().await? {
        rows.push(vec![
            format!("<a href=\"/tickets/{}\">{}</a>", esc(&t.id), esc(&t.number)),
            esc(&t.date),
            format!("<span class=\"muted\">{}</span>", esc(&t.cashier)),
            format!("<span class=\"num\">{}</span>", t.items.len()),
            format!(
                "<span class=\"num\">{}</span>",
                esc(&views::money_text(t.total, &cur_code))
            ),
            views::icon_link(&format!("/tickets/{}", t.id), "print", &tr(l, "a_print")),
        ]);
    }
    let body = format!(
        "{}{}",
        super::page_head(
            &ctx,
            "nav_tickets",
            "",
            vec![views::link_btn("/tickets/new", &format!("+ {}", tr(l, "m_direct_purchase")))]
        ),
        views::table_actions(
            l,
            &[
                &tr(l, "lbl_number"),
                &tr(l, "lbl_date"),
                &tr(l, "lbl_cashier"),
                &tr(l, "lbl_lines"),
                &tr(l, "lbl_total"),
                &tr(l, "lbl_actions"),
            ],
            rows,
            "msg_no_records",
        )
    );
    let title = views::Title::of(l, "nav_tickets");
    Ok(views::respond(views::page(&ctx, &title, body)))
}

pub async fn new(ctx: Ctx) -> Result<Response, AppError> {
    let l = ctx.lang;
    let products = product_index(&ctx.state.db, &ctx.company.id).await?;
    let mut list: Vec<crate::models::Product> = products.values().cloned().collect();
    list.sort_by(|a, b| a.name.cmp(&b.name));
    let cur_code = ctx.company.currency.clone();

    let mut opts = String::from("<option value=\"\">—</option>");
    for p in &list {
        opts.push_str(&format!(
            "<option value=\"{}\" data-price=\"{:.2}\">{}</option>",
            esc(&p.id),
            p.price,
            esc(&format!("{} · {}", p.name, views::money_text(p.price, &cur_code)))
        ));
    }

    let row = |opts: &str, name: &str, qty: &str, price: &str| -> String {
        format!(
            "<tr class=\"line-row\"><td><select name=\"item_product\">{opts}</select></td>\
             <td><input name=\"item_desc\" value=\"{name}\"/></td>\
             <td><input name=\"item_qty\" class=\"num\" type=\"number\" step=\"0.01\" value=\"{qty}\"/></td>\
             <td><input name=\"item_price\" class=\"num\" type=\"number\" step=\"0.01\" value=\"{price}\"/></td>\
             <td class=\"line-total\">0.00</td>\
             <td><button type=\"button\" class=\"btn-icon remove-line\">×</button></td></tr>"
        )
    };

    let first_row = row("", "", "1", "0").replace("<select name=\"item_product\">", &format!("<select name=\"item_product\">{opts}"));
    let template_row = row(&opts, "", "1", "0");

    let body = format!(
        "{}<form method=\"post\" action=\"/tickets\" class=\"card form\">\
           <div class=\"form-grid\">{}{}{}</div>\
           <div class=\"lines-wrap\" data-mode=\"sale\"><table class=\"lines-table\"><thead><tr>\
             <th>{}</th><th>{}</th><th class=\"num\">{}</th><th class=\"num\">{}</th><th class=\"num\">{}</th><th></th>\
           </tr></thead><tbody id=\"lines-body\">{first_row}</tbody></table>\
           <template id=\"line-tpl\">{template_row}</template>\
           <div class=\"lines-foot\"><button type=\"button\" class=\"btn btn-sm\" id=\"add-line\">+ {}</button>\
             <div class=\"lines-sum\"><span>{}: <b id=\"sum-sub\">0.00</b></span><span>{}: <b id=\"sum-tax\">0.00</b></span><span class=\"grand\">{}: <b id=\"sum-total\">0.00</b></span></div>\
           </div></div>\
           <div class=\"form-grid\">{}{}</div>\
           <input type=\"hidden\" name=\"default_tax\" value=\"0\"/>\
           {}</form>",
        super::page_head(&ctx, "nav_tickets", &tr(l, "m_direct_purchase"), vec![]),
        views::input(&tr(l, "lbl_date"), "date", &today(), "date", true),
        views::input(&tr(l, "lbl_cashier"), "cashier", &ctx.claims.un, "text", false),
        views::input_step(&tr(l, "lbl_tax_rate"), "default_tax", "0", "0.01", false),
        tr(l, "lbl_product"),
        tr(l, "lbl_description"),
        tr(l, "lbl_quantity"),
        tr(l, "lbl_unit_price"),
        tr(l, "lbl_total"),
        tr(l, "a_add_line"),
        tr(l, "lbl_subtotal"),
        tr(l, "lbl_tax"),
        tr(l, "lbl_total"),
        views::input_step(&tr(l, "lbl_paid"), "paid", "0", "0.01", false),
        views::input_step(&tr(l, "lbl_tax"), "tax", "0", "0.01", false),
        views::form_actions(&tr(l, "a_save"), "/tickets", &tr(l, "a_back")),
    );
    let title = views::Title::of(l, "m_direct_purchase");
    Ok(views::respond(views::page(&ctx, &title, body)))
}

pub async fn create(ctx: Ctx, pairs: Pairs) -> Result<Response, AppError> {
    let products = product_index(&ctx.state.db, &ctx.company.id).await?;
    let items = parse_ticket_items(&pairs, &products);
    if items.is_empty() {
        return Ok(ctx.back("/tickets/new", "msg_line_required", "err").into_response());
    }
    let subtotal = round2(items.iter().map(|i| i.line_total()).sum::<f64>());
    let tax = round2(pairs.num("tax", 0.0));
    let total = round2(subtotal + tax);
    let paid = round2(pairs.num("paid", 0.0));
    let change = round2((paid - total).max(0.0));
    let number = crate::db::next_number(&ctx.state.db.db, &ctx.company.id, "TK").await?;

    let ticket = Ticket {
        id: crate::util::new_id(),
        company_id: ctx.company.id.clone(),
        number,
        date: crate::util::if_empty(&pairs.get("date"), &today()),
        items,
        subtotal,
        tax,
        total,
        paid,
        change,
        cashier: crate::util::if_empty(&pairs.get("cashier"), &ctx.claims.un),
        created_at: now_iso(),
        created_by: ctx.claims.un.clone(),
    };
    ctx.state.db.tickets().insert_one(&ticket).await?;
    services::post_ticket(&ctx.state.db, &ticket).await?;
    ctx.state
        .bus
        .publish(
            "ticket.created",
            &new_event(
                "ticket.created",
                &ticket.company_id,
                &ticket.id,
                &ticket.number,
                "paid",
                ticket.total,
                &ctx.claims.un,
            ),
        )
        .await;

    Ok(axum::response::Redirect::to(&format!("/tickets/{}?m=msg_saved&k=ok", ticket.id))
        .into_response())
}

pub async fn detail(ctx: Ctx, Path(id): Path<String>) -> Result<Response, AppError> {
    let l = ctx.lang;
    let ticket = services::get_by_id::<Ticket>(&ctx.state.db.tickets(), &ctx.company.id, &id).await?;
    let cur_code = ctx.company.currency.clone();

    let items = ticket
        .items
        .iter()
        .map(|i| {
            vec![
                esc(&i.name),
                format!("<span class=\"num\">{}</span>", qty(i.qty)),
                format!("<span class=\"num\">{}</span>", money(i.price)),
                format!("<span class=\"num\">{}</span>", money(i.line_total())),
            ]
        })
        .collect::<Vec<_>>();

    let receipt = format!(
        "<div class=\"receipt\" id=\"receipt\">\
           <div class=\"receipt-head\"><strong>{}</strong><div>{}</div><div>{}</div></div>\
           <div class=\"receipt-meta\"><span>{}: {}</span><span>{}</span></div>\
           <table class=\"receipt-table\"><tbody>{}</tbody></table>\
           <div class=\"receipt-tot\">\
             <div><span>{}</span><b>{}</b></div>\
             <div><span>{}</span><b>{}</b></div>\
             <div><span>{}</span><b>{}</b></div>\
             <div><span>{}</span><b>{}</b></div>\
             <div><span>{}</span><b>{}</b></div>\
           </div>\
           <div class=\"receipt-foot\">{}: {}</div>\
         </div>",
        esc(&ctx.company.name),
        esc(&ctx.company.address),
        esc(&ctx.company.phone),
        tr(l, "lbl_number"),
        esc(&ticket.number),
        esc(&ticket.date),
        items
            .into_iter()
            .map(|cells| format!(
                "<tr>{}</tr>",
                cells
                    .into_iter()
                    .map(|c| format!("<td>{c}</td>"))
                    .collect::<String>()
            ))
            .collect::<String>(),
        tr(l, "lbl_subtotal"),
        esc(&views::money_text(ticket.subtotal, &cur_code)),
        tr(l, "lbl_tax"),
        esc(&views::money_text(ticket.tax, &cur_code)),
        tr(l, "lbl_total"),
        esc(&views::money_text(ticket.total, &cur_code)),
        tr(l, "lbl_paid"),
        esc(&views::money_text(ticket.paid, &cur_code)),
        tr(l, "lbl_change"),
        esc(&views::money_text(ticket.change, &cur_code)),
        tr(l, "lbl_cashier"),
        esc(&ticket.cashier),
    );

    let actions = vec![
        views::link_btn(&format!("/tickets/{}/pdf", ticket.id), &tr(l, "a_pdf")),
        format!(
            "<button class=\"btn btn-primary\" onclick=\"window.print()\">{}</button>",
            tr(l, "a_print")
        ),
        format!(
            "<button class=\"btn btn-ghost bt-print\" data-url=\"/tickets/{}/escpos\">{}</button>",
            esc(&ticket.id),
            tr(l, "a_bluetooth")
        ),
        views::link_btn(&format!("/tickets/{}/escpos", ticket.id), "ESC/POS"),
    ];

    let body = format!(
        "{}{}{}",
        super::page_head(&ctx, "nav_tickets", &ticket.number, actions),
        receipt,
        views::card(
            &tr(l, "m_ticket_printer"),
            format!(
                "<p class=\"muted\">{}</p><p class=\"mono small\">GET /tickets/{}/escpos</p>",
                tr(l, "m_ticket_hint"),
                esc(&ticket.id)
            )
        )
    );
    let title = views::Title::raw(&ticket.number);
    Ok(views::respond(views::page(&ctx, &title, body)))
}

/// Raw ESC/POS bytes for a USB/serial or Bluetooth mini printer.
pub async fn escpos(ctx: Ctx, Path(id): Path<String>) -> Result<Response, AppError> {
    let ticket = services::get_by_id::<Ticket>(&ctx.state.db.tickets(), &ctx.company.id, &id).await?;
    let labels = PdfLabels::new(ctx.lang);
    let bytes = crate::ticket::escpos_ticket(&ticket, &ctx.company, &labels);
    Ok(views::binary_response(bytes, "application/octet-stream"))
}

pub async fn pdf(ctx: Ctx, Path(id): Path<String>) -> Result<Response, AppError> {
    let ticket = services::get_by_id::<Ticket>(&ctx.state.db.tickets(), &ctx.company.id, &id).await?;
    let labels = PdfLabels::new(ctx.lang);
    let bytes = ticket_pdf(&ticket, &ctx.company, &labels);
    Ok(views::pdf_response(
        bytes,
        &format!("{}.pdf", ticket.number.replace(['/', ' '], "-")),
    ))
}
