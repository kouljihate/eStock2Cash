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
use crate::models::{Customer, DocLine, Invoice, InvoiceStatus, Ticket, Totals};
use crate::services;
use crate::util::{end_of_month, esc, now_iso, qty, round2, today};
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

    // What went out on credit today: invoices dated today, whether or not they are paid
    // already — the credit customer settles them on payment day.
    let mut credit_sold = 0.0_f64;
    let mut cur_invoices = db
        .customer_invoices()
        .find(doc! { "company_id": &company_id, "invoice_date": &today() })
        .await?;
    while let Some(inv) = cur_invoices.try_next().await? {
        credit_sold += inv.totals.total;
    }

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

    let row = |opts: &str, qt: &str, price: &str| -> String {
        format!(
            "<tr class=\"line-row\"><td><select name=\"item_product\" class=\"line-product\">{opts}</select></td>             <td><input name=\"item_qty\" class=\"num\" type=\"number\" step=\"0.01\" min=\"0\" value=\"{qt}\"/></td>             <td><input name=\"item_price\" class=\"num\" type=\"number\" step=\"0.01\" min=\"0\" value=\"{price}\"/></td>             <td class=\"line-total num\">0.00</td>             <td><button type=\"button\" class=\"btn-icon remove-line\">×</button></td></tr>"
        )
    };
    let first_row = row(&opts, "1", "0");
    let template_row = row(&opts, "1", "0");

    // The keys of the till: a calculator pad, so the amount received is tapped in
    // rather than typed on a keyboard the counter does not have.
    let mut keys = String::new();
    for d in ["7", "8", "9", "4", "5", "6", "1", "2", "3", "0", "."] {
        keys.push_str(&format!(
            "<button type=\"button\" class=\"key\" data-key=\"{d}\">{d}</button>"
        ));
    }

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
            &tr(l, "m_credit_sold_today"),
            &views::money_text(round2(credit_sold), &cur),
            &tr(l, "lbl_pay_credit"),
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
            "<form method=\"post\" action=\"/cash-registry\" class=\"form cash-desk till\">               <div class=\"form-grid till-head\">                 <label class=\"field\"><span>{customer_label}</span><select name=\"customer\">{cust_opts}</select></label>                 <label class=\"field\"><span>{payment_label}</span><select name=\"payment\" id=\"cash-mode\">                   <option value=\"cash\" selected>{pay_cash}</option>                   <option value=\"credit\">{pay_credit}</option>                 </select></label>                 {date}{cashier}{tax}               </div>               <div class=\"till-grid\">                 <div class=\"till-cart\">                   <div class=\"lines-wrap\" data-mode=\"sale\"><table class=\"lines-table\"><thead><tr>                     <th>{c_product}</th><th class=\"num\">{c_qty}</th><th class=\"num\">{c_price}</th><th class=\"num\">{c_tot}</th><th></th>                   </tr></thead><tbody id=\"lines-body\">{first_row}</tbody></table>                   <template id=\"line-tpl\">{template_row}</template>                   <div class=\"lines-foot\"><button type=\"button\" class=\"btn btn-sm\" id=\"add-line\">+ {c_add}</button></div></div>                 </div>                 <aside class=\"till-pay\">                   <div class=\"till-screen\">                     <span class=\"till-cap\">{c_tot}</span>                     <div class=\"till-amount\"><b id=\"sum-total\">0.00</b><span class=\"till-cur\">{currency}</span></div>                     <div class=\"till-sub\"><span>{c_sub} <b id=\"sum-sub\">0.00</b></span><span>{c_tax} <b id=\"sum-tax\">0.00</b></span></div>                   </div>                   <div class=\"cash-only\">                     <label class=\"till-screen till-received\"><span class=\"till-cap\">{pay_label}</span>                       <input id=\"cash-received\" name=\"paid\" type=\"text\" inputmode=\"decimal\" autocomplete=\"off\" pattern=\"[0-9]+([.,][0-9]{{0,2}})?\" value=\"\" required class=\"till-input\"/></label>                     <div class=\"keypad\">{keys}<button type=\"button\" class=\"key key-back\" data-key=\"back\">⌫</button>                       <button type=\"button\" class=\"key key-clear\" data-key=\"clear\">{clear_key}</button></div>                     <div class=\"tender-btns\">                       <button type=\"button\" class=\"btn btn-sm\" data-tender=\"20\">20</button>                       <button type=\"button\" class=\"btn btn-sm\" data-tender=\"50\">50</button>                       <button type=\"button\" class=\"btn btn-sm\" data-tender=\"100\">100</button>                       <button type=\"button\" class=\"btn btn-sm\" data-tender=\"200\">200</button>                       <button type=\"button\" class=\"btn btn-sm tender-exact\" data-tender-exact=\"1\">{exact}</button>                     </div>                     <div class=\"till-change\"><span>{change_label}</span><b id=\"cash-change\">0.00</b></div>                   </div>                   <div class=\"credit-only\" hidden>                     <label class=\"field\"><span>{due_label}</span><input type=\"date\" name=\"due_date\" value=\"{due_value}\"/></label>                     <div class=\"cash-credit-note\">{credit_hint}</div>                   </div>                   <input type=\"hidden\" name=\"default_tax\" value=\"0\"/>                   <button class=\"btn btn-primary till-charge\" type=\"submit\"><span class=\"cash-only\">{charge_cash}</span><span class=\"credit-only\" hidden>{charge_credit}</span></button>                   <a class=\"btn btn-ghost till-all\" href=\"/tickets\">{all_tickets}</a>                 </aside>               </div>             </form>",
            customer_label = tr(l, "lbl_customer"),
            cust_opts = cust_opts,
            payment_label = tr(l, "lbl_payment"),
            pay_cash = tr_lat(l, "lbl_pay_cash"),
            pay_credit = tr_lat(l, "lbl_pay_credit"),
            due_label = tr(l, "lbl_due_date"),
            due_value = end_of_month(&today()),
            credit_hint = tr(l, "m_credit_till_hint"),
            date = views::input(&tr(l, "lbl_date"), "date", &today(), "date", true),
            cashier = views::input(&tr(l, "lbl_cashier"), "cashier", &ctx.claims.un, "text", false),
            tax = views::input_step(&tr(l, "lbl_tax"), "tax", "0", "0.01", false),
            c_product = tr(l, "lbl_product"),
            c_qty = tr(l, "lbl_quantity"),
            c_price = tr(l, "lbl_unit_price"),
            c_tot = tr(l, "lbl_total"),
            c_add = tr(l, "a_add_line"),
            first_row = first_row,
            template_row = template_row,
            keys = keys,
            c_sub = tr_lat(l, "lbl_subtotal"),
            c_tax = tr_lat(l, "lbl_tax"),
            currency = esc(&cur),
            pay_label = tr(l, "lbl_amount_received"),
            exact = tr(l, "a_exact"),
            change_label = tr_lat(l, "lbl_change"),
            clear_key = tr(l, "a_clear"),
            charge_cash = tr(l, "a_charge"),
            charge_credit = tr(l, "a_charge_credit"),
            all_tickets = tr(l, "nav_tickets"),
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
    let cash = pairs.get("payment") != "credit";

    // Who is buying. A credit sale has no walk-in to fall back on: the invoice needs
    // a customer to be owed by.
    let customer = if pairs.get("customer").is_empty() {
        None
    } else {
        db.customers()
            .find_one(doc! { "_id": &pairs.get("customer"), "company_id": &company_id })
            .await?
    };
    if !cash && customer.is_none() {
        return Ok(ctx
            .back("/cash-registry", "msg_credit_needs_customer", "err")
            .into_response());
    }

    // A credit sale: the goods leave now, the money is owed until the invoice falls
    // due — the end of the month unless the cashier picked another day.
    if !cash {
        let Some(customer) = customer else {
            return Ok(ctx
                .back("/cash-registry", "msg_credit_needs_customer", "err")
                .into_response());
        };
        // the till's tax amount becomes one rate across the lines, so the invoice
        // totals what the customer was quoted
        let rate = if subtotal > 0.0 {
            round2(tax / subtotal * 100.0)
        } else {
            0.0
        };
        let lines = items
            .iter()
            .map(|i| DocLine {
                product_id: i.product_id.clone(),
                description: i.name.clone(),
                qty: i.qty,
                unit_price: i.price,
                tax_rate: rate,
            })
            .collect::<Vec<_>>();
        let totals = Totals::compute(&lines);
        let credit = services::credit_position(db, &company_id, &customer).await?;
        if !credit.allows(totals.total) {
            return Ok(ctx
                .back("/cash-registry", "msg_credit_exceeded", "err")
                .into_response());
        }

        let number = crate::db::next_number(&db.db(), &company_id, "CI").await?;
        let inv = Invoice {
            id: crate::util::new_id(),
            company_id: company_id.clone(),
            number,
            party_id: customer.id.clone(),
            party_name: customer.name.clone(),
            source_id: String::new(),
            source_number: String::new(),
            invoice_date: crate::util::if_empty(&pairs.get("date"), &today()),
            due_date: crate::util::if_empty(&pairs.get("due_date"), &end_of_month(&today())),
            lines,
            totals,
            status: InvoiceStatus::Open,
            paid_amount: 0.0,
            notes: String::new(),
            created_at: now_iso(),
            created_by: ctx.claims.un.clone(),
        };
        db.customer_invoices().insert_one(&inv).await?;
        services::post_credit_sale(db, &inv, &ctx.claims.un).await?;
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
        return Ok(axum::response::Redirect::to(&format!(
            "/customer-invoices/{}?m=msg_credit_sale&k=ok",
            inv.id
        ))
        .into_response());
    }

    if paid + 0.005 < total {
        return Ok(ctx
            .back("/cash-registry", "msg_paid_short", "err")
            .into_response());
    }
    let change = round2(paid - total);
    let (customer_id, customer_name) = match &customer {
        Some(c) => (c.id.clone(), c.name.clone()),
        None => (String::new(), String::new()),
    };

    let number = crate::db::next_number(&db.db(), &company_id, "TK").await?;
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
