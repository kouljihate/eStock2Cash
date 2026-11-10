//! Runtime configuration from environment variables (see `.env.example`).

use std::env;

use serde::{Deserialize, Serialize};

/// The database settings the operator can change from the app itself. They are
/// kept in a small JSON file next to the binary, so the choice survives restarts
/// and can be edited on a machine with no browser.
pub const SETTINGS_FILE: &str = "app-settings.json";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DbSettings {
    /// `local` (MongoDB on this computer) or `server` (MongoDB at an IP address).
    #[serde(default = "default_mode")]
    pub mode: String,
    #[serde(default = "default_host")]
    pub host: String,
    #[serde(default = "default_port")]
    pub port: u16,
    #[serde(default = "default_db_name")]
    pub db_name: String,
    #[serde(default)]
    pub username: String,
    #[serde(default)]
    pub password: String,
}

fn default_mode() -> String {
    "local".into()
}

fn default_host() -> String {
    "127.0.0.1".into()
}

fn default_port() -> u16 {
    27017
}

fn default_db_name() -> String {
    "stockinvoice".into()
}

impl Default for DbSettings {
    fn default() -> Self {
        Self {
            mode: default_mode(),
            host: default_host(),
            port: default_port(),
            db_name: default_db_name(),
            username: String::new(),
            password: String::new(),
        }
    }
}

impl DbSettings {
    pub fn is_server(&self) -> bool {
        self.mode.eq_ignore_ascii_case("server")
    }

    /// A host someone typed into the box: an IP, or a whole `mongodb://host:port`
    /// pasted by mistake. Only the host survives; the port box and the scheme are
    /// added back below.
    pub fn host_clean(&self) -> String {
        let raw = self.host.trim();
        let raw = raw.split('/').next().unwrap_or(raw);
        let raw = raw.trim_start_matches("mongodb://");
        let host = raw.split(':').next().unwrap_or(raw).trim();
        if host.is_empty() {
            default_host()
        } else {
            host.to_string()
        }
    }

    pub fn port_or_default(&self) -> u16 {
        if self.port == 0 {
            default_port()
        } else {
            self.port
        }
    }

    pub fn db_name_or_default(&self) -> String {
        let name = self.db_name.trim();
        if name.is_empty() {
            default_db_name()
        } else {
            name.to_string()
        }
    }

    /// The connection string the app should use. Local always means this computer,
    /// so a server address left over from an earlier setup cannot send a shop's data
    /// somewhere else by accident.
    pub fn uri(&self) -> String {
        let host = if self.is_server() {
            self.host_clean()
        } else {
            default_host()
        };
        let mut uri = String::from("mongodb://");
        let with_user = self.is_server() && !self.username.trim().is_empty();
        if with_user {
            uri.push_str(&format!(
                "{}:{}@",
                percent(&self.username),
                percent(&self.password)
            ));
        }
        uri.push_str(&format!("{host}:{}/", self.port_or_default()));

        let mut query = vec![
            "directConnection=true".to_string(),
            "serverSelectionTimeoutMS=4000".to_string(),
        ];
        if with_user {
            query.push(format!("authSource={}", percent(&self.db_name_or_default())));
        }
        format!("{uri}?{}", query.join("&"))
    }

    /// What to show on the settings page — never the password.
    pub fn summary(&self) -> String {
        let host = if self.is_server() {
            self.host_clean()
        } else {
            default_host()
        };
        format!(
            "{}:{}/{}",
            host,
            self.port_or_default(),
            self.db_name_or_default()
        )
    }
}

fn percent(value: &str) -> String {
    let mut out = String::new();
    for byte in value.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(byte as char)
            }
            _ => out.push_str(&format!("%{byte:02X}")),
        }
    }
    out
}

pub fn load_db_settings() -> DbSettings {
    match std::fs::read_to_string(SETTINGS_FILE) {
        Ok(text) => serde_json::from_str(&text).unwrap_or_else(|e| {
            tracing::warn!(error = %e, file = SETTINGS_FILE, "settings file is not valid JSON — using the local database");
            DbSettings::default()
        }),
        Err(_) => DbSettings::default(),
    }
}

pub fn save_db_settings(settings: &DbSettings) -> Result<(), String> {
    let text = serde_json::to_string_pretty(settings).map_err(|e| e.to_string())?;
    std::fs::write(SETTINGS_FILE, format!("{text}\n"))
        .map_err(|e| format!("cannot write {SETTINGS_FILE}: {e}"))
}

#[derive(Debug, Clone)]
pub struct Config {
    pub port: u16,
    pub mongo_uri: String,
    pub mongo_db: String,
    /// What the settings page shows and edits; `mongo_uri`/`mongo_db` are the
    /// effective values built from it.
    pub db: DbSettings,
    #[allow(dead_code)]
    pub secret_key: String,
    pub jwt_secret: String,
    pub jwt_expires: i64,
    pub cookie_secure: bool,
    pub rabbit_url: Option<String>,
    pub rabbit_mgmt_url: Option<String>,
    pub admin_username: String,
    pub admin_password: String,
    pub company_name: String,
    pub default_locale: String,
}

fn var(key: &str, default: &str) -> String {
    env::var(key)
        .ok()
        .filter(|v| !v.trim().is_empty())
        .unwrap_or_else(|| default.to_string())
}

impl Config {
    pub fn from_env() -> Self {
        // A port of 0 would silently bind a random port; fall back to 8080.
        let port = match var("PORT", "8080").parse::<u16>().unwrap_or(8080) {
            0 => 8080,
            p => p,
        };
        let jwt_expires = var("JWT_ACCESS_TOKEN_EXPIRES", "3600")
            .parse::<i64>()
            .unwrap_or(3600);

        // A weak/absent signing key would silently weaken every session.
        let secret_key = var("SECRET_KEY", "");
        let secret_key = if secret_key.chars().count() >= 32 {
            secret_key
        } else {
            tracing::warn!(
                "SECRET_KEY missing or shorter than 32 chars - using an ephemeral key for this run"
            );
            random_secret()
        };

        let jwt_secret = var("JWT_SECRET_KEY", "");
        let jwt_secret = if jwt_secret.chars().count() >= 32 {
            jwt_secret
        } else {
            tracing::warn!(
                "JWT_SECRET_KEY missing or shorter than 32 chars - using an ephemeral key for this run"
            );
            random_secret()
        };

        let rabbit_url = env::var("RABBITMQ_URL")
            .ok()
            .filter(|v| !v.trim().is_empty());

        // The saved file decides where the data lives, because that is the copy the
        // Settings page can change. The environment only has a say on the first run: it
        // seeds the file, so what the page shows is what the app is really using. Delete
        // the file to pick the environment up again.
        let mut db = load_db_settings();
        let first_run = !std::path::Path::new(SETTINGS_FILE).exists();
        let env_uri = env::var("MONGO_URI")
            .ok()
            .filter(|v| !v.trim().is_empty());
        let env_db = env::var("MONGO_DB")
            .ok()
            .filter(|v| !v.trim().is_empty());
        if first_run {
            if let Some(name) = &env_db {
                db.db_name = name.clone();
            }
            if let Err(e) = save_db_settings(&db) {
                tracing::warn!(error = %e, file = SETTINGS_FILE, "could not write the settings file");
            }
        }
        let mongo_db = db.db_name_or_default();
        let mongo_uri = if first_run {
            env_uri.unwrap_or_else(|| db.uri())
        } else {
            db.uri()
        };

        Self {
            port,
            mongo_uri,
            mongo_db,
            db,
            secret_key,
            jwt_secret,
            jwt_expires,
            cookie_secure: var("COOKIE_SECURE", "0") == "1",
            rabbit_url,
            rabbit_mgmt_url: env::var("RABBITMQ_MGMT_URL").ok().filter(|v| !v.is_empty()),
            admin_username: var("ADMIN_USERNAME", "admin"),
            admin_password: var("ADMIN_PASSWORD", "looklook"),
            company_name: var("COMPANY_NAME", "S2C MAROC"),
            default_locale: var("DEFAULT_LOCALE", "en"),
        }
    }

    pub fn bind_addr(&self) -> String {
        format!("0.0.0.0:{}", self.port)
    }

    pub fn rabbit_enabled(&self) -> bool {
        self.rabbit_url.is_some()
    }
}

fn random_secret() -> String {
    use rand::RngExt;
    let mut rng = rand::rng();
    (0..48)
        .map(|_| {
            let charset = b"abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789";
            charset[rng.random_range(0..charset.len())] as char
        })
        .collect()
}
