//! Error type for web handlers (HTML) plus a JSON wrapper for the mobile API.

use axum::http::StatusCode;
use axum::response::{Html, IntoResponse, Response};

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("{0}")]
    NotFound(String),
    #[error("{0}")]
    BadRequest(String),
    #[error("authentication required")]
    Unauthorized,
    #[error("forbidden")]
    #[allow(dead_code)]
    Forbidden,
    #[error("database error: {0}")]
    Db(String),
    #[error("{0}")]
    Internal(String),
}

impl From<mongodb::error::Error> for AppError {
    fn from(e: mongodb::error::Error) -> Self {
        tracing::error!(error = %e, "mongodb error");
        AppError::Db(e.to_string())
    }
}

impl From<mongodb::bson::ser::Error> for AppError {
    fn from(e: mongodb::bson::ser::Error) -> Self {
        AppError::Internal(e.to_string())
    }
}

impl From<mongodb::bson::de::Error> for AppError {
    fn from(e: mongodb::bson::de::Error) -> Self {
        AppError::Internal(e.to_string())
    }
}

impl AppError {
    pub fn status(&self) -> StatusCode {
        match self {
            AppError::NotFound(_) => StatusCode::NOT_FOUND,
            AppError::BadRequest(_) => StatusCode::BAD_REQUEST,
            AppError::Unauthorized => StatusCode::UNAUTHORIZED,
            AppError::Forbidden => StatusCode::FORBIDDEN,
            AppError::Db(_) | AppError::Internal(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }

    pub fn message(&self) -> String {
        match self {
            AppError::Db(_) | AppError::Internal(_) => {
                "An internal error occurred. Please try again or check the server log.".into()
            }
            other => other.to_string(),
        }
    }
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let status = self.status();
        let message = self.message();
        let body = format!(
            "<!doctype html><html><head><meta charset=\"utf-8\"><title>{status}</title>\
             <link rel=\"stylesheet\" href=\"/static/app.css\"></head>\
             <body class=\"errpage\"><div class=\"errbox\"><h1>{status}</h1>\
             <p>{msg}</p><p><a class=\"btn\" href=\"/\">Dashboard</a></p></div></body></html>",
            status = status.as_u16(),
            msg = crate::util::esc(&message)
        );
        let mut resp = Html(body).into_response();
        *resp.status_mut() = status;
        resp
    }
}

/// JSON envelope used by `/api/v1/*`.
pub struct ApiError(pub AppError);

impl From<AppError> for ApiError {
    fn from(e: AppError) -> Self {
        ApiError(e)
    }
}

impl From<mongodb::error::Error> for ApiError {
    fn from(e: mongodb::error::Error) -> Self {
        ApiError(AppError::from(e))
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let status = self.0.status();
        let msg = self.0.message();
        let body = serde_json::json!({
            "ok": false,
            "error": msg,
            "code": status.as_u16(),
        });
        (status, axum::Json(body)).into_response()
    }
}
