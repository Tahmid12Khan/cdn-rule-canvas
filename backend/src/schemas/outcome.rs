//! Outcome DTOs (BACKEND CONTRACT §5). `OutcomeRead` nests its components.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;
use validator::Validate;

use crate::models::outcome::Outcome;
use crate::schemas::component::ComponentRead;

/// Create-outcome request body.
#[derive(Debug, Clone, Deserialize, Validate, ToSchema)]
pub struct OutcomeCreate {
    /// Display title.
    #[validate(length(min = 1, max = 100))]
    pub title: String,
    /// Optional description.
    #[validate(length(max = 500))]
    pub description: Option<String>,
}

/// Patch-outcome request body. All fields optional.
#[derive(Debug, Clone, Deserialize, Validate, ToSchema)]
pub struct OutcomeUpdate {
    /// New title.
    #[validate(length(min = 1, max = 100))]
    pub title: Option<String>,
    /// New description.
    #[validate(length(max = 500))]
    pub description: Option<String>,
    /// New ordering within the version.
    pub order_index: Option<i32>,
}

/// Outcome read DTO with nested components (ordered by `order_index ASC`).
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct OutcomeRead {
    /// Outcome id.
    pub id: Uuid,
    /// Owning version id.
    pub version_id: Uuid,
    /// Title.
    pub title: String,
    /// Optional description.
    pub description: Option<String>,
    /// Whether this is the protected builtin outcome.
    pub is_builtin: bool,
    /// Ordering within the version.
    pub order_index: i32,
    /// Nested components, ordered by `order_index ASC`.
    pub components: Vec<ComponentRead>,
    /// Creation timestamp.
    pub created_at: DateTime<Utc>,
    /// Last-update timestamp.
    pub updated_at: DateTime<Utc>,
}

impl OutcomeRead {
    /// Build a read DTO from an outcome row and its (already ordered) components.
    pub fn from_parts(outcome: Outcome, components: Vec<ComponentRead>) -> Self {
        Self {
            id: outcome.id,
            version_id: outcome.version_id,
            title: outcome.title,
            description: outcome.description,
            is_builtin: outcome.is_builtin,
            order_index: outcome.order_index,
            components,
            created_at: outcome.created_at,
            updated_at: outcome.updated_at,
        }
    }
}

/// One element of a reorder request body (`Vec<ReorderItem>`).
#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct ReorderItem {
    /// Target component id.
    pub id: Uuid,
    /// New ordering value.
    pub order_index: i32,
}
