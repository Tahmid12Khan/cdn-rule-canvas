//! Types shared by the RRE backend's published-rule payloads, the proxy's
//! runtime caches and the edge bundle. Moved out of the proxy's
//! `infra::backend_client` so every host deserializes the SAME structs — a
//! backend/proxy/edge schema skew is the failure mode this prevents.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::graph::{CanvasGraph, RuleGraph};

/// Placement mirror (BACKEND CONTRACT §2). snake_case wire form.
#[derive(Deserialize, Serialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Placement {
    Inline,
    StickyFooter,
    Popup,
}

/// Version-level applicability gate (BACKEND CONTRACT §5). Mirrors the backend
/// `Applicability` DTO. serde defaults, no `deny_unknown_fields` — an older
/// backend that omits the field deserializes to the empty (apply-always) gate.
#[derive(Deserialize, Serialize, Clone, Debug, Default)]
pub struct Applicability {
    /// CSS selector that must match >=1 element for an HTML response's rules to
    /// apply. None/empty = apply whenever the response content-type is HTML.
    #[serde(default)]
    pub html_selector: Option<String>,
    /// JSONPath that must match >=1 node for a JSON response's rules to apply.
    /// None/empty = apply whenever the response content-type is JSON.
    #[serde(default)]
    pub json_selector: Option<String>,
}

/// Active-version payload — EXACT shape from BACKEND CONTRACT §5.
#[derive(Deserialize, Serialize, Clone, Debug)]
pub struct ActiveVersionRead {
    pub version_number: i32,
    pub rule_graph: RuleGraph,
    #[serde(default)]
    pub applicability: Applicability,
    pub outcomes: Vec<ActiveOutcome>,
}

#[derive(Deserialize, Serialize, Clone, Debug)]
pub struct ActiveOutcome {
    pub id: Uuid,
    pub title: String,
    pub is_builtin: bool,
    pub order_index: i32,
    pub components: Vec<ActiveComponent>,
}

#[derive(Deserialize, Serialize, Clone, Debug)]
pub struct ActiveComponent {
    pub id: Uuid,
    pub slug: String,
    pub r#type: String,
    pub config: serde_json::Value,
    pub placement: Placement,
    pub order_index: i32,
}

impl ActiveVersionRead {
    /// The single Rule Canvas.
    pub fn canvas(&self) -> &CanvasGraph {
        &self.rule_graph.canvas
    }

    /// Find an outcome by id (the eval result `outcomeId`).
    pub fn find_outcome(&self, id: Uuid) -> Option<&ActiveOutcome> {
        self.outcomes.iter().find(|o| o.id == id)
    }
}

impl ActiveOutcome {
    /// True if this is the builtin "ShowContent" outcome (a no-op terminal).
    pub fn is_builtin_show_content(&self) -> bool {
        self.is_builtin
    }
}

/// Which version of a Component template a rule node references (design §2/§4.1).
/// Parsed from a rule action's `version` field: the string `"default"` → `Default`
/// (resolved per the component's movable default pointer), a positive integer → the
/// pinned `Version(n)` (with proxy-side fall-back to the current default if `n` is
/// gone). Used as the second half of the component-cache key so a `"default"` and a
/// pinned reference cache independently.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum VersionSelector {
    Default,
    Version(i32),
}

impl VersionSelector {
    /// Parse the rule action's `version` value: `"default"` (or absent/Null) →
    /// `Default`; a JSON integer or numeric string → `Version(n)`. Anything else
    /// (non-positive, non-numeric, structured) falls back to `Default` — the
    /// proxy never trusts the value blindly and `Default` is always resolvable.
    pub fn from_action_value(v: Option<&serde_json::Value>) -> Self {
        match v {
            None | Some(serde_json::Value::Null) => VersionSelector::Default,
            Some(serde_json::Value::Number(n)) => match n.as_i64() {
                Some(i) if i > 0 && i <= i64::from(i32::MAX) => VersionSelector::Version(i as i32),
                _ => VersionSelector::Default,
            },
            Some(serde_json::Value::String(s)) => {
                let s = s.trim();
                if s.eq_ignore_ascii_case("default") {
                    VersionSelector::Default
                } else {
                    match s.parse::<i32>() {
                        Ok(i) if i > 0 => VersionSelector::Version(i),
                        _ => VersionSelector::Default,
                    }
                }
            }
            _ => VersionSelector::Default,
        }
    }

    /// The `?version=` query value the resolve endpoint expects.
    pub fn as_query(&self) -> String {
        match self {
            VersionSelector::Default => "default".to_string(),
            VersionSelector::Version(n) => n.to_string(),
        }
    }
}

/// Proxy-facing resolved Component payload — EXACT shape from the backend
/// `GET /api/v1/component-templates/{cid}/resolve` endpoint (design §3.2
/// `ResolvedComponentRead`). `variables` is carried as a raw JSON value (the
/// proxy renders against the rule action's `variables`, not these declared ones,
/// so it never inspects the shape here). No `deny_unknown_fields` so extra
/// backend fields are ignored.
#[derive(Deserialize, Serialize, Clone, Debug)]
pub struct ResolvedComponent {
    pub version_number: i32,
    pub html_body: String,
    #[serde(default)]
    pub variables: serde_json::Value,
}

/// A rendered Outcomes-Library outcome — EXACT shape from the backend
/// `resolve` route, which already picked the version and rendered the HTML.
#[derive(Deserialize, Serialize, Clone, Debug)]
pub struct ResolvedSavedOutcome {
    pub html_body: String,
}
