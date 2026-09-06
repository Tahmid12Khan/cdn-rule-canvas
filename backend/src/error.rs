//! Uniform error envelope and HTTP mapping (BACKEND CONTRACT §1).
//!
//! Every non-2xx response carries the shape:
//! ```json
//! { "error": { "code": "...", "message": "...", "details": [ ... ] } }
//! ```
//! `details` is present ONLY for 422 validation failures.

use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde::Serialize;
use utoipa::ToSchema;

/// One validation problem (the `error.details[*]` element).
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct ValidationDetail {
    /// Location of the offending value, e.g. `rule_graph.canvas.edges[1]`.
    pub loc: String,
    /// Human-readable message.
    pub msg: String,
    /// Stable rule identifier, e.g. `edge_endpoint_exists`.
    pub rule_id: String,
}

impl ValidationDetail {
    /// Convenience constructor.
    pub fn new(loc: impl Into<String>, msg: impl Into<String>, rule_id: impl Into<String>) -> Self {
        Self {
            loc: loc.into(),
            msg: msg.into(),
            rule_id: rule_id.into(),
        }
    }
}

/// Domain + infrastructure error type. Converts to the uniform envelope via
/// [`IntoResponse`].
#[derive(Debug, thiserror::Error)]
pub enum AppError {
    /// Request DTO / `validator` failure, or rule_graph validation failure. → 422
    #[error("validation error")]
    Validation { details: Vec<ValidationDetail> },
    /// Feature slug missing. → 404
    #[error("{0}")]
    FeatureNotFound(String),
    /// Site slug missing. → 404
    #[error("{0}")]
    SiteNotFound(String),
    /// Product label missing. → 404
    #[error("{0}")]
    ProductNotFound(String),
    /// Test-preset slug missing. → 404
    #[error("{0}")]
    TestPresetNotFound(String),
    /// Version (by number or id) missing. → 404
    #[error("{0}")]
    VersionNotFound(String),
    /// Outcome id missing. → 404
    #[error("{0}")]
    OutcomeNotFound(String),
    /// Component id missing. → 404
    #[error("{0}")]
    ComponentNotFound(String),
    /// Active-version requested, none LIVE/STAGING. → 404
    #[error("{0}")]
    NoLiveVersion(String),
    /// Duplicate feature slug. → 409
    #[error("{0}")]
    SlugConflict(String),
    /// Duplicate feature `execution_order` within a type. → 409
    #[error("{0}")]
    ExecutionOrderConflict(String),
    /// Mutate rule_graph/outcomes/components on a non-DRAFT version. → 409
    #[error("{0}")]
    VersionEditLocked(String),
    /// Illegal publish/unpublish/delete transition. → 409
    #[error("{0}")]
    InvalidStatusTransition(String),
    /// Delete builtin ShowContent outcome. → 409
    #[error("{0}")]
    BuiltinOutcomeProtected(String),
    /// Unexpected internal error (never leaks DB text to the client). → 500
    #[error(transparent)]
    Internal(#[from] anyhow::Error),
}

impl AppError {
    /// SCREAMING_SNAKE_CASE stable code for the envelope.
    pub fn code(&self) -> &'static str {
        match self {
            AppError::Validation { .. } => "VALIDATION_ERROR",
            AppError::FeatureNotFound(_) => "FEATURE_NOT_FOUND",
            AppError::SiteNotFound(_) => "SITE_NOT_FOUND",
            AppError::ProductNotFound(_) => "PRODUCT_NOT_FOUND",
            AppError::TestPresetNotFound(_) => "TEST_PRESET_NOT_FOUND",
            AppError::VersionNotFound(_) => "VERSION_NOT_FOUND",
            AppError::OutcomeNotFound(_) => "OUTCOME_NOT_FOUND",
            AppError::ComponentNotFound(_) => "COMPONENT_NOT_FOUND",
            AppError::NoLiveVersion(_) => "NO_LIVE_VERSION",
            AppError::SlugConflict(_) => "SLUG_CONFLICT",
            AppError::ExecutionOrderConflict(_) => "EXECUTION_ORDER_CONFLICT",
            AppError::VersionEditLocked(_) => "VERSION_EDIT_LOCKED",
            AppError::InvalidStatusTransition(_) => "INVALID_STATUS_TRANSITION",
            AppError::BuiltinOutcomeProtected(_) => "BUILTIN_OUTCOME_PROTECTED",
            AppError::Internal(_) => "INTERNAL_ERROR",
        }
    }

    /// HTTP status for this error.
    pub fn status(&self) -> StatusCode {
        match self {
            AppError::Validation { .. } => StatusCode::UNPROCESSABLE_ENTITY,
            AppError::FeatureNotFound(_)
            | AppError::SiteNotFound(_)
            | AppError::ProductNotFound(_)
            | AppError::TestPresetNotFound(_)
            | AppError::VersionNotFound(_)
            | AppError::OutcomeNotFound(_)
            | AppError::ComponentNotFound(_)
            | AppError::NoLiveVersion(_) => StatusCode::NOT_FOUND,
            AppError::SlugConflict(_)
            | AppError::ExecutionOrderConflict(_)
            | AppError::VersionEditLocked(_)
            | AppError::InvalidStatusTransition(_)
            | AppError::BuiltinOutcomeProtected(_) => StatusCode::CONFLICT,
            AppError::Internal(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }

    /// Construct a validation error from a list of details.
    pub fn validation(details: Vec<ValidationDetail>) -> Self {
        AppError::Validation { details }
    }
}

/// Convenience alias for handler/service results.
pub type AppResult<T> = Result<T, AppError>;

/// The serialized `error` object body.
#[derive(Debug, Serialize, ToSchema)]
pub struct ErrorBody {
    /// Stable error code.
    pub code: String,
    /// Human-readable message safe to display.
    pub message: String,
    /// Validation problems; present only for 422.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<Vec<ValidationDetail>>,
}

/// Top-level envelope: `{ "error": { ... } }`.
#[derive(Debug, Serialize, ToSchema)]
pub struct ErrorEnvelope {
    /// The error object.
    pub error: ErrorBody,
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let status = self.status();
        let code = self.code().to_string();

        let (message, details) = match &self {
            AppError::Validation { details } => {
                ("validation error".to_string(), Some(details.clone()))
            }
            AppError::Internal(err) => {
                // Log full detail server-side; return a generic message to the client.
                tracing::error!(error = ?err, "internal error");
                ("internal server error".to_string(), None)
            }
            other => (other.to_string(), None),
        };

        let envelope = ErrorEnvelope {
            error: ErrorBody {
                code,
                message,
                details,
            },
        };

        (status, Json(envelope)).into_response()
    }
}

/// Map raw `sqlx::Error` to an internal error. Slug-conflict (pg `23505`) is
/// special-cased in `feature_service` by inspecting the pg error code, NOT here.
impl From<sqlx::Error> for AppError {
    fn from(err: sqlx::Error) -> Self {
        AppError::Internal(anyhow::Error::new(err))
    }
}

/// Convert a `validator::ValidationErrors` into the uniform 422 envelope. Each
/// field error becomes one [`ValidationDetail`] keyed by `(loc, msg, rule_id)`;
/// the fallback message is the error `code` when no custom message is set. This
/// is the single mapping used by every router/service validation entrypoint.
impl From<validator::ValidationErrors> for AppError {
    fn from(errors: validator::ValidationErrors) -> Self {
        let details = errors
            .field_errors()
            .into_iter()
            .flat_map(|(field, errs)| {
                errs.iter().map(move |e| {
                    let msg = e
                        .message
                        .as_ref()
                        .map(|m| m.to_string())
                        .unwrap_or_else(|| e.code.to_string());
                    ValidationDetail::new(field.to_string(), msg, e.code.to_string())
                })
            })
            .collect();
        AppError::validation(details)
    }
}
