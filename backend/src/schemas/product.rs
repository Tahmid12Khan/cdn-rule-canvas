//! Product DTOs (Product Catalogue design).
//!
//! `ProductCreate`/`ProductUpdate` are request bodies (validated); `ProductRead`
//! is the response shape. The label regex enforces snake_case
//! (`^[a-z0-9]+(_[a-z0-9]+)*$`) and is immutable after create.

use chrono::{DateTime, Utc};
use once_cell::sync::Lazy;
use regex::Regex;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use validator::Validate;

use crate::models::product::Product;

/// Snake_case label grammar, matching the DB CHECK constraint.
pub static PRODUCT_LABEL_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"^[a-z0-9]+(_[a-z0-9]+)*$").expect("valid product label regex"));

/// Request body for `POST /products`.
#[derive(Debug, Deserialize, Validate, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct ProductCreate {
    /// Snake_case label primary key, immutable after create.
    #[validate(length(min = 1, max = 64), regex(path = *PRODUCT_LABEL_RE))]
    pub label: String,
    /// Human-readable name.
    #[validate(length(min = 1, max = 200))]
    pub name: String,
    /// Optional description.
    #[validate(length(max = 500))]
    pub description: Option<String>,
}

/// Request body for `PATCH /products/{label}`. Every field is optional; the
/// label is immutable (path-derived) and not present here.
#[derive(Debug, Default, Deserialize, Validate, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct ProductUpdate {
    /// New human-readable name.
    #[validate(length(min = 1, max = 200))]
    pub name: Option<String>,
    /// New description.
    #[validate(length(max = 500))]
    pub description: Option<String>,
}

impl ProductUpdate {
    /// Whether the update carries no fields (a no-op PATCH).
    pub fn is_empty(&self) -> bool {
        self.name.is_none() && self.description.is_none()
    }
}

/// Response shape for a product.
#[derive(Debug, Serialize, ToSchema)]
pub struct ProductRead {
    /// Snake_case label primary key.
    pub label: String,
    /// Human-readable name.
    pub name: String,
    /// Optional description.
    pub description: Option<String>,
    /// Creation timestamp.
    pub created_at: DateTime<Utc>,
    /// Last-update timestamp.
    pub updated_at: DateTime<Utc>,
}

impl From<Product> for ProductRead {
    fn from(p: Product) -> Self {
        Self {
            label: p.label,
            name: p.name,
            description: p.description,
            created_at: p.created_at,
            updated_at: p.updated_at,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_input(label: &str) -> ProductCreate {
        ProductCreate {
            label: label.to_string(),
            name: "Premium".to_string(),
            description: None,
        }
    }

    #[test]
    fn create_input_accepts_valid() {
        assert!(create_input("premium").validate().is_ok());
    }

    #[test]
    fn create_input_rejects_uppercase_label() {
        assert!(create_input("Premium").validate().is_err());
    }

    #[test]
    fn create_input_rejects_kebab_case_label() {
        assert!(create_input("premium-tier").validate().is_err());
    }

    #[test]
    fn create_input_rejects_empty_name() {
        let mut input = create_input("premium");
        input.name = String::new();
        assert!(input.validate().is_err());
    }

    #[test]
    fn update_input_accepts_partial() {
        let input = ProductUpdate {
            name: Some("Renamed".to_string()),
            ..ProductUpdate::default()
        };
        assert!(input.validate().is_ok());
        assert!(!input.is_empty());
        assert!(ProductUpdate::default().is_empty());
    }
}
