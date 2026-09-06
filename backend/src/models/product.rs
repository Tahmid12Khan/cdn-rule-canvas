//! Product domain model (`sqlx::FromRow`). Never serialized on the API
//! boundary — mapped to [`crate::schemas::product::ProductRead`] in the service.

use chrono::{DateTime, Utc};

/// A product row from `rre.products`: a flat entitlement label a visitor may
/// hold, tested by the `has_product` decision node.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct Product {
    /// Snake_case label primary key, immutable after create.
    pub label: String,
    /// Human-readable name (unique).
    pub name: String,
    /// Optional description.
    pub description: Option<String>,
    /// Creation timestamp.
    pub created_at: DateTime<Utc>,
    /// Last-update timestamp.
    pub updated_at: DateTime<Utc>,
}
