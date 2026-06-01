//! Version row model (BACKEND CONTRACT §4).
//!
//! `FromRow` struct for `rre.versions`. Never serialized on the API boundary —
//! the service layer maps it to a `VersionRead`/`VersionSummary` DTO.

use chrono::{DateTime, Utc};
use uuid::Uuid;

use crate::models::enums::VersionStatus;

/// A row from `rre.versions`.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct Version {
    /// Primary key.
    pub id: Uuid,
    /// Owning feature slug.
    pub feature_id: String,
    /// Monotonic version number within the feature (set by the service).
    pub version_number: i32,
    /// Optional human description.
    pub description: Option<String>,
    /// Lifecycle status.
    pub status: VersionStatus,
    /// The rule graph, stored as JSONB. Typed into a `RuleGraph` on read in the
    /// service layer.
    pub rule_graph: serde_json::Value,
    /// Version-level applicability gate, stored as JSONB. Typed into an
    /// `Applicability` on read in the service layer (default `{}`).
    pub applicability: serde_json::Value,
    /// Author of the version.
    pub created_by: String,
    /// Last user to update the version.
    pub last_updated_by: String,
    /// Timestamp of the last update.
    pub last_updated_at: DateTime<Utc>,
    /// Creation timestamp.
    pub created_at: DateTime<Utc>,
}
