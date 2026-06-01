//! Component domain model (BACKEND CONTRACT §4). `FromRow` only — never
//! serialized on the API boundary (mapped to `ComponentRead` in the service).

use chrono::{DateTime, Utc};
use uuid::Uuid;

use crate::models::enums::Placement;

/// A row of `rre.components`.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct Component {
    /// Component id.
    pub id: Uuid,
    /// Owning outcome id.
    pub outcome_id: Uuid,
    /// Stable slug within the outcome.
    pub slug: String,
    /// Discriminator string: `html_injection` | `content_truncation`.
    pub r#type: String,
    /// Typed config persisted as JSONB; the embedded `type` matches `r#type`.
    pub config: serde_json::Value,
    /// Placement on the page.
    pub placement: Placement,
    /// Ordering within the outcome (ascending).
    pub order_index: i32,
    /// Creation timestamp.
    pub created_at: DateTime<Utc>,
    /// Last-update timestamp.
    pub updated_at: DateTime<Utc>,
}
