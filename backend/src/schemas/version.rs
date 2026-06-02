//! Version DTOs (BACKEND CONTRACT §5).
//!
//! Routers map `Version` `FromRow` models to these serde DTOs — the row struct
//! is never serialized directly.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;
use validator::Validate;

use crate::{
    models::enums::VersionStatus,
    schemas::{applicability::Applicability, rule_graph::RuleGraph},
};

/// Request body for `POST /features/{fid}/versions`.
#[derive(Debug, Clone, Default, Deserialize, Validate, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct VersionCreate {
    /// Optional description for the new version.
    #[validate(length(max = 2000))]
    #[serde(default)]
    pub description: Option<String>,
    /// Optional rule graph to store on the new version. When present, its
    /// outcome references (which point at the SOURCE version's outcome ids) are
    /// remapped to the carried-forward outcome ids and validated before persist.
    /// When absent, the source version's rule_graph is cloned (and remapped).
    #[serde(default)]
    pub rule_graph: Option<RuleGraph>,
    /// Optional applicability gate. When present it is stored on the new version;
    /// when absent the source version's applicability is carried forward, else
    /// the default `{}`.
    #[serde(default)]
    pub applicability: Option<Applicability>,
}

/// Request body for `PATCH /features/{fid}/versions/{vnum}`.
#[derive(Debug, Clone, Deserialize, Validate, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct VersionUpdate {
    /// Updated description.
    #[validate(length(max = 2000))]
    #[serde(default)]
    pub description: Option<String>,
    /// Updated rule graph (validated by `rule_graph_service` when present).
    #[serde(default)]
    pub rule_graph: Option<RuleGraph>,
    /// Updated applicability gate. Editable on DRAFT only (same lock as
    /// `rule_graph`); validated (length-capped selectors) when present.
    #[serde(default)]
    pub applicability: Option<Applicability>,
}

/// Full version representation (`GET`/`PATCH`/`POST` responses).
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct VersionRead {
    /// Primary key.
    pub id: Uuid,
    /// Owning feature slug.
    pub feature_id: String,
    /// Monotonic version number within the feature.
    pub version_number: i32,
    /// Optional description.
    pub description: Option<String>,
    /// Lifecycle status.
    pub status: VersionStatus,
    /// Typed rule graph.
    pub rule_graph: RuleGraph,
    /// Version-level applicability gate (default `{}`).
    pub applicability: Applicability,
    /// Author.
    pub created_by: String,
    /// Last updater.
    pub last_updated_by: String,
    /// Last update timestamp.
    pub last_updated_at: DateTime<Utc>,
    /// Creation timestamp.
    pub created_at: DateTime<Utc>,
}

/// Lightweight version row for list responses (no `rule_graph` payload).
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct VersionSummary {
    /// Primary key.
    pub id: Uuid,
    /// Owning feature slug.
    pub feature_id: String,
    /// Monotonic version number within the feature.
    pub version_number: i32,
    /// Optional description.
    pub description: Option<String>,
    /// Lifecycle status.
    pub status: VersionStatus,
    /// Last updater.
    pub last_updated_by: String,
    /// Last update timestamp.
    pub last_updated_at: DateTime<Utc>,
    /// Creation timestamp.
    pub created_at: DateTime<Utc>,
}

/// Target environment for publish/unpublish (request DTO, NOT a DB enum).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "lowercase")]
pub enum PublishEnvironment {
    /// The staging environment.
    Staging,
    /// The live environment.
    Live,
}

/// Request body for publish/unpublish.
#[derive(Debug, Clone, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct PublishRequest {
    /// Target environment.
    pub environment: PublishEnvironment,
}

/// Query parameters for `GET /features/{fid}/versions`.
#[derive(Debug, Clone, Deserialize, IntoParams)]
pub struct VersionListQuery {
    /// Filter by lifecycle status.
    #[serde(default)]
    pub status: Option<VersionStatus>,
    /// Free-text search over description.
    #[serde(default)]
    pub search: Option<String>,
    /// 1-based page number. Inlined (not a flattened `PageParams`) because
    /// `serde_urlencoded` — axum's `Query` backend — cannot deserialize
    /// `#[serde(flatten)]` structs from a query string.
    #[serde(default)]
    pub page: Option<u32>,
    /// Page size (capped at `MAX_PAGE_SIZE`).
    #[serde(default)]
    pub page_size: Option<u32>,
}
