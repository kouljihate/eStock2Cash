//! StockInvoice — Procure-to-Pay / Quote-to-Cash for small businesses.
//!
//! A single-binary Leptos-SSR (Axum) application backed by MongoDB, with
//! RabbitMQ domain events mirrored into Mongo and a JSON API for the Android
//! client.

mod api;
mod auth;
mod config;
mod db;
mod error;
mod events;
mod form;
mod i18n;
mod models;
mod pages;
mod pdf;
mod routes;
mod services;
mod state;
mod ticket;
mod util;
mod views;

use std::sync::Arc;

use crate::config::Config;
use crate::state::AppState;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    match dotenvy::dotenv() {
        Ok(path) => tracing::debug!(path = %path.display(), "loaded .env"),
        Err(e) if e.not_found() => {}
        Err(e) => eprintln!("warning: .env not loaded: {e} (quote values that contain spaces)"),
    }

    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "stockinvoice=info,tower_http=warn,lapin=warn".into()),
        )
        .init();

    let cfg = Arc::new(Config::from_env());
    tracing::info!(
        version = env!("CARGO_PKG_VERSION"),
        port = cfg.port,
        mongo_db = %cfg.mongo_db,
        company = %cfg.company_name,
        rabbitmq = cfg.rabbit_enabled(),
        "starting StockInvoice"
    );

    let client = db::connect(&cfg.mongo_uri, &cfg.mongo_db).await?;
    let handle = db::handle(client, &cfg.mongo_db).await;
    db::ensure_indexes(&handle.db).await?;
    db::seed(&handle.db, &cfg).await?;

    let bus = events::Bus::new(cfg.clone());
    events::spawn_consumer(handle.clone(), cfg.clone());

    let state = AppState::new(handle, cfg.clone(), bus);
    let app = routes::router(state);

    let addr = cfg.bind_addr();
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    tracing::info!(%addr, "listening");
    axum::serve(listener, app).await?;
    Ok(())
}
