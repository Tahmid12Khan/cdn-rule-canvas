//! Health-check response DTO.

use serde::Serialize;
use utoipa::ToSchema;

/// Response body for `/health` and `/healthz/db`.
#[derive(Debug, Serialize, ToSchema)]
pub struct HealthResponse {
    /// Liveness status, e.g. `ok`.
    pub status: String,
    /// Application version (from settings).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    /// Git commit (from settings).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub git_commit: Option<String>,
}

impl HealthResponse {
    /// Build an `ok` health response carrying version/commit metadata.
    pub fn ok(version: impl Into<String>, git_commit: impl Into<String>) -> Self {
        Self {
            status: "ok".to_string(),
            version: Some(version.into()),
            git_commit: Some(git_commit.into()),
        }
    }
}
