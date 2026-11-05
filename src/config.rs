//! Runtime configuration from environment variables (see `.env.example`).

use std::env;

#[derive(Debug, Clone)]
pub struct Config {
    pub port: u16,
    pub mongo_uri: String,
    pub mongo_db: String,
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

        Self {
            port,
            mongo_uri: var("MONGO_URI", "mongodb://localhost:27017/?directConnection=true"),
            mongo_db: var("MONGO_DB", "stockinvoice"),
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
