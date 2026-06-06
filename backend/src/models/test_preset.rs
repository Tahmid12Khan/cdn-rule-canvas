//! Test-preset domain model (`sqlx::FromRow`). Never serialized on the API
//! boundary — mapped to [`crate::schemas::test_preset::TestPresetRead`] in the
//! service.

use chrono::{DateTime, Utc};

/// A test-preset row from `rre.test_presets`: a globally reusable saved input
/// for the rule-builder Test panels. `kind` is `rule` (synthetic "Test a rule"
/// inputs) or `url` ("Test with a live URL" inputs); `payload` is an opaque JSON
/// object owned by the frontend test panels.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct TestPreset {
    /// Slug primary key (kebab-case, 3..=64 chars).
    pub slug: String,
    /// Human-readable name (unique).
    pub name: String,
    /// Preset kind (`rule` | `url`).
    pub kind: String,
    /// Opaque saved Test-panel inputs, stored as JSONB (always a JSON object).
    pub payload: serde_json::Value,
    /// Creation timestamp.
    pub created_at: DateTime<Utc>,
    /// Last-update timestamp.
    pub updated_at: DateTime<Utc>,
}
