//! Component DTOs (BACKEND CONTRACT §5).
//!
//! `ComponentConfig` is a typed, internally-tagged (`type`) union; the persisted
//! `components.config` JSONB MUST carry a `type` discriminator equal to the row's
//! `type` column. The service validates by deserializing into `ComponentConfig`
//! before persist; a mismatch surfaces as `VALIDATION_ERROR`.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;
use validator::Validate;

use crate::models::component::Component;
use crate::models::enums::Placement;

/// HTML injection placement mode (component config, not a DB enum).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum HtmlPlacementMode {
    /// Replace the target element's contents.
    Replace,
    /// Append inside the target element.
    Append,
    /// Prepend inside the target element.
    Prepend,
    /// Insert before the target element.
    Before,
    /// Insert after the target element.
    After,
}

/// Typed component configuration. Internally tagged by `type`.
#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ComponentConfig {
    /// `type = "html_injection"`.
    HtmlInjection {
        /// CSS selector the injection targets.
        target_selector: String,
        /// How the HTML body is placed relative to the target.
        placement_mode: HtmlPlacementMode,
        /// Raw HTML to inject (sanitized by the proxy at apply time).
        html_body: String,
        /// Optional theme key.
        #[serde(default)]
        theme: Option<String>,
    },
    /// `type = "content_truncation"`.
    ContentTruncation {
        /// CSS selector whose content is truncated.
        target_selector: String,
        /// Word budget before truncation (`1..=10000`).
        word_count: u32,
        /// Whether to apply a fade-out on the truncated remainder.
        #[serde(default)]
        fade_out: bool,
    },
}

impl ComponentConfig {
    /// The discriminator string this config corresponds to. Matches the `type`
    /// column and the DB CHECK constraint values.
    pub fn type_str(&self) -> &'static str {
        match self {
            ComponentConfig::HtmlInjection { .. } => "html_injection",
            ComponentConfig::ContentTruncation { .. } => "content_truncation",
        }
    }

    /// Domain-specific config validation beyond shape (e.g. word-count bounds).
    /// Returns an error message on failure.
    pub fn validate_domain(&self) -> Result<(), String> {
        match self {
            ComponentConfig::HtmlInjection {
                target_selector, ..
            } => {
                if target_selector.trim().is_empty() {
                    return Err("target_selector must not be empty".to_string());
                }
                Ok(())
            }
            ComponentConfig::ContentTruncation {
                target_selector,
                word_count,
                ..
            } => {
                if target_selector.trim().is_empty() {
                    return Err("target_selector must not be empty".to_string());
                }
                if *word_count < 1 || *word_count > 10_000 {
                    return Err("word_count must be between 1 and 10000".to_string());
                }
                Ok(())
            }
        }
    }
}

/// Create-component request body.
#[derive(Debug, Clone, Deserialize, Validate, ToSchema)]
pub struct ComponentCreate {
    /// Stable slug within the outcome.
    #[validate(length(min = 1, max = 120))]
    pub slug: String,
    /// Discriminator string; must match `config`'s embedded `type`.
    pub r#type: String,
    /// Typed configuration payload.
    pub config: ComponentConfig,
    /// Placement on the page.
    pub placement: Placement,
    /// Optional explicit ordering; defaults to append.
    pub order_index: Option<i32>,
}

/// Patch-component request body. All fields optional.
#[derive(Debug, Clone, Deserialize, Validate, ToSchema)]
pub struct ComponentUpdate {
    /// New slug.
    #[validate(length(min = 1, max = 120))]
    pub slug: Option<String>,
    /// New discriminator string.
    pub r#type: Option<String>,
    /// New typed configuration.
    pub config: Option<ComponentConfig>,
    /// New placement.
    pub placement: Option<Placement>,
    /// New ordering.
    pub order_index: Option<i32>,
}

/// Component read DTO. `config` is the raw stored JSON (`serde_json::Value`).
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct ComponentRead {
    /// Component id.
    pub id: Uuid,
    /// Owning outcome id.
    pub outcome_id: Uuid,
    /// Slug.
    pub slug: String,
    /// Discriminator string.
    pub r#type: String,
    /// Raw stored config JSON.
    pub config: serde_json::Value,
    /// Placement.
    pub placement: Placement,
    /// Ordering.
    pub order_index: i32,
    /// Creation timestamp.
    pub created_at: DateTime<Utc>,
    /// Last-update timestamp.
    pub updated_at: DateTime<Utc>,
}

impl From<Component> for ComponentRead {
    fn from(c: Component) -> Self {
        Self {
            id: c.id,
            outcome_id: c.outcome_id,
            slug: c.slug,
            r#type: c.r#type,
            config: c.config,
            placement: c.placement,
            order_index: c.order_index,
            created_at: c.created_at,
            updated_at: c.updated_at,
        }
    }
}
