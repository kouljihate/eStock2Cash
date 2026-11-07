//! Administration: company profile, invoice branding, users, event log and
//! the RabbitMQ monitor.

use axum::extract::{Path, Query, State};
use axum::response::{IntoResponse, Response};
use futures_util::TryStreamExt;
use mongodb::bson::doc;
use std::collections::HashMap;

use crate::auth::{self, Ctx};
use crate::error::AppError;
use crate::events;
use crate::form::Pairs;
use crate::i18n::{Lang, tr};
use crate::models::{Role, User};
use crate::state::AppState;
use crate::util::{esc, now_iso};
use crate::views;

pub async fn index(ctx: Ctx) -> Result<Response, AppError> {
    let l = ctx.lang;
    let c = ctx.company.clone();

    let mut cur = ctx
        .state
        .db
        .users()
        .find(doc! { "company_id": &c.id })
        .sort(doc! { "created_at": 1 })
        .await?;
    let mut rows = Vec::new();
    let mut user_count = 0;
    while let Some(u) = cur.try_next().await? {
        user_count += 1;
        rows.push(vec![
            format!(
                "{}<div class=\"muted small\">{}</div>",
                esc(&u.full_name),
                esc(&u.username)
            ),
            format!("<span class=\"badge green\">{}</span>", tr(l, "lbl_role")),
            esc(&u.locale),
            views::icon_post_of(
                &format!("/admin/users/{}/delete", u.id),
                "trash",
                &tr(l, "a_delete"),
                &format!("{} · {}", u.full_name, u.username),
            ),
        ]);
    }

    let company_form = format!(
        "<form method=\"post\" action=\"/admin/company\" class=\"form\">\
           <div class=\"form-grid\">{}{}{}{}{}{}</div>\
           <label class=\"field field-wide\"><span>{}</span><textarea name=\"logo\" rows=\"4\" placeholder=\"data:image/jpeg;base64,...\">{}</textarea></label>\
           <label class=\"field field-wide\"><span>{}</span><textarea name=\"address\" rows=\"2\">{}</textarea></label>\
           {}</form>",
        views::input(&tr(l, "lbl_name"), "name", &c.name, "text", true),
        views::input(&tr(l, "lbl_legal_id"), "legal_name", &c.legal_name, "text", false),
        views::input(&tr(l, "lbl_tax_id"), "tax_id", &c.tax_id, "text", false),
        views::input(&tr(l, "lbl_currency"), "currency", &c.currency, "text", true),
        views::input(&tr(l, "lbl_phone"), "phone", &c.phone, "text", false),
        views::input(&tr(l, "lbl_email"), "email", &c.email, "email", false),
        tr(l, "lbl_logo"),
        esc(&c.logo),
        tr(l, "lbl_address"),
        esc(&c.address),
        views::form_actions(&tr(l, "a_save"), "/admin", &tr(l, "a_back")),
    );

    let t = &c.template;
    let template_form = format!(
        "<form method=\"post\" action=\"/admin/template\" class=\"form\">\
           <div class=\"form-grid\">{}{}{}{}{}</div>\
           <div class=\"form-grid\">{}{}{}</div>\
           {}</form>",
        views::input(&tr(l, "lbl_primary_color"), "primary", &t.primary, "color", false),
        views::input(&tr(l, "lbl_accent_color"), "accent", &t.accent, "color", false),
        views::input(&tr(l, "lbl_font"), "font", &t.font, "text", false),
        views::input(&tr(l, "lbl_header"), "header", &t.header, "text", false),
        views::input(&tr(l, "lbl_footer"), "footer", &t.footer, "text", false),
        views::textarea(&tr(l, "lbl_notes"), "terms", &t.terms),
        views::checkbox(&tr(l, "lbl_logo"), "show_logo", t.show_logo),
        views::checkbox(&tr(l, "lbl_tax"), "show_tax", t.show_tax),
        views::form_actions(&tr(l, "a_save"), "/admin", &tr(l, "a_back")),
    );

    let user_form = format!(
        "<form method=\"post\" action=\"/admin/users\" class=\"form\">\
           <div class=\"form-grid\">{}{}{}{}</div>{}</form>",
        views::input(&tr(l, "lbl_username"), "username", "", "text", true),
        views::input(&tr(l, "lbl_full_name"), "full_name", "", "text", false),
        views::input(&tr(l, "lbl_password"), "password", "", "password", true),
        views::select(
            &tr(l, "lbl_language"),
            "locale",
            &Lang::all()
                .iter()
                .map(|lang| (lang.code().to_string(), lang.label().to_string(), *lang == l))
                .collect::<Vec<_>>(),
            false
        ),
        views::form_actions(&tr(l, "a_add"), "/admin", &tr(l, "a_back")),
    );

    let password_form = format!(
        "<form method=\"post\" action=\"/admin/password\" class=\"form\">\
           <div class=\"form-grid\">{}{}</div>{}</form>",
        views::input(&tr(l, "lbl_current_password"), "current", "", "password", true),
        views::input(&tr(l, "lbl_new_password"), "new", "", "password", true),
        views::form_actions(&tr(l, "a_change_password"), "/admin", &tr(l, "a_back")),
    );

    let body = format!(
        "{}{}{}{}{}{}",
        super::page_head(
            &ctx,
            "nav_admin",
            &format!("{} · {}", c.name, c.currency),
            vec![
                views::link_btn("/admin/events", &tr(l, "nav_events")),
                views::link_btn("/admin/broker", &tr(l, "nav_broker")),
            ]
        ),
        views::card(&tr(l, "m_company_profile"), company_form),
        views::card(&tr(l, "m_invoice_template"), template_form),
        views::card(
            &tr(l, "m_users"),
            format!(
                "{}{}",
                views::table_actions(
                    l,
                    &[
                        &tr(l, "lbl_name"),
                        &tr(l, "lbl_role"),
                        &tr(l, "lbl_language"),
                        &tr(l, "lbl_actions"),
                    ],
                    rows,
                    "msg_no_records"
                ),
                user_form
            )
        ),
        views::card(&tr(l, "a_change_password"), password_form),
        views::notice(l, "m_sample_admin", "info"),
    );
    let _ = user_count;
    let title = views::Title::of(l, "nav_admin");
    Ok(views::respond(views::page(&ctx, &title, body)))
}

pub async fn company_update(ctx: Ctx, pairs: Pairs) -> Result<Response, AppError> {
    let mut c = ctx.company.clone();
    c.name = crate::util::if_empty(&pairs.get("name"), &c.name);
    c.legal_name = pairs.get("legal_name");
    c.tax_id = pairs.get("tax_id");
    c.currency = crate::util::if_empty(&pairs.get("currency"), &c.currency);
    c.phone = pairs.get("phone");
    c.email = pairs.get("email");
    c.address = pairs.get("address");
    let logo = pairs.get("logo");
    if !logo.is_empty() {
        if !logo.starts_with("data:image/jpeg") && !looks_like_base64_jpeg(&logo) {
            tracing::warn!("logo rejected: only JPEG data is supported for PDF branding");
        } else {
            c.logo = logo;
        }
    }
    c.updated_at = now_iso();
    ctx.state
        .db
        .companies()
        .replace_one(doc! { "_id": &c.id }, &c)
        .await?;
    Ok(ctx.back("/admin", "msg_saved", "ok").into_response())
}

fn looks_like_base64_jpeg(s: &str) -> bool {
    // "/9j/" is the base64 encoding of the JPEG SOI marker.
    s.trim_start().starts_with("/9j/")
}

pub async fn template_update(ctx: Ctx, pairs: Pairs) -> Result<Response, AppError> {
    let mut c = ctx.company.clone();
    c.template.primary = crate::util::if_empty(&pairs.get("primary"), &c.template.primary);
    c.template.accent = crate::util::if_empty(&pairs.get("accent"), &c.template.accent);
    c.template.font = crate::util::if_empty(&pairs.get("font"), &c.template.font);
    c.template.header = pairs.get("header");
    c.template.footer = pairs.get("footer");
    c.template.terms = pairs.get("terms");
    c.template.show_logo = pairs.flag("show_logo");
    c.template.show_tax = pairs.flag("show_tax");
    c.updated_at = now_iso();
    ctx.state
        .db
        .companies()
        .replace_one(doc! { "_id": &c.id }, &c)
        .await?;
    Ok(ctx.back("/admin", "msg_saved", "ok").into_response())
}

pub async fn password_change(ctx: Ctx, pairs: Pairs) -> Result<Response, AppError> {
    let user = ctx.state.db.user(&ctx.claims.sub).await?;
    let current = pairs.get("current");
    let new = pairs.get("new");
    if !auth::verify_password(&current, &user.password_salt, &user.password_hash) {
        return Ok(ctx.back("/admin", "msg_invalid_credentials", "err").into_response());
    }
    if new.chars().count() < 6 {
        return Ok(ctx.back("/admin", "msg_required", "err").into_response());
    }
    let (salt, hash) = auth::hash_password(&new)?;
    ctx.state
        .db
        .users()
        .update_one(
            doc! { "_id": &user.id },
            doc! { "$set": { "password_salt": salt, "password_hash": hash, "updated_at": now_iso() } },
        )
        .await?;
    Ok(ctx.back("/admin", "msg_password_changed", "ok").into_response())
}

pub async fn user_create(ctx: Ctx, pairs: Pairs) -> Result<Response, AppError> {
    let username = pairs.get("username");
    let password = pairs.get("password");
    if username.is_empty() || password.chars().count() < 6 {
        return Ok(ctx.back("/admin", "msg_required", "err").into_response());
    }
    let (salt, hash) = auth::hash_password(&password)?;
    let now = now_iso();
    let user = User {
        id: crate::util::new_id(),
        company_id: ctx.company.id.clone(),
        username: username.clone(),
        full_name: crate::util::if_empty(&pairs.get("full_name"), &username),
        password_hash: hash,
        password_salt: salt,
        role: Role::LocalAdmin,
        locale: crate::util::if_empty(&pairs.get("locale"), ctx.lang.code()),
        active: true,
        created_at: now.clone(),
        updated_at: now,
    };
    match ctx.state.db.users().insert_one(&user).await {
        Ok(_) => Ok(ctx.back("/admin", "msg_saved", "ok").into_response()),
        Err(e) if e.to_string().contains("duplicate key") => {
            Ok(ctx.back("/admin", "msg_duplicate", "err").into_response())
        }
        Err(e) => Err(e.into()),
    }
}

pub async fn user_delete(ctx: Ctx, Path(id): Path<String>) -> Result<Response, AppError> {
    if id == ctx.claims.sub {
        // Never lock yourself out of the only account.
        return Ok(ctx.back("/admin", "msg_error", "err").into_response());
    }
    ctx.state
        .db
        .users()
        .delete_one(doc! { "_id": &id, "company_id": &ctx.company.id })
        .await?;
    Ok(ctx.back("/admin", "msg_deleted", "ok").into_response())
}

pub async fn events(
    ctx: Ctx,
    Query(q): Query<HashMap<String, String>>,
) -> Result<Response, AppError> {
    let l = ctx.lang;
    let search = q.get("q").cloned().unwrap_or_default();
    let mut cur = ctx
        .state
        .db
        .event_log()
        .find(doc! {})
        .sort(doc! { "received_at": -1 })
        .limit(200)
        .await?;
    let mut rows = Vec::new();
    while let Some(ev) = cur.try_next().await? {
        if !super::row_matches(&search, &[&ev.routing_key, &ev.event_type, &ev.received_at]) {
            continue;
        }
        rows.push(vec![
            format!(
                "<span class=\"muted small\">{}</span>",
                esc(&events::event_age(&ev.received_at))
            ),
            format!("<code>{}</code>", esc(&ev.routing_key)),
            esc(&ev.event_type),
            format!("<span class=\"muted small\">{}</span>", esc(&ev.received_at)),
        ]);
    }

    let tools = super::search_form(l, "/admin/events", &search, "");
    let body = format!(
        "{}{}",
        if ctx.state.cfg.rabbit_enabled() {
            String::new()
        } else {
            views::notice(l, "m_no_broker", "warn")
        },
        views::card(
            &tr(l, "m_event_log"),
            views::list_table(
                l,
                &[
                    &tr(l, "lbl_days"),
                    &tr(l, "lbl_reference"),
                    &tr(l, "lbl_document"),
                    &tr(l, "lbl_date"),
                ],
                rows,
                "msg_no_records",
                &tools
            )
        )
    );
    let title = views::Title::of(l, "nav_events");
    Ok(views::respond(views::page(&ctx, &title, body)))
}

pub async fn broker(
    ctx: Ctx,
    Query(q): Query<HashMap<String, String>>,
) -> Result<Response, AppError> {
    let l = ctx.lang;
    let search = q.get("q").cloned().unwrap_or_default();
    let info = events::broker_info(&ctx.state.cfg).await;

    let status = if !info.configured {
        format!("<span class=\"badge slate\">{}</span>", tr(l, "m_not_configured"))
    } else if info.online {
        format!("<span class=\"badge green\">{}</span>", tr(l, "m_connected"))
    } else {
        format!("<span class=\"badge red\">{}</span>", tr(l, "m_disconnected"))
    };

    let mut body = format!(
        "{}",
        views::card(
            &tr(l, "m_rabbitmq"),
            super::meta_grid(vec![
                (tr(l, "m_management_api"), status),
                ("Version".into(), esc(&info.version)),
                ("Node".into(), esc(&info.node)),
                (tr(l, "m_messages"), esc(&info.total_messages.to_string())),
            ])
        )
    );

    if let Some(err) = &info.error {
        body.push_str(&format!(
            "<div class=\"notice warn\">{}<div class=\"mono small\">{}</div></div>",
            tr(l, "m_broker_offline"),
            esc(err)
        ));
    }

    let rows = info
        .queues
        .iter()
        .filter(|queue| super::row_matches(&search, &[&queue.name]))
        .map(|queue| {
            vec![
                format!("<code>{}</code>", esc(&queue.name)),
                format!("<span class=\"num\">{}</span>", queue.messages),
                format!("<span class=\"num\">{}</span>", queue.consumers),
                esc(&queue.state),
            ]
        })
        .collect::<Vec<_>>();
    let tools = super::search_form(l, "/admin/broker", &search, "");

    body.push_str(&views::card(
        &tr(l, "m_queues"),
        views::list_table(
            l,
            &[
                &tr(l, "lbl_name"),
                &tr(l, "m_messages"),
                &tr(l, "lbl_contact"),
                &tr(l, "lbl_status"),
            ],
            rows,
            "msg_no_records",
            &tools,
        ),
    ));
    body.push_str(&views::notice(l, "m_ticket_hint", "info"));

    let title = views::Title::of(l, "nav_broker");
    Ok(views::respond(views::page(&ctx, &title, body)))
}

#[allow(dead_code)]
pub async fn unused_state(State(_st): State<AppState>) {}
