//! Feature DTOs (BACKEND CONTRACT §5).
//!
//! `FeatureCreate`/`FeatureUpdate` are request bodies (validated); `FeatureRead`
//! is the response shape. The slug regex `SLUG_RE` enforces lowercase kebab-case.

use chrono::{DateTime, Utc};
use once_cell::sync::Lazy;
use regex::Regex;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;
use validator::Validate;

use crate::models::{enums::FeatureType, feature::Feature};

/// Slug pattern: lowercase kebab-case (`^[a-z0-9]+(?:-[a-z0-9]+)*$`).
pub static SLUG_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"^[a-z0-9]+(?:-[a-z0-9]+)*$").expect("valid slug regex"));

/// Request body for `POST /features`.
#[derive(Debug, Deserialize, Validate, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct FeatureCreate {
    /// Slug primary key (kebab-case, lowercase, 3..=64 chars).
    #[validate(length(min = 3, max = 64), regex(path = *SLUG_RE))]
    pub id: String,
    /// Human-readable name.
    #[validate(length(min = 1, max = 200))]
    pub name: String,
    /// Content type.
    pub r#type: FeatureType,
}

/// Request body for `PATCH /features/{fid}`.
#[derive(Debug, Deserialize, Validate, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct FeatureUpdate {
    /// New human-readable name.
    #[validate(length(min = 1, max = 200))]
    pub name: Option<String>,
}

/// Response shape for a feature.
#[derive(Debug, Serialize, ToSchema)]
pub struct FeatureRead {
    /// Slug primary key.
    pub id: String,
    /// Human-readable name.
    pub name: String,
    /// Content type.
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

impl From<Feature> for FeatureRead {
    fn from(f: Feature) -> Self {
        Self {
            id: f.id,
            name: f.name,
            r#type: f.r#type,
            staging_version_id: f.staging_version_id,
            live_version_id: f.live_version_id,
            created_at: f.created_at,
            updated_at: f.updated_at,
        }
    }
}
