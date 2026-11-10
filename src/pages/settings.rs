//! Runtime settings: which database this installation talks to.
//!
//! The choice is saved to `app-settings.json` next to the binary and applied at
//! once — the shared handle is swapped, so the next request reads and writes the
//! new database without restarting the app.

use axum::response::{IntoResponse, Response};

use crate::auth::Ctx;
use crate::config::{self, SETTINGS_FILE};
use crate::error::AppError;
use crate::form::Pairs;
use crate::i18n::tr;
use crate::util::esc;
use crate::views;

pub async fn index(ctx: Ctx) -> Result<Response, AppError> {
    let l = ctx.lang;
    let s = ctx
        .state
        .db_settings
        .read()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .clone();

    let mode = views::select(
        &tr(l, "lbl_db_mode"),
        "mode",
        &[
            ("local".to_string(), tr(l, "m_db_local"), !s.is_server()),
            ("server".to_string(), tr(l, "m_db_server"), s.is_server()),
        ],
        true,
    );

    let form = format!(
        "<form method=\"post\" action=\"/settings/database\" class=\"form\">\
           <div class=\"form-grid\">{mode}{host}{port}{db}{user}{pass}</div>\
           <p class=\"muted small\">{hint}</p>\
           {actions}</form>",
        host = views::input(&tr(l, "lbl_db_host"), "host", &s.host, "text", false),
        port = views::input(
            &tr(l, "lbl_db_port"),
            "port",
            &s.port_or_default().to_string(),
            "number",
            false
        ),
        db = views::input(&tr(l, "lbl_db_name"), "db_name", &s.db_name, "text", false),
        user = views::input(&tr(l, "lbl_db_user"), "username", &s.username, "text", false),
        pass = views::input(&tr(l, "lbl_db_password"), "password", "", "password", false),
        hint = tr(l, "m_db_hint"),
        actions = views::form_actions(&tr(l, "a_save"), "/admin", &tr(l, "a_back")),
    );

    let current = format!(
        "<p>{label} <strong>{where_now}</strong></p>\
         <p>{name_label} <strong>{name}</strong></p>\
         <p class=\"muted small\">{file_label} <code>{file}</code></p>",
        label = tr(l, "m_db_current"),
        where_now = esc(&if s.is_server() {
            tr(l, "m_db_server")
        } else {
            tr(l, "m_db_local")
        }),
        name_label = tr(l, "lbl_db_name"),
        name = esc(&s.db_name_or_default()),
        file_label = tr(l, "m_db_file"),
        file = esc(SETTINGS_FILE),
    );

    let body = format!(
        "{}{}{}",
        super::page_head(
            &ctx,
            "nav_settings",
            &tr(l, "nav_settings"),
            Vec::new()
        ),
        views::card(&tr(l, "m_database"), form),
        views::card(&tr(l, "m_db_current_card"), current),
    );
    let title = views::Title::of(l, "nav_settings");
    Ok(views::respond(views::page(&ctx, &title, body)))
}

pub async fn database_save(ctx: Ctx, pairs: Pairs) -> Result<Response, AppError> {
    let cfg = ctx.state.cfg.clone();
    let previous = ctx
        .state
        .db_settings
        .read()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .clone();
    let mut next = previous.clone();
    next.mode = if pairs.get("mode").eq_ignore_ascii_case("server") {
        "server".into()
    } else {
        "local".into()
    };
    next.host = crate::util::if_empty(&pairs.get("host"), &previous.host);
    next.port = pairs
        .int("port", i64::from(previous.port_or_default()))
        .clamp(1, 65535) as u16;
    next.db_name = crate::util::if_empty(&pairs.get("db_name"), &previous.db_name);
    next.username = pairs.get("username");
    let password = pairs.get("password");
    if !password.is_empty() {
        // An empty box keeps the stored password: it is never sent back to the page.
        next.password = password;
    }

    // Try the new database before anything is changed. An unreachable address leaves
    // the app on the database that works instead of locking the shop out of its data.
    match crate::db::prepare(&next, &cfg).await {
        Ok(database) => {
            if let Err(problem) = config::save_db_settings(&next) {
                tracing::error!(error = %problem, "database settings could not be saved");
                return Ok(ctx.back("/settings", "msg_db_save_failed", "err").into_response());
            }
            let same = next.uri() == previous.uri()
                && next.db_name_or_default() == previous.db_name_or_default();
            ctx.state.db.swap(database);
            *ctx
                .state
                .db_settings
                .write()
                .unwrap_or_else(|poisoned| poisoned.into_inner()) = next.clone();
            tracing::info!(target = %next.summary(), "database changed");
            if same {
                Ok(ctx.back("/settings", "msg_db_saved", "ok").into_response())
            } else {
                // A different database has different users: this session no longer exists.
                Ok(ctx.back("/login", "msg_db_switched", "ok").into_response())
            }
        }
        Err(e) => {
            tracing::warn!(error = %e, target = %next.summary(), "the database in the form could not be reached");
            Ok(ctx.back("/settings", "msg_db_unreachable", "err").into_response())
        }
    }
}
