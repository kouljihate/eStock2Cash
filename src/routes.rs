//! HTTP routing: public pages, the authenticated web app and static assets.

use axum::http::{header, HeaderValue, StatusCode};
use axum::middleware;
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::Router;
use tower_http::compression::CompressionLayer;
use tower_http::trace::TraceLayer;

use crate::auth;
use crate::pages;
use crate::state::AppState;

const APP_CSS: &str = include_str!("../static/app.css");
const APP_JS: &str = include_str!("../static/app.js");
const FAVICON: &str = include_str!("../static/favicon.svg");

/// Cache key for the embedded assets: the crate version plus a checksum of the
/// stylesheet and script. The version alone is not enough — a rebuild that keeps
/// the same version, or a page loaded while one was being edited, would keep an
/// old sheet in the browser for an hour.
pub fn asset_token() -> &'static str {
    static TOKEN: std::sync::OnceLock<String> = std::sync::OnceLock::new();
    TOKEN.get_or_init(|| {
        let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
        for byte in APP_CSS.as_bytes().iter().chain(APP_JS.as_bytes()) {
            hash ^= u64::from(*byte);
            hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
        }
        format!("{}-{:08x}", env!("CARGO_PKG_VERSION"), (hash >> 32) as u32 ^ hash as u32)
    })
}

/// Fonts live on disk rather than in the binary: they are licensed assets the
/// operator may replace (VIP Rawy), and a 100-400 KB face does not belong in a
/// `include_str!`. Only known font extensions are served, and only from the
/// fonts directory.
async fn font_asset(axum::extract::Path(file): axum::extract::Path<String>) -> Response {
    let mime = match file.rsplit_once('.').map(|(_, ext)| ext.to_ascii_lowercase()) {
        Some(ext) if ext == "ttf" => "font/ttf",
        Some(ext) if ext == "otf" => "font/otf",
        Some(ext) if ext == "woff" => "font/woff",
        Some(ext) if ext == "woff2" => "font/woff2",
        _ => return StatusCode::NOT_FOUND.into_response(),
    };
    let plain_name = !file.is_empty()
        && file.len() < 96
        && !file.contains('/')
        && !file.contains('\\')
        && !file.contains(':')
        && !file.contains("..");
    if !plain_name {
        return StatusCode::NOT_FOUND.into_response();
    }
    match tokio::fs::read(std::path::Path::new("static/fonts").join(&file)).await {
        Ok(bytes) => (
            StatusCode::OK,
            [
                (header::CONTENT_TYPE, HeaderValue::from_static(mime)),
                (
                    header::CACHE_CONTROL,
                    HeaderValue::from_static("public, max-age=31536000, immutable"),
                ),
            ],
            bytes,
        )
            .into_response(),
        Err(err) => {
            tracing::debug!(file, %err, "font not found");
            StatusCode::NOT_FOUND.into_response()
        }
    }
}

fn asset(body: &'static str, mime: &'static str) -> Response {
    (
        StatusCode::OK,
        [
            (header::CONTENT_TYPE, HeaderValue::from_static(mime)),
            (
                header::CACHE_CONTROL,
                HeaderValue::from_static("public, max-age=3600"),
            ),
        ],
        body,
    )
        .into_response()
}

pub fn router(state: AppState) -> Router {
    let public = Router::new()
        .route(
            "/login",
            get(pages::login::form).post(pages::login::submit),
        )
        .route("/lang/{code}", get(pages::login::switch_lang))
        .route("/font/{face}/{size}", get(pages::login::switch_font))
        .route("/icon/{size}", get(pages::login::switch_icon))
        .route("/static/app.css", get(|| async { asset(APP_CSS, "text/css; charset=utf-8") }))
        .route(
            "/static/app.js",
            get(|| async { asset(APP_JS, "application/javascript; charset=utf-8") }),
        )
        .route(
            "/static/favicon.svg",
            get(|| async { asset(FAVICON, "image/svg+xml") }),
        )
        .route("/static/fonts/{file}", get(font_asset));

    let protected = Router::new()
        .route("/", get(pages::dashboard::index))
        .route("/logout", post(pages::login::logout))
        // Suppliers
        .route("/suppliers", get(pages::parties::suppliers).post(pages::parties::supplier_create))
        .route("/suppliers/new", get(pages::parties::supplier_new))
        .route("/suppliers/{id}/edit", get(pages::parties::supplier_edit))
        .route(
            "/suppliers/{id}",
            post(pages::parties::supplier_update),
        )
        .route(
            "/suppliers/{id}/delete",
            post(pages::parties::supplier_delete),
        )
        // Customers
        .route("/customers", get(pages::parties::customers).post(pages::parties::customer_create))
        .route("/customers/new", get(pages::parties::customer_new))
        .route("/customers/{id}", get(pages::parties::customer_detail).post(pages::parties::customer_update))
        .route("/customers/{id}/edit", get(pages::parties::customer_edit))
        .route("/customers/{id}/delete", post(pages::parties::customer_delete))
        // Products
        .route("/products", get(pages::catalog::products).post(pages::catalog::product_create))
        .route("/products/new", get(pages::catalog::product_new))
        .route("/products/{id}", post(pages::catalog::product_update))
        .route("/products/{id}/edit", get(pages::catalog::product_edit))
        .route("/products/{id}/delete", post(pages::catalog::product_delete))
        .route("/products/{id}/movements", get(pages::catalog::product_movements))
        .route("/products/{id}/adjust", post(pages::catalog::product_adjust))
        // Procure-to-pay
        .route(
            "/purchase-orders",
            get(pages::purchasing::po_list).post(pages::purchasing::po_create),
        )
        .route("/purchase-orders/new", get(pages::purchasing::po_new))
        .route("/purchase-orders/{id}", get(pages::purchasing::po_detail))
        .route("/purchase-orders/{id}/action", post(pages::purchasing::po_send))
        .route("/purchase-orders/{id}/cancel", post(pages::purchasing::po_cancel))
        .route("/purchase-orders/{id}/delete", post(pages::purchasing::po_delete))
        .route(
            "/goods-receipts",
            get(pages::purchasing::gr_list).post(pages::purchasing::gr_create),
        )
        .route("/goods-receipts/new", get(pages::purchasing::gr_new))
        .route("/goods-receipts/{id}", get(pages::purchasing::gr_detail))
        .route("/goods-receipts/{id}/post", post(pages::purchasing::gr_post))
        .route(
            "/supplier-invoices",
            get(pages::purchasing::si_list).post(pages::purchasing::si_create),
        )
        .route("/supplier-invoices/new", get(pages::purchasing::si_new))
        .route("/supplier-invoices/{id}", get(pages::purchasing::si_detail))
        .route("/supplier-invoices/{id}/issue", post(pages::purchasing::si_issue))
        .route("/supplier-invoices/{id}/cancel", post(pages::purchasing::si_cancel))
        .route("/supplier-invoices/{id}/delete", post(pages::purchasing::si_delete))
        .route("/supplier-invoices/{id}/pdf", get(pages::purchasing::si_pdf))
        // Quote-to-cash
        .route("/quotes", get(pages::sales::quote_list).post(pages::sales::quote_create))
        .route("/quotes/new", get(pages::sales::quote_new))
        .route("/quotes/{id}", get(pages::sales::quote_detail))
        .route("/quotes/{id}/send", post(pages::sales::quote_send))
        .route("/quotes/{id}/accept", post(pages::sales::quote_accept))
        .route("/quotes/{id}/reject", post(pages::sales::quote_reject))
        .route(
            "/customer-orders",
            get(pages::sales::order_list).post(pages::sales::order_create),
        )
        .route("/customer-orders/new", get(pages::sales::order_new))
        .route("/customer-orders/{id}", get(pages::sales::order_detail))
        .route("/customer-orders/{id}/confirm", post(pages::sales::order_confirm))
        .route(
            "/delivery-notes",
            get(pages::sales::delivery_list).post(pages::sales::delivery_create),
        )
        .route("/delivery-notes/new", get(pages::sales::delivery_new))
        .route("/delivery-notes/{id}", get(pages::sales::delivery_detail))
        .route("/delivery-notes/{id}/post", post(pages::sales::delivery_post))
        .route(
            "/customer-invoices",
            get(pages::invoicing::ci_list).post(pages::invoicing::ci_create),
        )
        .route("/customer-invoices/new", get(pages::invoicing::ci_new))
        .route("/customer-invoices/{id}", get(pages::invoicing::ci_detail))
        .route("/customer-invoices/{id}/issue", post(pages::invoicing::ci_issue))
        .route("/customer-invoices/{id}/cancel", post(pages::invoicing::ci_cancel))
        .route("/customer-invoices/{id}/pdf", get(pages::invoicing::ci_pdf))
        // Payments & credit
        .route("/payments", get(pages::invoicing::payment_list).post(pages::invoicing::payment_create))
        .route("/payments/new", get(pages::invoicing::payment_new))
        .route("/credit", get(pages::invoicing::credit))
        // Cash registry
        .route(
            "/cash-registry",
            get(pages::cash::desk).post(pages::cash::sell),
        )
        // Tickets
        .route("/tickets", get(pages::tickets::list).post(pages::tickets::create))
        .route("/tickets/new", get(pages::tickets::new))
        .route("/tickets/{id}", get(pages::tickets::detail))
        .route("/tickets/{id}/escpos", get(pages::tickets::escpos))
        .route("/tickets/{id}/pdf", get(pages::tickets::pdf))
        // Admin
        .route("/admin", get(pages::admin::index))
        .route("/admin/company", post(pages::admin::company_update))
        .route("/admin/template", post(pages::admin::template_update))
        .route("/admin/password", post(pages::admin::password_change))
        .route("/admin/users", post(pages::admin::user_create))
        .route("/admin/users/{id}/delete", post(pages::admin::user_delete))
        .route("/settings", get(pages::settings::index))
        .route("/settings/database", post(pages::settings::database_save))
        .route("/admin/events", get(pages::admin::events))
        .route("/admin/broker", get(pages::admin::broker))
        .route_layer(middleware::from_fn(auth::same_origin));

    Router::new()
        .merge(public)
        .merge(protected)
        .nest("/api/v1", crate::api::router())
        .with_state(state)
        .layer(CompressionLayer::new())
        .layer(TraceLayer::new_for_http())
}
