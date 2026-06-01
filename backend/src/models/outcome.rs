//! Outcome domain model (BACKEND CONTRACT §4). `FromRow` only — never
//! serialized on the API boundary (mapped to `OutcomeRead` in the service).

use chrono::{DateTime, Utc};
use uuid::Uuid;

/// A row of `rre.outcomes`.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct Outcome {
    /// Outcome id.
    pub id: Uuid,
    /// Owning version id.
    pub version_id: Uuid,
    /// Display title.
    pub title: String,
    /// Optional description.
    pub description: Option<String>,
    /// Whether this is the protected builtin `ShowContent` outcome.
    pub is_builtin: bool,
    /// Ordering within the version (ascending).
    pub order_index: i32,
    /// Creation timestamp.
    pub created_at: DateTime<Utc>,
    /// Last-update timestamp.
    pub updated_at: DateTime<Utc>,
}
