//! Active-version payload (BACKEND CONTRACT §5) — consumed by the proxy.
//!
//! The proxy mirrors this exact shape (the `Deserialize` derive lets it reuse
//! the type). `outcomes` are ordered by `order_index ASC`, and each
//! `outcomes[*].components` is ordered by `order_index ASC`.

use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

use crate::{
    models::enums::Placement,
    schemas::{applicability::Applicability, rule_graph::RuleGraph},
};

/// The active (LIVE or STAGING) version for a feature, flattened for the proxy.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ActiveVersionRead {
    /// The active version's number.
    pub version_number: i32,
    /// The typed rule graph.
    pub rule_graph: RuleGraph,
    /// Version-level applicability gate (default `{}`). The proxy gates whether
    /// the feature's outcome components apply to a given response on this.
    pub applicability: Applicability,
    /// Outcomes ordered by `order_index ASC`.
    pub outcomes: Vec<ActiveOutcome>,
}

/// One outcome within the active version.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ActiveOutcome {
    /// Outcome id.
    pub id: Uuid,
    /// Outcome title.
    pub title: String,
    /// Whether this is the protected builtin ShowContent outcome.
    pub is_builtin: bool,
    /// Ordering index.
    pub order_index: i32,
    /// Components ordered by `order_index ASC`.
    pub components: Vec<ActiveComponent>,
}

/// One component within an active outcome.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ActiveComponent {
    /// Component id.
    pub id: Uuid,
    /// Component slug.
    pub slug: String,
    /// Component type discriminator (`html_injection` | `content_truncation` |
    /// `html_remove` | `json_remove` | `json_set` | `json_replace`). The proxy
    /// dispatches on it.
    pub r#type: String,
    /// Raw component config JSON.
    pub config: serde_json::Value,
    /// Placement.
    pub placement: Placement,
    /// Ordering index.
    pub order_index: i32,
}
