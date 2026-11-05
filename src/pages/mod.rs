//! Page handlers + their Leptos-rendered bodies, grouped by domain.

pub mod admin;
pub mod catalog;
pub mod dashboard;
pub mod invoicing;
pub mod login;
pub mod parties;
pub mod purchasing;
pub mod sales;
pub mod tickets;

use crate::auth::Ctx;
use crate::i18n::tr;
use crate::models::{DocLine, Product};
use crate::util::{esc, money, qty};

/// Small header with a title and action buttons.
pub fn page_head(ctx: &Ctx, title_key: &str, subtitle: &str, actions: Vec<String>) -> String {
    format!(
        "<div class=\"page-head\"><div><h2>{}</h2><p class=\"muted\">{}</p></div><div class=\"page-actions\">{}</div></div>",
        esc(&tr(ctx.lang, title_key)),
        esc(subtitle),
        actions.join("")
    )
}

/// Escape a user search string before using it as a MongoDB regex.
pub fn regex_escape(s: &str) -> String {
    s.chars()
        .flat_map(|c| {
            if "\\.+*?()|[]{}^$#".contains(c) {
                vec!['\\', c]
            } else {
                vec![c]
            }
        })
        .collect()
}

pub fn meta_grid(items: Vec<(String, String)>) -> String {
    let rows = items
        .into_iter()
        .map(|(k, v)| {
            format!(
                "<div class=\"meta-item\"><span>{}</span><strong>{}</strong></div>",
                esc(&k),
                if v.is_empty() { "—".to_string() } else { v }
            )
        })
        .collect::<String>();
    format!("<div class=\"meta\">{rows}</div>")
}

fn product_options(products: &[Product], selected: &str) -> String {
    let mut out = String::from("<option value=\"\">—</option>");
    for p in products {
        out.push_str(&format!(
            "<option value=\"{}\" data-price=\"{:.2}\" data-cost=\"{:.2}\"{}>{}</option>",
            esc(&p.id),
            p.price,
            p.cost,
            if p.id == selected { " selected" } else { "" },
            esc(&format!("{} ({})", p.name, p.sku))
        ));
    }
    out
}

/// Editable line editor used by every document create form.
///
/// Rows submit repeated keys (`item_product`, `item_desc`, `item_qty`,
/// `item_price`, `item_tax`) which the server zips by index.
pub fn lines_editor(
    ctx: &Ctx,
    products: &[Product],
    lines: &[DocLine],
    default_tax: f64,
    use_cost: bool,
) -> String {
    let options = product_options(products, "");
    let mut rows = String::new();

    let render_row = |line: &DocLine| -> String {
        format!(
            "<tr class=\"line-row\">\
              <td><select name=\"item_product\" class=\"line-product\"><option value=\"\">—</option>{opts}</select></td>\
              <td><input name=\"item_desc\" value=\"{desc}\" placeholder=\"{ph}\"/></td>\
              <td><input name=\"item_qty\" class=\"num\" type=\"number\" step=\"0.01\" value=\"{q}\"/></td>\
              <td><input name=\"item_price\" class=\"num\" type=\"number\" step=\"0.01\" value=\"{price}\"/></td>\
              <td><input name=\"item_tax\" class=\"num\" type=\"number\" step=\"0.01\" value=\"{tax}\"/></td>\
              <td class=\"line-total\">0.00</td>\
              <td><button type=\"button\" class=\"btn-icon remove-line\" title=\"{rm}\">×</button></td>\
            </tr>",
            opts = if line.product_id.is_empty() {
                options.clone()
            } else {
                product_options(products, &line.product_id)
            },
            desc = esc(&line.description),
            ph = esc(&tr(ctx.lang, "lbl_description")),
            q = if line.qty > 0.0 { qty(line.qty) } else { "1".into() },
            price = format!("{:.2}", line.unit_price),
            tax = qty(line.tax_rate),
            rm = esc(&tr(ctx.lang, "a_remove")),
        )
    };

    if lines.is_empty() {
        rows.push_str(&render_row(&DocLine {
            product_id: String::new(),
            description: String::new(),
            qty: 1.0,
            unit_price: 0.0,
            tax_rate: default_tax,
        }));
    } else {
        for line in lines {
            rows.push_str(&render_row(line));
        }
    }

    let tax = qty(default_tax);

    format!(
        "<div class=\"lines-wrap\" data-mode=\"{}\">\
          <table class=\"lines-table\">\
            <thead><tr>\
              <th>{}</th><th>{}</th><th class=\"num\">{}</th><th class=\"num\">{}</th><th class=\"num\">{}</th><th class=\"num\">{}</th><th></th>\
            </tr></thead>\
            <tbody id=\"lines-body\">{rows}</tbody>\
          </table>\
          <template id=\"line-tpl\"><tr class=\"line-row\">\
              <td><select name=\"item_product\" class=\"line-product\"><option value=\"\">—</option>{options}</select></td>\
              <td><input name=\"item_desc\" value=\"\" placeholder=\"{}\"/></td>\
              <td><input name=\"item_qty\" class=\"num\" type=\"number\" step=\"0.01\" value=\"1\"/></td>\
              <td><input name=\"item_price\" class=\"num\" type=\"number\" step=\"0.01\" value=\"0\"/></td>\
              <td><input name=\"item_tax\" class=\"num\" type=\"number\" step=\"0.01\" value=\"{tax}\"/></td>\
              <td class=\"line-total\">0.00</td>\
              <td><button type=\"button\" class=\"btn-icon remove-line\">×</button></td>\
            </tr></template>\
          <div class=\"lines-foot\">\
            <button type=\"button\" class=\"btn btn-sm\" id=\"add-line\">+ {}</button>\
            <div class=\"lines-sum\">\
              <span>{}: <b id=\"sum-sub\">0.00</b></span>\
              <span>{}: <b id=\"sum-tax\">0.00</b></span>\
              <span class=\"grand\">{}: <b id=\"sum-total\">0.00</b></span>\
            </div>\
          </div>\
          <input type=\"hidden\" name=\"default_tax\" value=\"{tax}\"/>\
        </div>",
        if use_cost { "cost" } else { "sale" },
        esc(&tr(ctx.lang, "lbl_product")),
        esc(&tr(ctx.lang, "lbl_description")),
        esc(&tr(ctx.lang, "lbl_quantity")),
        esc(&tr(ctx.lang, "lbl_unit_price")),
        esc(&tr(ctx.lang, "lbl_tax")),
        esc(&tr(ctx.lang, "lbl_total")),
        esc(&tr(ctx.lang, "lbl_description")),
        esc(&tr(ctx.lang, "a_add_line")),
        esc(&tr(ctx.lang, "lbl_subtotal")),
        esc(&tr(ctx.lang, "lbl_tax")),
        esc(&tr(ctx.lang, "lbl_total")),
    )
}

/// Read-only lines table for document detail pages.
pub fn lines_view(ctx: &Ctx, lines: &[DocLine], currency: &str) -> String {
    if lines.is_empty() {
        return format!(
            "<div class=\"empty\">{}</div>",
            esc(&tr(ctx.lang, "m_no_lines"))
        );
    }
    let rows = lines
        .iter()
        .map(|l| {
            vec![
                esc(&l.description),
                format!("<span class=\"num\">{}</span>", qty(l.qty)),
                format!("<span class=\"num\">{}</span>", money(l.unit_price)),
                format!("<span class=\"num\">{}%</span>", qty(l.tax_rate)),
                format!(
                    "<span class=\"num\">{}</span>",
                    esc(&money_text(l.net(), currency))
                ),
            ]
        })
        .collect::<Vec<_>>();
    format!(
        "<div class=\"table-wrap\"><table><thead><tr><th>{}</th><th class=\"num\">{}</th><th class=\"num\">{}</th><th class=\"num\">{}</th><th class=\"num\">{}</th></tr></thead><tbody>{}</tbody></table></div>",
        esc(&tr(ctx.lang, "lbl_description")),
        esc(&tr(ctx.lang, "lbl_quantity")),
        esc(&tr(ctx.lang, "lbl_unit_price")),
        esc(&tr(ctx.lang, "lbl_tax")),
        esc(&tr(ctx.lang, "lbl_total")),
        rows.into_iter()
            .map(|cells| format!(
                "<tr>{}</tr>",
                cells
                    .into_iter()
                    .map(|c| format!("<td>{c}</td>"))
                    .collect::<String>()
            ))
            .collect::<String>()
    )
}

/// `<option>` list for a product select (used by receipt/delivery editors).
pub fn catalog_select(products: &std::collections::HashMap<String, Product>, selected: &str) -> String {
    let mut list: Vec<&Product> = products.values().collect();
    list.sort_by(|a, b| a.name.cmp(&b.name));
    let mut out = String::from("<option value=\"\">—</option>");
    for p in list {
        out.push_str(&format!(
            "<option value=\"{}\"{}>{}</option>",
            esc(&p.id),
            if p.id == selected { " selected" } else { "" },
            esc(&format!("{} ({})", p.name, p.sku))
        ));
    }
    out
}

pub fn money_text(v: f64, currency: &str) -> String {
    format!("{} {}", money(v), currency)
}

/// Totals block (subtotal / tax / total / paid / outstanding).
pub fn totals_block(ctx: &Ctx, totals: &crate::models::Totals, paid: Option<f64>, currency: &str) -> String {
    let mut rows = vec![
        (tr(ctx.lang, "lbl_subtotal"), money_text(totals.subtotal, currency)),
        (tr(ctx.lang, "lbl_tax"), money_text(totals.tax, currency)),
    ];
    if let Some(p) = paid {
        rows.push((tr(ctx.lang, "lbl_paid"), money_text(p, currency)));
        rows.push((
            tr(ctx.lang, "lbl_outstanding"),
            money_text(crate::util::round2((totals.total - p).max(0.0)), currency),
        ));
    }
    let body = rows
        .into_iter()
        .map(|(k, v)| {
            format!(
                "<div class=\"tot-row\"><span>{}</span><b>{}</b></div>",
                esc(&k),
                esc(&v)
            )
        })
        .collect::<String>();
    format!(
        "<div class=\"totals\">{body}<div class=\"tot-row grand\"><span>{}</span><b>{}</b></div></div>",
        esc(&tr(ctx.lang, "lbl_total")),
        esc(&money_text(totals.total, currency))
    )
}
