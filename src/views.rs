//! Server-rendered shell (Leptos `view!`) and reusable HTML fragments.

use axum::http::header::{CACHE_CONTROL, CONTENT_DISPOSITION, CONTENT_TYPE};
use axum::http::HeaderValue;
use axum::response::{Html, IntoResponse, Response};
use leptos::prelude::*;

use crate::auth::Ctx;
use crate::i18n::{Lang, tr, tr_ar, tr_lat};
use crate::util::{esc, money};

fn icon(name: &str) -> String {
    let path = match name {
        "dashboard" => r#"<rect x="3" y="3" width="7" height="7" rx="1"/><rect x="14" y="3" width="7" height="7" rx="1"/><rect x="3" y="14" width="7" height="7" rx="1"/><rect x="14" y="14" width="7" height="7" rx="1"/>"#,
        "suppliers" => r#"<path d="M3 17h18"/><path d="M5 17V8l7-4 7 4v9"/><path d="M9 17v-5h6v5"/>"#,
        "po" => r#"<path d="M6 3h9l4 4v14H6z"/><path d="M9 12h7M9 16h5M9 8h4"/>"#,
        "receipt" => r#"<path d="M5 3h14v18l-3-2-2 2-2-2-2 2-2-2-3 2z"/><path d="M9 8h6M9 12h6"/>"#,
        "bill" => r#"<path d="M4 5h16v14H4z"/><path d="M8 9h8M8 13h5"/><path d="M16 17h2"/>"#,
        "wallet" => r#"<path d="M3 7h15a3 3 0 0 1 3 3v7H3z"/><path d="M3 7V5h13"/><circle cx="17" cy="13" r="1.5"/>"#,
        "customers" => r#"<circle cx="9" cy="8" r="3"/><path d="M3 20c0-3.3 2.7-6 6-6s6 2.7 6 6"/><path d="M16 11a3 3 0 1 0 0-6"/><path d="M17 20c0-2.2-.9-4.2-2.3-5.7"/>"#,
        "quote" => r#"<path d="M4 4h16v12H8l-4 4z"/><path d="M8 9h8M8 12h5"/>"#,
        "order" => r#"<path d="M4 6h16l-1.5 12H5.5z"/><path d="M8 6a4 4 0 0 1 8 0"/>"#,
        "delivery" => r#"<path d="M3 16V7h11v9"/><path d="M14 10h4l3 3v3h-7"/><circle cx="7" cy="18" r="2"/><circle cx="17" cy="18" r="2"/>"#,
        "products" => r#"<path d="M12 3l8 4.5v9L12 21l-8-4.5v-9z"/><path d="M4 7.5l8 4.5 8-4.5M12 12v9"/>"#,
        "credit" => r#"<rect x="3" y="6" width="18" height="12" rx="2"/><path d="M3 10h18"/><path d="M7 15h4"/>"#,
        "ticket" => r#"<path d="M4 9V6h16v3a2 2 0 0 0 0 4v3H4v-3a2 2 0 0 0 0-4z"/><path d="M12 7v10" stroke-dasharray="2 2"/>"#,
        "admin" => r#"<circle cx="12" cy="12" r="3"/><path d="M12 3v3M12 18v3M3 12h3M18 12h3M5.6 5.6l2.1 2.1M16.3 16.3l2.1 2.1M18.4 5.6l-2.1 2.1M7.7 16.3l-2.1 2.1"/>"#,
        "events" => r#"<path d="M3 12h4l2-6 3 12 2-6h4"/>"#,
        "globe" => r#"<circle cx="12" cy="12" r="9"/><path d="M3 12h18"/><path d="M12 3c2.4 2.6 3.7 5.6 3.7 9s-1.3 6.4-3.7 9c-2.4-2.6-3.7-5.6-3.7-9S9.6 5.6 12 3z"/>"#,
        "broker" => r#"<path d="M9 7V3M15 7V3"/><rect x="6" y="7" width="12" height="7" rx="2"/><path d="M9 14v3a3 3 0 0 0 3 3 3 3 0 0 0 3-3v-3"/>"#,
        "cog" => r#"<circle cx="12" cy="12" r="3.2"/><path d="M12 2.5v3M12 18.5v3M2.5 12h3M18.5 12h3M5.2 5.2l2.1 2.1M16.7 16.7l2.1 2.1M18.8 5.2l-2.1 2.1M7.3 16.7l-2.1 2.1"/>"#,
        "logout" => r#"<path d="M15 4h4v16h-4"/><path d="M10 8l-4 4 4 4"/><path d="M6 12h9"/>"#,
        "panel" => r#"<rect x="3" y="4" width="18" height="16" rx="2"/><path d="M9 4v16"/>"#,
        _ => r#"<circle cx="12" cy="12" r="8"/>"#,
    };
    format!(
        "<svg viewBox=\"0 0 24 24\" fill=\"none\" stroke=\"currentColor\" stroke-width=\"1.6\" \
         stroke-linecap=\"round\" stroke-linejoin=\"round\">{path}</svg>"
    )
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

fn nav_section(title: &str, items: &[&str]) -> String {
    format!(
        "<div class=\"nav-section\"><span class=\"nav-title\">{}</span>{}</div>",
        title,
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

    out.push_str(&nav_section(
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
        &tr(l, "lbl_stock"),
        &[
            &nav_item(on("/products"), "/products", &tr(l, "nav_products"), "products"),
            &nav_item(on("/credit"), "/credit", &tr(l, "nav_credit"), "credit"),
            &nav_item(on("/tickets"), "/tickets", &tr(l, "nav_tickets"), "ticket"),
        ],
    ));
    out.push_str(&nav_section(
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
/// It holds two submenus: `Languages`, listing the two duos (AR/EN and AR/FR)
/// one per row, and `Font size`, which splits the choice per font — Comfortaa for
/// the Latin half of every label, VIP Rawy for the Arabic half.
pub fn settings_menu(lang: Lang, next: &str, fs_lat: &str, fs_ar: &str) -> String {
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

    format!(
        "<details class=\"settings\"><summary class=\"settings-trigger\" title=\"{title}\">{gear}<span class=\"settings-name\">{label}</span></summary><div class=\"settings-panel\"><details class=\"sub\"><summary class=\"sub-head\">{langs}<span class=\"sub-caret\">▸</span></summary><div class=\"sub-panel\">{en}{fr}</div></details><details class=\"sub\"><summary class=\"sub-head\">{font_size}<span class=\"sub-caret\">▸</span></summary><div class=\"sub-panel\"><div class=\"fsrow\"><span class=\"fsname\">Comfortaa</span>{lat_sizes}</div><div class=\"fsrow\"><span class=\"fsname ar\">VIP Rawy</span>{ar_sizes}</div></div></details></div></details>",
        label = tr(lang, "lbl_settings"),
        title = esc(&crate::i18n::tr_text(lang, "lbl_settings")),
        gear = icon("cog"),
        langs = tr(lang, "lbl_languages"),
        font_size = tr(lang, "lbl_font_size"),
        en = duo(Lang::En, "EN"),
        fr = duo(Lang::Fr, "FR"),
        lat_sizes = sizes("lat", fs_lat),
        ar_sizes = sizes("ar", fs_ar),
    )
}

/// Runs in `<head>`: restores the collapsed sidebar before the first paint so the
/// rail never flashes open on a page load.
pub const NAV_STATE_SCRIPT: &str = "try{if(localStorage.getItem('s2c-nav')==='closed'){document.documentElement.classList.add('nav-closed')}}catch(e){}";

/// Full page shell.
pub fn page(ctx: &Ctx, title: &Title, body: String) -> String {
    let lang = ctx.lang;
    let nav = nav_html(ctx);
    let settings = settings_menu(lang, &ctx.path, ctx.fs_lat, ctx.fs_ar);
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
    let app_name = tr(lang, "app_name");
    let sign_out_text = crate::i18n::tr_text(lang, "a_logout");
    let signed_in = tr(lang, "m_signed_in_as");
    let signed_in_ar = tr_ar("m_signed_in_as");
    let nav_toggle_text = crate::i18n::tr_text(lang, "lbl_toggle_menu");
    let year = chrono::Utc::now().format("%Y").to_string();

    let view = view! {
        <html lang=lang.code() dir=lang.dir() data-fs-lat=ctx.fs_lat data-fs-ar=ctx.fs_ar>
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
                            <div class="brand-mark">"S2C"</div>
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
                            <span class="muted" inner_html=format!("{} {}", signed_in, username.clone())></span>
                        </div>
                        <div class="tb-col tb-mid">
                            <form method="post" action="/logout" class="inline">
                                <button class="btn btn-ghost btn-icon" type="submit" title=sign_out_text.clone() aria-label=sign_out_text.clone() inner_html=icon("logout")></button>
                            </form>
                            <div class="tb-settings" inner_html=settings></div>
                        </div>
                        <div class="tb-col tb-ar">
                            <h1 class="ar">{title_ar}</h1>
                            <span class="muted" inner_html=format!("<span class=\"ar\">{}</span> {}", signed_in_ar, username)></span>
                        </div>
                    </header>
                    <div class="flash-wrap" inner_html=flash_html></div>
                    <main class="content" inner_html=body></main>
                    <footer class="appfoot">
                        <span class="foot-id">
                            <span inner_html=app_name.clone()></span>
                            <span class="foot-ver">{ concat!("v", env!("CARGO_PKG_VERSION")) }</span>
                        </span>
                        <span class="foot-year">"© " {year}</span>
                    </footer>
                </div>
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
    if rows.is_empty() {
        return format!(
            "<div class=\"empty\">{}</div>",
            tr(lang, empty_key)
        );
    }
    let head = headers
        .iter()
        .map(|h| format!("<th>{}</th>", h))
        .collect::<String>();
    let body = rows
        .into_iter()
        .map(|cells| {
            format!(
                "<tr>{}</tr>",
                cells
                    .into_iter()
                    .map(|c| format!("<td>{c}</td>"))
                    .collect::<String>()
            )
        })
        .collect::<String>();
    format!(
        "<div class=\"table-wrap\"><table><thead><tr>{head}</tr></thead><tbody>{body}</tbody></table></div>"
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
    }

    #[test]
    fn badges_map_to_tones() {
        assert_eq!(status_tone("st_paid"), "green");
        assert_eq!(status_tone("st_overdue"), "red");
        assert!(status_badge(Lang::Fr, "st_draft").contains("Brouillon"));
    }
}
