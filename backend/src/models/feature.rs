//! Feature domain model (`sqlx::FromRow`). Never serialized on the API
//! boundary — mapped to [`crate::schemas::feature::FeatureRead`] in the service.

use chrono::{DateTime, Utc};
use uuid::Uuid;

use crate::models::enums::FeatureType;

/// A feature row from `rre.features`.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct Feature {
    /// Slug primary key (kebab-case, 3..=64 chars).
    pub id: String,
    /// Human-readable name.
    pub name: String,
    /// Content type (`html` | `json`).
    pub r#type: FeatureType,
    /// Currently-staged version id, if any.
    pub staging_version_id: Option<Uuid>,
    /// Currently-live version id, if any.
    pub live_version_id: Option<Uuid>,
    /// Creation timestamp.
    pub created_at: DateTime<Utc>,
    /// Last-update timestamp.
    pub updated_at: DateTime<Utc>,
}
