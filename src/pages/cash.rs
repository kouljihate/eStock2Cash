//! Cash Registry: the till. It takes the cash from a customer, moves the stock
//! through the ticket it writes, and hands back a receipt ready for the printer.

use axum::response::{IntoResponse, Response};
use futures_util::TryStreamExt;
use mongodb::bson::doc;

use crate::auth::Ctx;
use crate::error::AppError;
use crate::events::new_event;
use crate::form::{Pairs, parse_ticket_items, product_index};
use crate::i18n::{tr, tr_lat};
use crate::models::{Customer, Ticket};
use crate::services;
use crate::util::{esc, now_iso, qty, round2, today};
use crate::views;

/// The `HH:MM` half of an ISO timestamp, for the day's receipts.
fn time_of(created_at: &str) -> String {
    if created_at.len() >= 16 {
        created_at[11..16].to_string()
    } else {
        created_at.to_string()
    }
}

pub async fn desk(ctx: Ctx) -> Result<Response, AppError> {
    let l = ctx.lang;
    let db = &ctx.state.db;
    let company_id = ctx.company.id.clone();
    let cur = ctx.company.currency.clone();

    let products = product_index(db, &company_id).await?;
    let mut list: Vec<crate::models::Product> = products.values().cloned().collect();
    list.sort_by(|a, b| a.name.cmp(&b.name));

    let mut customers: Vec<Customer> = Vec::new();
    let mut cur_customers = db
        .customers()
        .find(doc! { "company_id": &company_id })
        .await?;
    while let Some(c) = cur_customers.try_next().await? {
        customers.push(c);
    }
    customers.sort_by(|a, b| a.name.cmp(&b.name));

    let mut opts = String::from("<option value=\"\">—</option>");
    for p in &list {
        opts.push_str(&format!(
            "<option value=\"{}\" data-price=\"{:.2}\" data-name=\"{}\">{} · {} · {} {}</option>",
            esc(&p.id),
            p.price,
            esc(&p.name),
            esc(&p.name),
            esc(&views::money_text(p.price, &cur)),
            esc(&tr_lat(l, "lbl_stock")),
            esc(&qty(p.qty_on_hand))
        ));
    }

    let row = |opts: &str, name: &str, qt: &str, price: &str| -> String {
        format!(
            "<tr class=\"line-row\"><td><select name=\"item_product\" class=\"line-product\">{opts}</select></td>\
             <td><input name=\"item_desc\" value=\"{name}\"/></td>\
             <td><input name=\"item_qty\" class=\"num\" type=\"number\" step=\"0.01\" value=\"{qt}\"/></td>\
             <td><input name=\"item_price\" class=\"num\" type=\"number\" step=\"0.01\" value=\"{price}\"/></td>\
             <td class=\"line-total\">0.00</td>\
             <td><button type=\"button\" class=\"btn-icon remove-line\">×</button></td></tr>"
        )
    };
    let first_row = row("", "", "1", "0").replace(
        "<select name=\"item_product\" class=\"line-product\">",
        &format!("<select name=\"item_product\" class=\"line-product\">{opts}"),
    );
    let template_row = row(&opts, "", "1", "0");

    let mut cust_opts = format!(
        "<option value=\"\">{}</option>",
        esc(&tr_lat(l, "lbl_walk_in"))
    );
    for c in &customers {
        cust_opts.push_str(&format!(
            "<option value=\"{}\">{}</option>",
            esc(&c.id),
            esc(&format!("{} · {}", c.name, c.code))
        ));
    }

    // The day's takings: what the drawer holds, how many receipts, how much stock
    // left the shelves, and the average basket.
    let mut cur_tickets = db
        .tickets()
        .find(doc! { "company_id": &company_id, "date": &today() })
        .sort(doc! { "created_at": -1 })
        .limit(200)
        .await?;
    let (mut cash, mut sales, mut count, mut items) = (0.0_f64, 0.0_f64, 0_usize, 0.0_f64);
    let mut rows = Vec::new();
    while let Some(t) = cur_tickets.try_next().await? {
        count += 1;
        cash += t.paid;
        sales += t.total;
        items += t.items.iter().map(|i| i.qty).sum::<f64>();
        rows.push(vec![
            format!("<a href=\"/tickets/{}\">{}</a>", esc(&t.id), esc(&t.number)),
            format!("<span class=\"mono small\">{}</span>", esc(&time_of(&t.created_at))),
            if t.customer_name.is_empty() {
                format!("<span class=\"muted\">{}</span>", esc(&tr_lat(l, "lbl_walk_in")))
            } else {
                esc(&t.customer_name)
            },
            format!("<span class=\"num\">{}</span>", t.items.len()),
            format!(
                "<span class=\"num\">{}</span>",
                esc(&views::money_text(t.total, &cur))
            ),
            format!(
                "<span class=\"num\">{}</span>",
                esc(&views::money_text(t.change, &cur))
            ),
            views::icon_link(
                &format!("/tickets/{}?print=1", t.id),
                "print",
                &tr(l, "a_print"),
            ),
        ]);
    }
    let average = if count > 0 {
        round2(sales / count as f64)
    } else {
        0.0
    };

    let stats = views::stats_grid(vec![
        views::stat(
            &tr(l, "m_cash_taken_today"),
            &views::money_text(round2(cash), &cur),
            &tr(l, "lbl_paid"),
        ),
        views::stat(
            &tr(l, "m_receipts_today"),
            &count.to_string(),
            &tr(l, "nav_cash_registry"),
        ),
        views::stat(&tr(l, "m_items_today"), &qty(items), &tr(l, "nav_products")),
        views::stat(
            &tr(l, "m_average_receipt"),
            &views::money_text(average, &cur),
            &tr(l, "lbl_total"),
        ),
    ]);

    let sale = views::card(
        &tr(l, "m_cash_new_sale"),
        format!(
            "<form method=\"post\" action=\"/cash-registry\" class=\"form cash-desk\">\
               <div class=\"form-grid\">\
                 <label class=\"field\"><span>{customer_label}</span><select name=\"customer\">{cust_opts}</select></label>\
                 {date}{cashier}\
               </div>\
               <div class=\"lines-wrap\" data-mode=\"sale\"><table class=\"lines-table\"><thead><tr>\
                 <th>{c_product}</th><th>{c_desc}</th><th class=\"num\">{c_qty}</th><th class=\"num\">{c_price}</th><th class=\"num\">{c_tot}</th><th></th>\
               </tr></thead><tbody id=\"lines-body\">{first_row}</tbody></table>\
               <template id=\"line-tpl\">{template_row}</template>\
               <div class=\"lines-foot\"><button type=\"button\" class=\"btn btn-sm\" id=\"add-line\">+ {c_add}</button>\
                 <div class=\"lines-sum\"><span>{c_sub}: <b id=\"sum-sub\">0.00</b></span><span>{c_tax}: <b id=\"sum-tax\">0.00</b></span><span class=\"grand\">{c_tot}: <b id=\"sum-total\">0.00</b></span></div>\
               </div></div>\
               <div class=\"form-grid cash-pay\">\
                 <label class=\"field\"><span>{pay_label}</span><input id=\"cash-received\" name=\"paid\" type=\"number\" step=\"0.01\" min=\"0\" value=\"\" required/></label>\
                 {tax}\
                 <div class=\"field\"><span>{tender_label}</span><div class=\"tender-btns\">\
                   <button type=\"button\" class=\"btn btn-sm\" data-tender=\"20\">20</button>\
                   <button type=\"button\" class=\"btn btn-sm\" data-tender=\"50\">50</button>\
                   <button type=\"button\" class=\"btn btn-sm\" data-tender=\"100\">100</button>\
                   <button type=\"button\" class=\"btn btn-sm\" data-tender=\"200\">200</button>\
                   <button type=\"button\" class=\"btn btn-sm\" data-tender-exact=\"1\">{exact}</button>\
                 </div></div>\
                 <div class=\"field\"><span>{change_label}</span><div class=\"cash-change\" id=\"cash-change\">0.00</div></div>\
               </div>\
               <input type=\"hidden\" name=\"default_tax\" value=\"0\"/>\
               {actions}\
             </form>",
            customer_label = tr(l, "lbl_customer"),
            cust_opts = cust_opts,
            date = views::input(&tr(l, "lbl_date"), "date", &today(), "date", true),
            cashier = views::input(&tr(l, "lbl_cashier"), "cashier", &ctx.claims.un, "text", false),
            c_product = tr(l, "lbl_product"),
            c_desc = tr(l, "lbl_description"),
            c_qty = tr(l, "lbl_quantity"),
            c_price = tr(l, "lbl_unit_price"),
            c_tot = tr(l, "lbl_total"),
            c_add = tr(l, "a_add_line"),
            first_row = first_row,
            template_row = template_row,
            c_sub = tr(l, "lbl_subtotal"),
            c_tax = tr(l, "lbl_tax"),
            pay_label = tr(l, "lbl_amount_received"),
            tax = views::input_step(&tr(l, "lbl_tax"), "tax", "0", "0.01", false),
            tender_label = tr(l, "lbl_quick_tender"),
            exact = tr(l, "a_exact"),
            change_label = tr(l, "lbl_change"),
            actions = views::form_actions(&tr(l, "a_charge"), "/tickets", &tr(l, "nav_tickets")),
        ),
    );

    let receipts = views::card(
        &tr(l, "m_cash_today"),
        views::table_actions(
            l,
            &[
                &tr(l, "lbl_number"),
                &tr(l, "lbl_time"),
                &tr(l, "lbl_customer"),
                &tr(l, "lbl_lines"),
                &tr(l, "lbl_total"),
                &tr(l, "lbl_change"),
                &tr(l, "lbl_actions"),
            ],
            rows,
            "msg_no_records",
        ),
    );

    let body = format!("{stats}{sale}{receipts}");
    let title = views::Title::of(l, "nav_cash_registry");
    Ok(views::respond(views::page(&ctx, &title, body)))
}

/// Take the cash: one ticket, the stock moved by `services::post_ticket`, then the
/// receipt, open on its print dialog.
pub async fn sell(ctx: Ctx, pairs: Pairs) -> Result<Response, AppError> {
    let db = &ctx.state.db;
    let company_id = ctx.company.id.clone();

    let products = product_index(db, &company_id).await?;
    let items = parse_ticket_items(&pairs, &products);
    if items.is_empty() {
        return Ok(ctx
            .back("/cash-registry", "msg_line_required", "err")
            .into_response());
    }
    let subtotal = round2(items.iter().map(|i| i.line_total()).sum::<f64>());
    let tax = round2(pairs.num("tax", 0.0));
    let total = round2(subtotal + tax);
    let paid = round2(pairs.num("paid", 0.0));
    if paid + 0.005 < total {
        return Ok(ctx
            .back("/cash-registry", "msg_paid_short", "err")
            .into_response());
    }
    let change = round2(paid - total);

    let mut customer_id = pairs.get("customer");
    let mut customer_name = String::new();
    if !customer_id.is_empty() {
        let found = db
            .customers()
            .find_one(doc! { "_id": &customer_id, "company_id": &company_id })
            .await?;
        match found {
            Some(c) => customer_name = c.name,
            None => customer_id = String::new(),
        }
    }

    let number = crate::db::next_number(&db.db, &company_id, "TK").await?;
    let ticket = Ticket {
        id: crate::util::new_id(),
        company_id: company_id.clone(),
        number,
        date: crate::util::if_empty(&pairs.get("date"), &today()),
        items,
        subtotal,
        tax,
        total,
        paid,
        change,
        cashier: crate::util::if_empty(&pairs.get("cashier"), &ctx.claims.un),
        customer_id,
        customer_name,
        created_at: now_iso(),
        created_by: ctx.claims.un.clone(),
    };
    db.tickets().insert_one(&ticket).await?;
    services::post_ticket(db, &ticket).await?;
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

    Ok(axum::response::Redirect::to(&format!(
        "/tickets/{}?m=msg_cash_taken&k=ok&print=1",
        ticket.id
    ))
    .into_response())
}
