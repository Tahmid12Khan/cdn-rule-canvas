//! Component-template domain models (`sqlx::FromRow`). Never serialized on the
//! API boundary — mapped to the `*Read`/`*Summary` DTOs in
//! [`crate::schemas::component_template`] in the service layer.
//!
//! A Component (UI label) is a globally-scoped, INDEPENDENTLY-versioned HTML
//! (mustache) template that holds ONLY a result payload. Rules supply the
//! control flow + variable values; the proxy renders at request time.

use chrono::{DateTime, Utc};
use uuid::Uuid;

use crate::models::enums::DefaultMode;

/// A row from `rre.component_templates`: the library entry + the movable default
/// pointer. `default_version_id` is meaningful only when `default_mode = pinned`
/// (when `latest`, the default resolves to the highest `version_number`).
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct ComponentTemplate {
    /// Component UUID (primary key).
    pub id: Uuid,
    /// Slug (unique, kebab-case, 3..=120 chars).
    pub slug: String,
    /// Human-readable name.
    pub name: String,
    /// Optional description.
    pub description: Option<String>,
    /// How the `default` version resolves (`latest` | `pinned`).
    pub default_mode: DefaultMode,
    /// The pinned default version id (used only when `default_mode = pinned`).
    pub default_version_id: Option<Uuid>,
    /// Creation timestamp.
    pub created_at: DateTime<Utc>,
    /// Last-update timestamp.
    pub updated_at: DateTime<Utc>,
}

/// A row from `rre.component_template_versions`: an independently-numbered
/// version holding the mustache `html_body` (the only payload) plus the declared
/// `variables` metadata (a JSON array of `{name, title, description}`).
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct ComponentTemplateVersion {
    /// Version UUID (primary key).
    pub id: Uuid,
    /// Owning component id.
    pub component_id: Uuid,
    /// Per-component version number (unique within the component).
    pub version_number: i32,
    /// Optional per-version description.
    pub description: Option<String>,
    /// Mustache template body (the only payload).
    pub html_body: String,
    /// Declared variables (`[{name, title, description}]`), stored as JSONB.
    pub variables: serde_json::Value,
    /// Creation timestamp.
    pub created_at: DateTime<Utc>,
    /// Last-update timestamp.
    pub updated_at: DateTime<Utc>,
}
