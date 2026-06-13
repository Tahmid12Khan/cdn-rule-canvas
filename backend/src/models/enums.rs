//! Shared Postgres ⇄ Rust enums (BACKEND CONTRACT §2).
//!
//! DB string forms are lowercase/snake. JSON over the API uses the same DB
//! string form (`live`, `sticky_footer`). These types map onto the Postgres
//! enum types declared in `migrations/0001_baseline.up.sql`.

use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

/// Feature content type. DB type `rre.feature_type`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, sqlx::Type, Serialize, Deserialize, ToSchema)]
#[sqlx(type_name = "feature_type", rename_all = "lowercase")]
#[serde(rename_all = "lowercase")]
pub enum FeatureType {
    /// HTML feature.
    Html,
    /// JSON feature.
    Json,
}

/// Version lifecycle status. DB type `rre.version_status`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, sqlx::Type, Serialize, Deserialize, ToSchema)]
#[sqlx(type_name = "version_status", rename_all = "snake_case")]
#[serde(rename_all = "snake_case")]
pub enum VersionStatus {
    /// Editable draft.
    Draft,
    /// Published to the staging environment.
    Staging,
    /// Published to the live environment.
    Live,
    /// Previously published, now superseded.
    Prev,
}

/// How a Component template's `default` version resolves. Stored as a plain
/// `VARCHAR(8)` (NOT a Postgres enum type) — the DB string form is lowercase.
/// `Latest` tracks the highest `version_number`; `Pinned` resolves to the
/// component's `default_version_id`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, sqlx::Type, Serialize, Deserialize, ToSchema)]
#[sqlx(type_name = "VARCHAR", rename_all = "lowercase")]
#[serde(rename_all = "lowercase")]
pub enum DefaultMode {
    /// Default tracks the highest `version_number` (auto-advances).
    Latest,
    /// Default is pinned to `default_version_id`.
    Pinned,
}

/// Component placement. DB type `rre.placement`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, sqlx::Type, Serialize, Deserialize, ToSchema)]
#[sqlx(type_name = "placement", rename_all = "snake_case")]
#[serde(rename_all = "snake_case")]
pub enum Placement {
    /// Inline within the matched element.
    Inline,
    /// Sticky footer overlay.
    StickyFooter,
    /// Popup/modal overlay.
    Popup,
}
