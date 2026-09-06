//! Saved-outcome domain model (`sqlx::FromRow`). Never serialized on the API
//! boundary directly — mapped to [`crate::schemas::saved_outcome::SavedOutcomeRead`].

use chrono::{DateTime, Utc};
use uuid::Uuid;

/// A `rre.saved_outcomes` row: a named, reusable reference to one Library
/// component version plus fixed variable values, usable from any rule via the
/// `apply_saved_outcome`/`apply_saved_outcome_json` action nodes.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct SavedOutcome {
    pub id: Uuid,
    pub slug: String,
    pub name: String,
    pub component_id: Uuid,
    /// `None` = "Latest" (tracks the component's own default pointer).
    pub version_number: Option<i32>,
    pub variables: serde_json::Value,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}
