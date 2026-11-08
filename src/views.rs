//! Server-rendered shell (Leptos `view!`) and reusable HTML fragments.

use axum::http::header::{CACHE_CONTROL, CONTENT_DISPOSITION, CONTENT_TYPE};
use axum::http::HeaderValue;
use axum::response::{Html, IntoResponse, Response};
use leptos::prelude::*;

use crate::auth::Ctx;
use crate::i18n::{Lang, tr, tr_ar, tr_lat};
use crate::util::{esc, money};

/// One icon by the app's own name; the Papicons glyph behind it is filled with
/// `currentColor`, so the stylesheet decides its size and its tone.
fn icon(name: &str) -> String {
    crate::icons::svg(name)
}

fn nav_item(active: bool, href: &str, label: &str, ico: &str) -> String {
    format!(
        "<a class=\"nav-item{}\" href=\"{}\" title=\"{}\">{}<span>{}</span></a>",
        if active { " active" } else { "" },
        href,
        esc(&crate::util::text_of(label)),
        icon(ico),
        label
    )
}

/// One collapsible group of the sidebar. `<details>` does the folding, so the menu
/// works without JS; `app.js` only remembers which groups the user folded away.
fn nav_section(key: &str, title: &str, items: &[&str]) -> String {
    format!(
        "<details class=\"nav-section\" data-nav-sec=\"{}\" open>         <summary class=\"nav-title\" title=\"{}\">         <span class=\"nav-title-text\">{}</span>{}</summary>         <div class=\"nav-links\">{}</div></details>",
        key,
        esc(&crate::util::text_of(title)),
        title,
        icon("chevron"),
        items.join("")
    )
}

fn nav_html(ctx: &Ctx) -> String {
    let path = ctx.path.as_str();
    let on = |prefix: &str| -> bool {
        if prefix == "/" {
            path == "/"
        } else {
            path.starts_with(prefix)
        }
    };
    let l = ctx.lang;

    let mut out = String::new();
    out.push_str(&nav_item(on("/"), "/", &tr(l, "nav_dashboard"), "dashboard"));
    // the till sits straight under the dashboard: it is where the day starts
    out.push_str(&nav_item(
        on("/cash-registry"),
        "/cash-registry",
        &tr(l, "nav_cash_registry"),
        "wallet",
    ));

    out.push_str(&nav_section(
        "buy",
        &tr(l, "nav_purchasing"),
        &[
            &nav_item(on("/suppliers"), "/suppliers", &tr(l, "nav_suppliers"), "suppliers"),
            &nav_item(on("/purchase-orders"), "/purchase-orders", &tr(l, "nav_purchase_orders"), "po"),
            &nav_item(on("/goods-receipts"), "/goods-receipts", &tr(l, "nav_goods_receipts"), "receipt"),
            &nav_item(on("/supplier-invoices"), "/supplier-invoices", &tr(l, "nav_supplier_invoices"), "bill"),
            &nav_item(on("/payments"), "/payments", &tr(l, "nav_payments"), "wallet"),
        ],
    ));
    out.push_str(&nav_section(
        "sell",
        &tr(l, "nav_sales"),
        &[
            &nav_item(on("/customers"), "/customers", &tr(l, "nav_customers"), "customers"),
            &nav_item(on("/quotes"), "/quotes", &tr(l, "nav_quotes"), "quote"),
            &nav_item(on("/customer-orders"), "/customer-orders", &tr(l, "nav_customer_orders"), "order"),
            &nav_item(on("/delivery-notes"), "/delivery-notes", &tr(l, "nav_delivery_notes"), "delivery"),
            &nav_item(on("/customer-invoices"), "/customer-invoices", &tr(l, "nav_customer_invoices"), "bill"),
        ],
    ));
    out.push_str(&nav_section(
        "stock",
        &tr(l, "lbl_stock"),
        &[
            &nav_item(on("/products"), "/products", &tr(l, "nav_products"), "products"),
            &nav_item(on("/credit"), "/credit", &tr(l, "nav_credit"), "credit"),
            &nav_item(on("/tickets"), "/tickets", &tr(l, "nav_tickets"), "ticket"),
        ],
    ));
    out.push_str(&nav_section(
        "admin",
        &tr(l, "nav_admin"),
        &[
            &nav_item(on("/admin"), "/admin", &tr(l, "nav_admin"), "admin"),
            &nav_item(on("/admin/events"), "/admin/events", &tr(l, "nav_events"), "events"),
            &nav_item(on("/admin/broker"), "/admin/broker", &tr(l, "nav_broker"), "broker"),
        ],
    ));
    out
}

/// Page title in both languages. The header shows the Latin half on the left and
/// the Arabic half on the right, so a title has to carry both.
#[derive(Clone)]
pub struct Title {
    pub lat: String,
    pub ar: String,
}

impl Title {
    /// Title from a dictionary key, e.g. `nav_suppliers`.
    pub fn of(lang: Lang, key: &str) -> Self {
        Self {
            lat: tr_lat(lang, key),
            ar: tr_ar(key),
        }
    }

    /// `Products · New`, joined in each language.
    pub fn join(self, lang: Lang, key: &str) -> Self {
        Self {
            lat: format!("{} · {}", self.lat, tr_lat(lang, key)),
            ar: format!("{} · {}", self.ar, tr_ar(key)),
        }
    }

    /// A document number or party name: the same text on both sides.
    pub fn raw(name: &str) -> Self {
        Self {
            lat: name.to_string(),
            ar: name.to_string(),
        }
    }
}

/// Settings dropdown shown in the middle column of every page header.
///
/// It holds three submenus: `Languages`, listing the two duos (AR/EN and AR/FR)
/// one per row; `Font size`, which splits the choice per font — Comfortaa for the
/// Latin half of every label, VIP Rawy for the Arabic half; and `Icon size`, one
/// multiplier for every glyph the app draws.
pub fn settings_menu(lang: Lang, next: &str, fs_lat: &str, fs_ar: &str, ico: &str) -> String {
    let next = crate::util::urlenc(next);

    let duo = |l: Lang, codes: &str| -> String {
        format!(
            "<a class=\"duorow{}\" href=\"/lang/{}?next={}\"{}><span class=\"duo-ar\">AR</span><span class=\"duo-sep\">/</span><span class=\"duo-lat\">{}</span><span class=\"duo-tick\">{}</span></a>",
            if l == lang { " active" } else { "" },
            l.code(),
            next,
            if l == lang { " aria-current=\"true\"" } else { "" },
            codes,
            if l == lang { "✓" } else { "" },
        )
    };

    let sizes = |face: &str, cur: &str| -> String {
        [("s", "S"), ("m", "M"), ("l", "L"), ("xl", "XL")]
            .iter()
            .map(|(code, label)| {
                format!(
                    "<a class=\"fsopt{}\" href=\"/font/{face}/{code}?next={next}\"{}>{label}</a>",
                    if *code == cur { " active" } else { "" },
                    if *code == cur { " aria-current=\"true\"" } else { "" },
                )
            })
            .collect::<String>()
    };

    let icon_sizes = |cur: &str| -> String {
        [("s", "S"), ("m", "M"), ("l", "L"), ("xl", "XL")]
            .iter()
            .map(|(code, label)| {
                format!(
                    "<a class=\"fsopt{}\" href=\"/icon/{code}?next={next}\"{}>{label}</a>",
                    if *code == cur { " active" } else { "" },
                    if *code == cur { " aria-current=\"true\"" } else { "" },
                )
            })
            .collect::<String>()
    };

    format!(
        "<details class=\"settings\"><summary class=\"settings-trigger\" title=\"{title}\">{gear}<span class=\"settings-name\">{label}</span></summary><div class=\"settings-panel\"><details class=\"sub\"><summary class=\"sub-head\">{langs}<span class=\"sub-caret\">▸</span></summary><div class=\"sub-panel\">{en}{fr}</div></details><details class=\"sub\"><summary class=\"sub-head\">{font_size}<span class=\"sub-caret\">▸</span></summary><div class=\"sub-panel\"><div class=\"fsrow\"><span class=\"fsname\">Comfortaa</span>{lat_sizes}</div><div class=\"fsrow\"><span class=\"fsname ar\">VIP Rawy</span>{ar_sizes}</div></div></details><details class=\"sub\"><summary class=\"sub-head\">{icon_size}<span class=\"sub-caret\">▸</span></summary><div class=\"sub-panel\"><div class=\"fsrow\"><span class=\"fsname\">{icons}</span>{ico_sizes}</div></div></details></div></details>",
        label = tr(lang, "lbl_settings"),
        title = esc(&crate::i18n::tr_text(lang, "lbl_settings")),
        gear = icon("cog"),
        langs = tr(lang, "lbl_languages"),
        font_size = tr(lang, "lbl_font_size"),
        icon_size = tr(lang, "lbl_icon_size"),
        icons = tr(lang, "lbl_icons"),
        en = duo(Lang::En, "EN"),
        fr = duo(Lang::Fr, "FR"),
        lat_sizes = sizes("lat", fs_lat),
        ar_sizes = sizes("ar", fs_ar),
        ico_sizes = icon_sizes(ico),
    )
}

/// Runs in `<head>`: restores the collapsed sidebar before the first paint so the
/// rail never flashes open on a page load.
pub const NAV_STATE_SCRIPT: &str = "try{if(localStorage.getItem('s2c-nav')==='closed'){document.documentElement.classList.add('nav-closed')}}catch(e){}";

/// Content of the bar that closes a page: the app's two names, the version pill and
/// the credit. Every shell wraps it in `<footer class="appfoot">`, so the app, the
/// sign-in page and the error page end on the same bar.
pub fn foot(lang: Lang) -> String {
    format!(
        "<span class=\"foot-id\"><span class=\"foot-brand\">{}</span>\
         <span class=\"foot-ver\">v{}</span></span>\
         <span class=\"foot-year\">© {} (koul)</span>",
        tr(lang, "app_name"),
        env!("CARGO_PKG_VERSION"),
        chrono::Utc::now().format("%Y")
    )
}

/// Full page shell.
pub fn page(ctx: &Ctx, title: &Title, body: String) -> String {
    let lang = ctx.lang;
    let nav = nav_html(ctx);
    let settings = settings_menu(lang, &ctx.path, ctx.fs_lat, ctx.fs_ar, ctx.ico);
    let flash_html = match &ctx.flash {
        Some(f) => format!(
            "<div class=\"flash {}\">{}<button class=\"flash-x\" onclick=\"this.parentNode.remove()\">×</button></div>",
            if f.kind == "err" { "err" } else { "ok" },
            f.message
        ),
        None => String::new(),
    };
    let page_title = format!("{} · {} · {}", title.lat, title.ar, ctx.company.name);
    let title_lat = esc(&title.lat);
    let title_ar = esc(&title.ar);
    let username = esc(&ctx.claims.un);
    let company_name = esc(&ctx.company.name);
    let tagline1 = tr(lang, "app_tagline_1");
    let tagline2 = tr(lang, "app_tagline_2");
    let sign_out_text = crate::i18n::tr_text(lang, "a_logout");
    let signed_in_ar = tr_ar("m_signed_in_as");
    let nav_toggle_text = crate::i18n::tr_text(lang, "lbl_toggle_menu");
    let foot_html = foot(lang);
    let confirm_html = confirm_dialog(lang);

    let view = view! {
        <html lang=lang.code() dir=lang.dir() data-fs-lat=ctx.fs_lat data-fs-ar=ctx.fs_ar data-ico=ctx.ico>
            <head>
                <meta charset="utf-8"/>
                <meta name="viewport" content="width=device-width, initial-scale=1"/>
                <title>{page_title}</title>
                <link rel="stylesheet" href=format!("/static/app.css?v={}", crate::routes::asset_token())/>
                <link rel="icon" href="/static/favicon.svg" type="image/svg+xml"/>
                <script inner_html=crate::views::NAV_STATE_SCRIPT></script>
            </head>
            <body class=format!("app lang-{}", lang.code())>
                <aside class="sidebar">
                    <div class="side-head">
                        <div class="brand">
                            // <div class="brand-mark">"S2C"</div>
                            <div class="brand-text">
                                <strong>{company_name}</strong>
                                <small inner_html=tagline1.clone()></small>
                                <small inner_html=tagline2.clone()></small>
                            </div>
                        </div>
                        <button class="side-toggle" type="button" data-nav-toggle aria-expanded="true" aria-controls="side-nav" title=nav_toggle_text.clone() inner_html=icon("panel")></button>
                    </div>
                    <nav class="nav" id="side-nav" inner_html=nav></nav>
                    <div class="side-foot">
                        <div class="ver">"v" { env!("CARGO_PKG_VERSION") }</div>
                    </div>
                </aside>
                <div class="main">
                    <header class="topbar">
                        <div class="tb-col tb-lat">
                            <h1>{title_lat}</h1>
                            // <span class="muted" inner_html=format!("{} {}", signed_in, username.clone())></span>
                        </div>
                        <div class="tb-col tb-mid">
                            <form method="post" action="/logout" class="inline">
                                <button class="btn btn-ghost btn-icon" type="submit" title=sign_out_text.clone() aria-label=sign_out_text.clone() inner_html=icon("logout")></button>
                            </form>
                            <div class="tb-settings" inner_html=settings></div>
                        </div>
                        // <div class="tb-col tb-ar">
                        //     <h1 class="ar">{title_ar}</h1>
                        //     <span class="muted" inner_html=format!("<span class=\"ar\">{}</span> {}", signed_in_ar, username)></span>
                        // </div>
                    </header>
                    <div class="flash-wrap" inner_html=flash_html></div>
                    <main class="content" inner_html=body></main>
                    <footer class="appfoot" inner_html=foot_html></footer>
                </div>
                <div class="modal-host" inner_html=confirm_html></div>
                <script src=format!("/static/app.js?v={}", crate::routes::asset_token())></script>
            </body>
        </html>
    };
    format!("<!doctype html>{}", view.to_html())
}

/// HTML response that is never cached (every page is per-session data).
pub fn respond(body: String) -> Response {
    let mut resp = Html(body).into_response();
    resp.headers_mut()
        .insert(CACHE_CONTROL, HeaderValue::from_static("no-store"));
    resp
}

pub fn pdf_response(bytes: Vec<u8>, filename: &str) -> Response {
    let mut resp = bytes.into_response();
    resp.headers_mut()
        .insert(CONTENT_TYPE, HeaderValue::from_static("application/pdf"));
    resp.headers_mut().insert(
        CONTENT_DISPOSITION,
        HeaderValue::from_str(&format!("inline; filename=\"{filename}\""))
            .unwrap_or(HeaderValue::from_static("inline")),
    );
    resp
}

pub fn binary_response(bytes: Vec<u8>, content_type: &'static str) -> Response {
    let mut resp = bytes.into_response();
    resp.headers_mut()
        .insert(CONTENT_TYPE, HeaderValue::from_static(content_type));
    resp.headers_mut()
        .insert(CACHE_CONTROL, HeaderValue::from_static("no-store"));
    resp
}

/// Table with headers and pre-rendered row cells.
pub fn table(lang: Lang, headers: &[&str], rows: Vec<Vec<String>>, empty_key: &str) -> String {
    table_layout(lang, headers, rows, empty_key, false, "")
}

/// The same table for a list whose last column holds the row actions: that cell is
/// marked so the stylesheet can centre its round buttons.
pub fn table_actions(
    lang: Lang,
    headers: &[&str],
    rows: Vec<Vec<String>>,
    empty_key: &str,
) -> String {
    table_layout(lang, headers, rows, empty_key, true, "")
}

/// A list table, headed by two rows: the first spans every column and holds the
/// page's own tools (the search box and the new button), the second names the
/// columns. `tools` is empty for a table that offers neither.
pub fn list_table(
    lang: Lang,
    headers: &[&str],
    rows: Vec<Vec<String>>,
    empty_key: &str,
    tools: &str,
) -> String {
    table_layout(lang, headers, rows, empty_key, false, tools)
}

/// The same, for a list whose last column holds the row actions.
pub fn list_table_actions(
    lang: Lang,
    headers: &[&str],
    rows: Vec<Vec<String>>,
    empty_key: &str,
    tools: &str,
) -> String {
    table_layout(lang, headers, rows, empty_key, true, tools)
}

fn table_layout(
    lang: Lang,
    headers: &[&str],
    rows: Vec<Vec<String>>,
    empty_key: &str,
    actions: bool,
    tools: &str,
) -> String {
    let cols = headers.len();
    let head = headers
        .iter()
        .map(|h| format!("<th>{}</th>", h))
        .collect::<String>();
    let body = if rows.is_empty() {
        // the header rows stay, so a search that matches nothing can still be
        // read and cleared from the box that made it
        format!(
            "<tr class=\"row-empty\"><td colspan=\"{cols}\"><div class=\"empty\">{}</div></td></tr>",
            tr(lang, empty_key)
        )
    } else {
        rows.into_iter()
            .map(|cells| {
                let last = cells.len().saturating_sub(1);
                format!(
                    "<tr>{}</tr>",
                    cells
                        .into_iter()
                        .enumerate()
                        .map(|(i, c)| if actions && i == last {
                            format!("<td class=\"acts\">{c}</td>")
                        } else {
                            format!("<td>{c}</td>")
                        })
                        .collect::<String>()
                )
            })
            .collect::<String>()
    };
    let tools_row = if tools.is_empty() {
        String::new()
    } else {
        format!(
            "<tr class=\"thead-tools\"><th colspan=\"{cols}\"><div class=\"page-actions\">{tools}</div></th></tr>"
        )
    };
    format!(
        "<div class=\"table-wrap\"><table><thead>{tools_row}<tr>{head}</tr></thead><tbody>{body}</tbody></table></div>"
    )
}

pub fn status_tone(key: &str) -> &'static str {
    match key {
        "st_paid" | "st_received" | "st_delivered" | "st_posted" | "st_accepted"
        | "st_confirmed" => "green",
        "st_sent" | "st_open" => "blue",
        "st_partially_received" | "st_partially_paid" => "amber",
        "st_overdue" | "st_rejected" => "red",
        "st_cancelled" | "st_expired" => "slate",
        "st_invoiced" | "st_converted" => "violet",
        _ => "slate",
    }
}

pub fn status_badge(lang: Lang, key: &str) -> String {
    format!(
        "<span class=\"badge {}\">{}</span>",
        status_tone(key),
        tr(lang, key)
    )
}

pub fn card(title: &str, body: String) -> String {
    format!(
        "<section class=\"card\"><header class=\"card-head\"><h2>{}</h2></header><div class=\"card-body\">{}</div></section>",
        title,
        body
    )
}

pub fn stat(label: &str, value: &str, hint: &str) -> String {
    format!(
        "<div class=\"stat\"><span class=\"stat-label\">{}</span><strong>{}</strong><small>{}</small></div>",
        label,
        value,
        hint
    )
}

pub fn money_text(v: f64, currency: &str) -> String {
    format!("{} {}", money(v), esc(currency))
}

pub fn input(label: &str, name: &str, value: &str, kind: &str, required: bool) -> String {
    format!(
        "<label class=\"field\"><span>{}</span><input type=\"{kind}\" name=\"{}\" value=\"{}\"{}/></label>",
        label,
        esc(name),
        esc(value),
        if required { " required" } else { "" }
    )
}

pub fn input_step(label: &str, name: &str, value: &str, step: &str, required: bool) -> String {
    format!(
        "<label class=\"field\"><span>{}</span><input type=\"number\" step=\"{step}\" name=\"{}\" value=\"{}\"{}/></label>",
        label,
        esc(name),
        esc(value),
        if required { " required" } else { "" }
    )
}

pub fn textarea(label: &str, name: &str, value: &str) -> String {
    format!(
        "<label class=\"field field-wide\"><span>{}</span><textarea name=\"{}\" rows=\"3\">{}</textarea></label>",
        label,
        esc(name),
        esc(value)
    )
}

pub fn checkbox(label: &str, name: &str, checked: bool) -> String {
    format!(
        "<label class=\"field check\"><input type=\"checkbox\" name=\"{}\"{} /><span>{}</span></label>",
        esc(name),
        if checked { " checked" } else { "" },
        label
    )
}

/// `options` = (value, label, selected)
pub fn select(label: &str, name: &str, options: &[(String, String, bool)], required: bool) -> String {
    let opts = options
        .iter()
        .map(|(v, l, s)| {
            format!(
                "<option value=\"{}\"{}>{}</option>",
                esc(v),
                if *s { " selected" } else { "" },
                esc(&crate::util::text_of(l))
            )
        })
        .collect::<String>();
    format!(
        "<label class=\"field\"><span>{}</span><select name=\"{}\"{}>{opts}</select></label>",
        label,
        esc(name),
        if required { " required" } else { "" }
    )
}

pub fn form_actions(submit_label: &str, back_href: &str, back_label: &str) -> String {
    format!(
        "<div class=\"form-actions\"><button class=\"btn btn-primary\" type=\"submit\">{}</button>\
         <a class=\"btn btn-ghost\" href=\"{}\">{}</a></div>",
        submit_label,
        esc(back_href),
        back_label
    )
}

pub fn link_btn(href: &str, label: &str) -> String {
    format!("<a class=\"btn btn-sm\" href=\"{}\">{}</a>", esc(href), label)
}

pub fn post_btn(action: &str, label: &str, tone: &str) -> String {
    format!(
        "<form method=\"post\" action=\"{}\" class=\"inline\"><button class=\"btn btn-sm {}\" type=\"submit\">{}</button></form>",
        esc(action),
        tone,
        label
    )
}

/// A row action in a table's last column is the icon only: the bilingual label the
/// button would have carried moves to its tooltip, so the cell stays a row of circles.
pub fn icon_link(href: &str, ico: &str, label: &str) -> String {
    let text = esc(&crate::util::text_of(label));
    format!(
        "<a class=\"btn-icon\" href=\"{}\" title=\"{}\" aria-label=\"{}\">{}</a>",
        esc(href),
        text,
        text,
        icon(ico)
    )
}

/// A delete is an icon too, as a form post. The form asks for the confirmation
/// (`data-confirm`) that `app.js` turns into the dialog below, and `note` — plain
/// text, escaped here — names the record the button deletes, so the dialog says
/// which one is about to go.
pub fn icon_post_of(action: &str, ico: &str, label: &str, note: &str) -> String {
    let text = esc(&crate::util::text_of(label));
    let note_attr = if note.is_empty() {
        String::new()
    } else {
        format!(" data-confirm-note=\"{}\"", esc(note))
    };
    format!(
        "<form method=\"post\" action=\"{}\" class=\"inline\" data-confirm=\"{}\"{}><button class=\"btn-icon danger\" type=\"submit\" title=\"{}\" aria-label=\"{}\">{}</button></form>",
        esc(action),
        text,
        note_attr,
        text,
        text,
        icon(ico)
    )
}

/// The confirmation a delete asks for: a `<dialog>` the browser centres and keeps the
/// focus inside, opened by `app.js` from `data-confirm` on the form. It replaces the
/// browser's own box, so the question is asked in the app's two languages and drawn
/// like the rest of it. `Cancel` comes first, so Enter on a keyboard closes the dialog
/// instead of deleting anything.
pub fn confirm_dialog(lang: Lang) -> String {
    format!(
        "<dialog class=\"modal\" id=\"confirm-dialog\" aria-labelledby=\"confirm-title\" aria-describedby=\"confirm-text\">\
           <div class=\"modal-card\">\
             <header class=\"modal-head\">\
               <div class=\"modal-mark\">{mark}</div>\
               <h2 class=\"modal-title\" id=\"confirm-title\">{title}</h2>\
             </header>\
             <div class=\"modal-body\">\
               <p class=\"modal-text\" id=\"confirm-text\">{note}</p>\
               <p class=\"modal-subject\" data-confirm-subject hidden></p>\
             </div>\
             <footer class=\"modal-actions\">\
               <button class=\"btn btn-ghost\" type=\"button\" data-confirm-cancel>{cancel}</button>\
               <button class=\"btn btn-danger-solid\" type=\"button\" data-confirm-ok>{delete}</button>\
             </footer>\
           </div>\
         </dialog>",
        mark = icon("trash"),
        title = tr(lang, "m_confirm_delete"),
        note = tr(lang, "m_confirm_note"),
        cancel = tr(lang, "a_cancel"),
        delete = tr(lang, "a_delete"),
    )
}

/// Card grid for stat blocks.
pub fn stats_grid(items: Vec<String>) -> String {
    format!("<div class=\"stats\">{}</div>", items.join(""))
}

pub fn notice(lang: Lang, key: &str, tone: &str) -> String {
    format!(
        "<div class=\"notice {}\">{}</div>",
        tone,
        tr(lang, key)
    )
}

pub fn doc_chain(steps: Vec<(String, String, bool)>) -> String {
    let items = steps
        .into_iter()
        .map(|(label, href, done)| {
            let class = if done { "chain-step done" } else { "chain-step" };
            if href.is_empty() || !done {
                format!("<span class=\"{class}\">{}</span>", label)
            } else {
                format!("<a class=\"{class}\" href=\"{}\">{}</a>", esc(&href), label)
            }
        })
        .collect::<Vec<_>>()
        .join("<span class=\"chain-sep\">→</span>");
    format!("<div class=\"chain\">{items}</div>")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn table_takes_markup_and_keeps_escaped_cells() {
        let empty = table(Lang::En, &["A"], vec![], "msg_no_records");
        assert!(empty.contains("No records"));
        // Headers arrive pre-formatted by `tr()` and cells carry badges/links, so the
        // table inserts both as markup; every caller passes data through `esc()`.
        let t = table(
            Lang::En,
            &["<b>H</b>"],
            vec![vec![esc("<x>")]],
            "msg_no_records",
        );
        assert!(t.contains("<b>H</b>"));
        assert!(t.contains("<td>&lt;x&gt;</td>"));
        // only a list with an actions column marks that cell
        assert!(!t.contains("class=\"acts\""));
        let a = table_actions(
            Lang::En,
            &["<b>H</b>", "<b>A</b>"],
            vec![vec!["1".into(), "x".into()]],
            "msg_no_records",
        );
        assert!(a.contains("<td>1</td><td class=\"acts\">x</td>"));
    }

    #[test]
    fn row_actions_are_icons_only() {
        // the label the button used to show becomes the tooltip; the cell itself is
        // just the circle, so nothing but the svg is left inside the anchor
        let a = icon_link("/suppliers/1/edit", "pencil", &tr(Lang::En, "a_edit"));
        assert!(a.contains("class=\"btn-icon\""));
        assert!(a.contains("title=\"Edit · تعديل\""));
        assert!(a.contains("aria-label=\"Edit · تعديل\""));
        assert!(a.contains("<svg"));
        assert!(!a.contains("\"lat\"") && !a.contains("\"ar\""));
        let p = icon_post_of("/suppliers/1/delete", "trash", &tr(Lang::En, "a_delete"), "");
        assert!(p.contains("class=\"btn-icon danger\""));
        assert!(p.contains("<svg") && !p.contains("\"lat\""));
        // a delete asks through the dialog; with no record named it says nothing extra
        assert!(p.contains("data-confirm=\"Delete · حذف\""));
        assert!(!p.contains("data-confirm-note"));
        let named =
            icon_post_of("/customers/9/delete", "trash", &tr(Lang::En, "a_delete"), "Acme <b>");
        assert!(named.contains("data-confirm-note=\"Acme &lt;b&gt;\""));

        // the dialog itself: two answers, cancel first, and it names nothing until a
        // form fills the subject in
        let d = confirm_dialog(Lang::En);
        assert!(d.starts_with("<dialog class=\"modal\" id=\"confirm-dialog\""));
        assert!(d.contains("data-confirm-cancel") && d.contains("data-confirm-ok"));
        assert!(d.contains("data-confirm-subject hidden"));
        // red head, pale red body, red foot: three bands, in that order
        let head = d.find("modal-head").unwrap();
        let body = d.find("modal-body").unwrap();
        let foot = d.find("modal-actions").unwrap();
        assert!(head < body && body < foot);
        // the two answers are plain buttons: the script asks the dialog, not a form
        assert_eq!(d.matches("type=\"button\"").count(), 2);
    }

    #[test]
    fn sidebar_groups_fold() {
        // each group is a <details>: the header is its <summary>, so the browser
        // folds the links without any script
        let s = nav_section("buy", "Procure-to-Pay", &["<a class=\"nav-item\">x</a>"]);
        assert!(s.starts_with("<details class=\"nav-section\" data-nav-sec=\"buy\" open>"));
        assert!(s.contains("<summary class=\"nav-title\" title=\"Procure-to-Pay\">"));
        // the chevron is a Papicons glyph, filled with the text colour
        assert!(s.contains("<svg viewBox=\"0 0 24 24\" fill=\"currentColor\""));
        assert!(s.contains("<div class=\"nav-links\"><a class=\"nav-item\">x</a></div>"));
        assert!(s.ends_with("</details>"));
    }

    #[test]
    fn every_glyph_carries_its_path_data() {
        // the generator once glued `<path` to `d=`, so the trash drew nothing; a
        // glyph without path data in its body would fail here
        for name in [
            "dashboard", "suppliers", "po", "receipt", "bill", "wallet", "customers",
            "quote", "order", "delivery", "products", "credit", "ticket", "admin",
            "events", "broker", "cog", "logout", "panel", "chevron", "eye", "pencil",
            "trash", "file", "print", "globe", "unknown-glyph",
        ] {
            let svg = icon(name);
            assert!(svg.contains(" d=\""), "{} has no path data", name);
            assert!(!svg.contains("pathd="), "{} has a glued attribute", name);
        }
    }

    #[test]
    fn settings_offer_a_size_for_every_icon() {
        // the fourth step of the icon row is the current one, and every step keeps
        // the page the menu was opened from
        let m = settings_menu(Lang::En, "/customers", "m", "m", "l");
        assert!(m.contains("Icon size"));
        assert!(m.contains("href=\"/icon/l?next=%2Fcustomers\" aria-current=\"true\">L</a>"));
        assert!(m.contains("href=\"/icon/s?next=%2Fcustomers\">S</a>"));
        assert!(m.contains("href=\"/icon/xl?next=%2Fcustomers\">XL</a>"));
    }

    #[test]
    fn one_footer_bar_serves_every_page() {
        // both names, the running version and the credit: the app shell, the sign-in
        // page and the error page all print this same content
        let f = foot(Lang::En);
        assert!(f.contains("S2C MAROC") && f.contains(&tr_ar("app_name")));
        assert!(f.contains(concat!("v", env!("CARGO_PKG_VERSION"))));
        assert!(f.contains("(koul)"));
        assert!(f.starts_with("<span class=\"foot-id\">"));
    }

    #[test]
    fn badges_map_to_tones() {
        assert_eq!(status_tone("st_paid"), "green");
        assert_eq!(status_tone("st_overdue"), "red");
        assert!(status_badge(Lang::Fr, "st_draft").contains("Brouillon"));
    }
    #[test]
    fn a_list_table_heads_with_tools_then_columns() {
        let t = list_table(
            Lang::En,
            &["<b>H</b>", "<b>N</b>"],
            vec![vec!["1".into(), "2".into()]],
            "msg_no_records",
            "<form class=\"search\"></form>",
        );
        // two header rows: the tools span every column, the second names them
        assert!(t.contains(
            "<thead><tr class=\"thead-tools\"><th colspan=\"2\"><div class=\"page-actions\"><form class=\"search\"></form></div></th></tr><tr><th><b>H</b></th><th><b>N</b></th></tr></thead>"
        ));
        // an empty list keeps both rows, so a search that found nothing can be cleared
        let e = list_table(
            Lang::En,
            &["<b>H</b>"],
            vec![],
            "msg_no_records",
            "<form></form>",
        );
        assert!(e.contains("<th colspan=\"1\"><div class=\"page-actions\"><form></form></div></th>"));
        assert!(e.contains("<th><b>H</b></th>"));
        assert!(e.contains("<td colspan=\"1\"><div class=\"empty\">"));
        assert!(e.contains("No records"));
        // a table without tools gets no first row
        assert!(!table(Lang::En, &["<b>H</b>"], vec![vec!["1".into()]], "msg_no_records")
            .contains("thead-tools"));
    }

}
