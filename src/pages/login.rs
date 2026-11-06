//! Sign-in, sign-out and language switching.

use axum::extract::{Path, Query, State};
use axum::http::header::{CACHE_CONTROL, CONTENT_TYPE, SET_COOKIE};
use axum::http::{HeaderMap, HeaderValue};
use axum::response::{Html, IntoResponse, Redirect, Response};
use leptos::prelude::*;
use mongodb::bson::doc;
use std::collections::HashMap;

use crate::auth::{self, COOKIE_TOKEN};
use crate::error::AppError;
use crate::form::Pairs;
use crate::i18n::{Lang, tr};
use crate::state::AppState;
use crate::util::{esc, urlenc};

fn shell(
    lang: Lang,
    brand: &str,
    body: String,
    fs_lat: &'static str,
    fs_ar: &'static str,
) -> Response {
    let settings = crate::views::settings_menu(lang, "/login", fs_lat, fs_ar);
    let tagline1 = tr(lang, "app_tagline_1");
    let tagline2 = tr(lang, "app_tagline_2");
    let view = view! {
        <html lang=lang.code() dir=lang.dir() data-fs-lat=fs_lat data-fs-ar=fs_ar>
            <head>
                <meta charset="utf-8"/>
                <meta name="viewport" content="width=device-width, initial-scale=1"/>
                <title>{format!("{} · {}", tr(lang, "a_login"), brand)}</title>
                <link rel="stylesheet" href=format!("/static/app.css?v={}", crate::routes::asset_token())/>
                <link rel="icon" href="/static/favicon.svg" type="image/svg+xml"/>
            </head>
            <body class=format!("login-body lang-{}", lang.code())>
                <div class="login-wrap">
                    <div class="login-top" inner_html=settings></div>
                    <div class="login-brand">
                        <div class="brand-mark lg">"S2C"</div>
                        <h1>{esc(brand)}</h1>
                        <p>{tagline1}</p>
                        <br/>
                        <p>{tagline2}</p>
                    </div>
                    <div inner_html=body></div>
                </div>
            </body>
        </html>
    };
    let mut resp = Html(format!("<!doctype html>{}", view.to_html())).into_response();
    resp.headers_mut()
        .insert(CACHE_CONTROL, HeaderValue::from_static("no-store"));
    resp.headers_mut().insert(
        CONTENT_TYPE,
        HeaderValue::from_static("text/html; charset=utf-8"),
    );
    resp
}

pub async fn form(
    State(st): State<AppState>,
    headers: HeaderMap,
    Query(q): Query<HashMap<String, String>>,
) -> Result<Response, AppError> {
    let lang = auth::cookie_lang(&headers, &st.cfg.default_locale);
    let company = st
        .db
        .companies()
        .find_one(doc! {})
        .await?
        .map(|c| c.name)
        .unwrap_or_else(|| st.cfg.company_name.clone());

    let error = q.get("m").map(|m| tr(lang, m)).unwrap_or_default();
    let next = q.get("next").cloned().unwrap_or_else(|| "/".into());
    let err_html = if error.is_empty() {
        String::new()
    } else {
        format!("<div class=\"flash err\">{}</div>", esc(&error))
    };

    let body = format!(
        "<div class=\"login-card\">{err_html}\
          <form method=\"post\" action=\"/login\" class=\"stack\">\
            <input type=\"hidden\" name=\"next\" value=\"{next}\"/>\
            <label class=\"field\"><span>{user}</span><input name=\"username\" autocomplete=\"username\" required autofocus value=\"{last_user}\"/></label>\
            <label class=\"field\"><span>{pass}</span><input name=\"password\" type=\"password\" autocomplete=\"current-password\" required/></label>\
            <button class=\"btn btn-primary btn-block\" type=\"submit\">{submit}</button>\
          </form>\
          <p class=\"hint\">{default_hint}</p>\
        </div>",
        user = esc(&tr(lang, "lbl_username")),
        pass = esc(&tr(lang, "lbl_password")),
        submit = esc(&tr(lang, "a_login")),
        next = esc(&next),
        last_user = esc(q.get("u").map(|s| s.as_str()).unwrap_or("")),
        default_hint = esc(&tr(lang, "m_sample_admin")),
    );
    Ok(shell(
        lang,
        &company,
        body,
        auth::cookie_size(&headers, auth::COOKIE_FS_LAT),
        auth::cookie_size(&headers, auth::COOKIE_FS_AR),
    ))
}

pub async fn submit(
    State(st): State<AppState>,
    headers: HeaderMap,
    pairs: Pairs,
) -> Result<Response, AppError> {
    let _lang = auth::cookie_lang(&headers, &st.cfg.default_locale);
    let username = pairs.get("username");
    let password = pairs.get("password");
    let next = pairs.opt("next").unwrap_or_else(|| "/".into());
    let next = if next.starts_with('/') { next } else { "/".into() };

    let user = st
        .db
        .users()
        .find_one(doc! { "username": &username })
        .await?;

    let Some(user) = user.filter(|u| u.active) else {
        return Ok(Redirect::to(&format!(
            "/login?m=msg_invalid_credentials&k=err&u={}",
            urlenc(&username)
        ))
        .into_response());
    };

    if !auth::verify_password(&password, &user.password_salt, &user.password_hash) {
        tracing::warn!(username = %username, "failed sign-in attempt");
        return Ok(Redirect::to(&format!(
            "/login?m=msg_invalid_credentials&k=err&u={}",
            urlenc(&username)
        ))
        .into_response());
    }

    let token = auth::issue_token(&user, &st.cfg)?;
    let cookie = auth::set_cookie(
        COOKIE_TOKEN,
        &token,
        Some(st.cfg.jwt_expires),
        st.cfg.cookie_secure,
        true,
    );
    // Signing in must not reset the duo/font choices made in the header.

    let mut resp = Redirect::to(&next).into_response();
    if let Ok(v) = axum::http::HeaderValue::from_str(&cookie) {
        resp.headers_mut().append(SET_COOKIE, v);
    }
    Ok(resp)
}

pub async fn logout(State(st): State<AppState>) -> Response {
    let cookie = auth::set_cookie(COOKIE_TOKEN, "", Some(0), st.cfg.cookie_secure, true);
    let mut resp = Redirect::to("/login").into_response();
    if let Ok(v) = axum::http::HeaderValue::from_str(&cookie) {
        resp.headers_mut().append(SET_COOKIE, v);
    }
    resp
}

pub async fn switch_lang(
    State(st): State<AppState>,
    Path(code): Path<String>,
    Query(q): Query<HashMap<String, String>>,
) -> Response {
    let lang = Lang::from_code(&code).unwrap_or(Lang::En);
    let next = q
        .get("next")
        .filter(|n| n.starts_with('/'))
        .cloned()
        .unwrap_or_else(|| "/".into());
    let cookie = auth::set_cookie(
        auth::COOKIE_LANG,
        lang.code(),
        Some(31_536_000),
        st.cfg.cookie_secure,
        false,
    );
    let mut resp = Redirect::to(&next).into_response();
    if let Ok(v) = axum::http::HeaderValue::from_str(&cookie) {
        resp.headers_mut().append(SET_COOKIE, v);
    }
    resp
}

/// `/font/{face}/{size}` — the Settings › Font size options. `face` is `lat`
/// (Comfortaa) or `ar` (VIP Rawy); `size` is `s`, `m` or `l`.
pub async fn switch_font(
    State(st): State<AppState>,
    Path((face, size)): Path<(String, String)>,
    Query(q): Query<HashMap<String, String>>,
) -> Response {
    let name = match face.as_str() {
        "ar" => auth::COOKIE_FS_AR,
        _ => auth::COOKIE_FS_LAT,
    };
    let value = match size.as_str() {
        "s" => "s",
        "l" => "l",
        _ => "m",
    };
    let next = q
        .get("next")
        .filter(|n| n.starts_with('/'))
        .cloned()
        .unwrap_or_else(|| "/".into());
    let cookie = auth::set_cookie(name, value, Some(31_536_000), st.cfg.cookie_secure, false);
    let mut resp = Redirect::to(&next).into_response();
    if let Ok(v) = axum::http::HeaderValue::from_str(&cookie) {
        resp.headers_mut().append(SET_COOKIE, v);
    }
    resp
}
