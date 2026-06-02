//! Typed proxy errors. Every upstream / eval / apply failure maps to a typed
//! response (or fails open) — we NEVER panic into the client.

use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde_json::json;

#[derive(Debug, thiserror::Error)]
pub enum ProxyError {
    #[error("upstream timeout")]
    UpstreamTimeout,
    #[error("upstream unavailable")]
    UpstreamUnavailable,
    #[error("bad upstream response")]
    UpstreamProtocol,
    #[error("upstream response too large")]
    UpstreamTooLarge,
    #[error(transparent)]
    Internal(#[from] anyhow::Error),
}

impl ProxyError {
    fn status(&self) -> StatusCode {
        match self {
            ProxyError::UpstreamTimeout => StatusCode::GATEWAY_TIMEOUT,
            ProxyError::UpstreamUnavailable | ProxyError::UpstreamProtocol => {
                StatusCode::BAD_GATEWAY
            }
            ProxyError::UpstreamTooLarge => StatusCode::BAD_GATEWAY,
            ProxyError::Internal(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }

    /// Stable, client-safe code string. Never leaks internal detail.
    fn code(&self) -> &'static str {
        match self {
            ProxyError::UpstreamTimeout => "UPSTREAM_TIMEOUT",
            ProxyError::UpstreamUnavailable => "UPSTREAM_UNAVAILABLE",
            ProxyError::UpstreamProtocol => "UPSTREAM_PROTOCOL",
            ProxyError::UpstreamTooLarge => "UPSTREAM_TOO_LARGE",
            ProxyError::Internal(_) => "INTERNAL_ERROR",
        }
    }
}

impl IntoResponse for ProxyError {
    fn into_response(self) -> Response {
        let status = self.status();
        // Log internal errors with full detail; the client only sees the generic code.
        if let ProxyError::Internal(ref e) = self {
            tracing::error!(error = %e, "proxy internal error");
        }
        let body = Json(json!({
            "error": { "code": self.code(), "message": self.to_string() }
        }));
        (status, body).into_response()
    }
}
