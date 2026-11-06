//! Authentication (Argon2id + JWT), the request context extractor, RBAC and
//! the session/CSRF middleware.

use axum::extract::{FromRequestParts, Request};
use axum::http::header::{AUTHORIZATION, COOKIE, HOST, ORIGIN};
use axum::http::request::Parts;
use axum::http::{HeaderMap, StatusCode};
use axum::middleware::Next;
use axum::response::{IntoResponse, Redirect, Response};
use jsonwebtoken::{Algorithm, DecodingKey, EncodingKey, Header, Validation, decode, encode};
use rand::RngExt;
use serde::{Deserialize, Serialize};

use crate::config::Config;
use crate::error::AppError;
use crate::i18n::Lang;
use crate::models::{Company, Role, User};
use crate::state::AppState;
use crate::util::{ct_eq, esc, urlenc};

pub const COOKIE_TOKEN: &str = "si_token";
pub const COOKIE_LANG: &str = "si_lang";
/// Text size of each face: `s`, `m` (default) or `l`.
pub const COOKIE_FS_LAT: &str = "si_fs_lat";
pub const COOKIE_FS_AR: &str = "si_fs_ar";

fn hex(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        s.push_str(&format!("{b:02x}"));
    }
    s
}

/// Argon2id hash with a fresh random salt. Returns `(salt_hex, hash_hex)`.
pub fn hash_password(password: &str) -> Result<(String, String), AppError> {
    use argon2::Argon2;
    let mut salt = [0u8; 16];
    rand::rng().fill(&mut salt);
    let salt_hex = hex(&salt);
    let mut out = [0u8; 32];
    Argon2::default()
        .hash_password_into(password.as_bytes(), salt_hex.as_bytes(), &mut out)
        .map_err(|e| AppError::Internal(format!("password hash failed: {e}")))?;
    Ok((salt_hex, hex(&out)))
}

pub fn verify_password(password: &str, salt_hex: &str, hash_hex: &str) -> bool {
    use argon2::Argon2;
    let mut out = [0u8; 32];
    if Argon2::default()
        .hash_password_into(password.as_bytes(), salt_hex.as_bytes(), &mut out)
        .is_err()
    {
        return false;
    }
    ct_eq(hex(&out).as_bytes(), hash_hex.as_bytes())
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Claims {
    /// user id
    pub sub: String,
    /// company id (tenant scope)
    pub cid: String,
    pub un: String,
    pub nm: String,
    pub role: String,
    pub exp: usize,
    pub iat: usize,
}

impl Claims {
    #[allow(dead_code)]
    pub fn role(&self) -> Role {
        Role::parse(&self.role).unwrap_or(Role::LocalAdmin)
    }

    /// RBAC: the local administrator has full access to every module.
    /// (One profile is supported today; the permission enum keeps the checks
    /// in place for additional profiles.)
    #[allow(dead_code)]
    pub fn can(&self, _permission: Permission) -> bool {
        matches!(self.role(), Role::LocalAdmin)
    }
}

#[derive(Clone, Copy, Debug)]
#[allow(dead_code)]
pub enum Permission {
    Read,
    Write,
    Admin,
}

pub fn issue_token(user: &User, cfg: &Config) -> Result<String, AppError> {
    let now = chrono::Utc::now().timestamp() as usize;
    let claims = Claims {
        sub: user.id.clone(),
        cid: user.company_id.clone(),
        un: user.username.clone(),
        nm: user.full_name.clone(),
        role: user.role.as_str().to_string(),
        iat: now,
        exp: now + cfg.jwt_expires.max(60) as usize,
    };
    encode(
        &Header::new(Algorithm::HS256),
        &claims,
        &EncodingKey::from_secret(cfg.jwt_secret.as_bytes()),
    )
    .map_err(|e| AppError::Internal(format!("token signing failed: {e}")))
}

pub fn verify_token(token: &str, cfg: &Config) -> Result<Claims, AppError> {
    let data = decode::<Claims>(
        token,
        &DecodingKey::from_secret(cfg.jwt_secret.as_bytes()),
        &Validation::new(Algorithm::HS256),
    )
    .map_err(|_| AppError::Unauthorized)?;
    Ok(data.claims)
}

fn header_value<'a>(headers: &'a HeaderMap, name: &str) -> Option<&'a str> {
    headers.get(name).and_then(|v| v.to_str().ok())
}

pub fn token_from_headers(headers: &HeaderMap) -> Option<String> {
    if let Some(auth) = header_value(headers, AUTHORIZATION.as_str()) {
        if let Some(rest) = auth.strip_prefix("Bearer ") {
            if !rest.trim().is_empty() {
                return Some(rest.trim().to_string());
            }
        }
    }
    let cookies = header_value(headers, COOKIE.as_str())?;
    cookies.split(';').find_map(|pair| {
        let mut it = pair.trim().splitn(2, '=');
        let k = it.next()?;
        let v = it.next().unwrap_or("");
        if k == COOKIE_TOKEN && !v.is_empty() {
            Some(v.to_string())
        } else {
            None
        }
    })
}

pub fn claims_from_headers(headers: &HeaderMap, cfg: &Config) -> Option<Claims> {
    token_from_headers(headers).and_then(|t| verify_token(&t, cfg).ok())
}

pub fn cookie_lang(headers: &HeaderMap, default: &str) -> Lang {
    if let Some(cookies) = header_value(headers, COOKIE.as_str()) {
        for pair in cookies.split(';') {
            let mut it = pair.trim().splitn(2, '=');
            if it.next() == Some(COOKIE_LANG) {
                if let Some(code) = it.next() {
                    if let Some(l) = Lang::from_code(code) {
                        return l;
                    }
                }
            }
        }
    }
    Lang::from_code(default).unwrap_or(Lang::En)
}

/// Text size for one face, read from `si_fs_lat` / `si_fs_ar`.
pub fn cookie_size(headers: &HeaderMap, name: &str) -> &'static str {
    if let Some(cookies) = header_value(headers, COOKIE.as_str()) {
        for pair in cookies.split(';') {
            let mut it = pair.trim().splitn(2, '=');
            if it.next() == Some(name) {
                return match it.next().map(str::trim) {
                    Some("s") => "s",
                    Some("l") => "l",
                    _ => "m",
                };
            }
        }
    }
    "m"
}

pub fn set_cookie(name: &str, value: &str, max_age: Option<i64>, secure: bool, http_only: bool) -> String {
    let mut c = format!("{name}={value}; Path=/; SameSite=Lax");
    if http_only {
        c.push_str("; HttpOnly");
    }
    if let Some(age) = max_age {
        c.push_str(&format!("; Max-Age={age}"));
    }
    if secure {
        c.push_str("; Secure");
    }
    c
}

pub fn query_param(query: Option<&str>, key: &str) -> Option<String> {
    let q = query?;
    for pair in q.split('&') {
        let mut it = pair.splitn(2, '=');
        if it.next() == Some(key) {
            let raw = it.next().unwrap_or("");
            return Some(decode_component(raw));
        }
    }
    None
}

/// Public wrapper for URL-component decoding (used by the broker config parser).
pub fn decode_public(s: &str) -> String {
    decode_component(s)
}

fn decode_component(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out: Vec<u8> = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'%' if i + 2 < bytes.len() => {
                let hex = std::str::from_utf8(&bytes[i + 1..i + 3]).unwrap_or("");
                if let Ok(b) = u8::from_str_radix(hex, 16) {
                    out.push(b);
                    i += 3;
                } else {
                    out.push(bytes[i]);
                    i += 1;
                }
            }
            b'+' => {
                out.push(b' ');
                i += 1;
            }
            b => {
                out.push(b);
                i += 1;
            }
        }
    }
    String::from_utf8_lossy(&out).to_string()
}

/// Everything a page needs: session claims, language, tenant company, flash.
#[derive(Clone)]
pub struct Ctx {
    pub claims: Claims,
    pub lang: Lang,
    pub company: Company,
    pub state: AppState,
    pub flash: Option<Flash>,
    pub path: String,
    /// Text size of the Latin and the Arabic face, chosen in Settings.
    pub fs_lat: &'static str,
    pub fs_ar: &'static str,
}

#[derive(Clone)]
pub struct Flash {
    pub message: String,
    pub kind: String,
}

impl Ctx {
    #[allow(dead_code)]
    pub fn tr(&self, key: &str) -> String {
        crate::i18n::tr(self.lang, key)
    }

    /// Build a `?m=<key>&k=<kind>` redirect target.
    pub fn back(&self, path: &str, msg_key: &str, kind: &str) -> Redirect {
        let sep = if path.contains('?') { '&' } else { '?' };
        Redirect::to(&format!(
            "{path}{sep}m={}&k={}",
            urlenc(msg_key),
            if kind == "err" { "err" } else { "ok" }
        ))
    }
}

impl FromRequestParts<AppState> for Ctx {
    type Rejection = Response;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let cfg = state.cfg.clone();
        let claims = claims_from_headers(&parts.headers, &cfg).ok_or_else(|| {
            let next = parts
                .uri
                .path_and_query()
                .map(|p| p.as_str())
                .unwrap_or("/");
            if next.starts_with("/api/") {
                (StatusCode::UNAUTHORIZED, "unauthorized").into_response()
            } else {
                Redirect::to(&format!("/login?next={}", urlenc(next))).into_response()
            }
        })?;

        let company = state
            .db
            .company(&claims.cid)
            .await
            .map_err(|_| (StatusCode::UNAUTHORIZED, "session expired").into_response())?;

        let lang = auth_lang(&parts.headers, &cfg.default_locale);
        let query = parts.uri.query();
        let flash = query_param(query, "m").map(|m| Flash {
            message: crate::i18n::tr(lang, &m),
            kind: query_param(query, "k").unwrap_or_else(|| "ok".into()),
        });

        Ok(Ctx {
            claims,
            lang,
            company,
            state: state.clone(),
            flash,
            path: parts.uri.path().to_string(),
            fs_lat: cookie_size(&parts.headers, COOKIE_FS_LAT),
            fs_ar: cookie_size(&parts.headers, COOKIE_FS_AR),
        })
    }
}

fn auth_lang(headers: &HeaderMap, default: &str) -> Lang {
    cookie_lang(headers, default)
}

/// Reject cross-site form submissions about as cheaply as possible.
pub async fn same_origin(req: Request, next: Next) -> Response {
    if matches!(
        *req.method(),
        axum::http::Method::POST | axum::http::Method::PUT | axum::http::Method::PATCH | axum::http::Method::DELETE
    ) {
        if let (Some(origin), Some(host)) = (
            header_value(req.headers(), ORIGIN.as_str()),
            header_value(req.headers(), HOST.as_str()),
        ) {
            let origin_host = origin
                .split("://")
                .nth(1)
                .unwrap_or(origin)
                .trim_end_matches('/');
            if !host.is_empty() && origin_host != host {
                return (StatusCode::FORBIDDEN, "cross-origin request rejected").into_response();
            }
        }
    }
    next.run(req).await
}

#[allow(dead_code)]
pub fn permission_denied_html(lang: Lang) -> String {
    format!(
        "<div class=\"card\"><h2>{}</h2><p>{}</p></div>",
        esc(&crate::i18n::tr(lang, "msg_error")),
        esc("Your profile does not allow this action.")
    )
}
