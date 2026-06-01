//! Rule-graph DTOs (BACKEND CONTRACT §6).
//!
//! `RuleGraph` is the top-level shape of `versions.rule_graph` JSONB: one
//! [`CanvasGraph`] per user class (anonymous / registered / customer). Each
//! canvas is a directed graph of [`Node`]s connected by [`Edge`]s. Decision
//! nodes carry a generic [`ProcessorConfig`] (a snake_case `type` discriminator
//! plus an open field map, validated against the node-type manifest); outcome
//! nodes reference an outcome row.
//!
//! This shape is STABLE and mirrored verbatim by the proxy
//! (`proxy/src/domain/graph.rs`). Do not change it without bumping the contract.

use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

/// The `version.rule_graph` JSONB top-level object — one [`CanvasGraph`] per
/// user class.
#[derive(Deserialize, Serialize, Clone, Debug, Default, PartialEq, ToSchema)]
pub struct RuleGraph {
    /// Canvas evaluated for anonymous (un-identified) users.
    pub anonymous: CanvasGraph,
    /// Canvas evaluated for registered (logged-in, non-paying) users.
    pub registered: CanvasGraph,
    /// Canvas evaluated for paying customers.
    pub customer: CanvasGraph,
}

impl RuleGraph {
    /// Borrow the three canvases alongside their stable names, for per-canvas
    /// iteration (used by validation `loc` paths).
    pub fn canvases(&self) -> [(&'static str, &CanvasGraph); 3] {
        [
            ("anonymous", &self.anonymous),
            ("registered", &self.registered),
            ("customer", &self.customer),
        ]
    }
}

/// One decision canvas: nodes, edges, and an optional designated root node.
#[derive(Deserialize, Serialize, Clone, Debug, Default, PartialEq, ToSchema)]
pub struct CanvasGraph {
    /// Graph nodes (decision + outcome).
    pub nodes: Vec<Node>,
    /// Directed, branch-labelled edges between nodes.
    pub edges: Vec<Edge>,
    /// Optional entry node id; must exist in `nodes` when set.
    #[serde(default)]
    pub root_node_id: Option<String>,
}

/// A canvas node. Internally tagged by `kind`.
#[derive(Deserialize, Serialize, Clone, Debug, PartialEq, ToSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Node {
    /// A decision node carrying a processor; has outgoing yes/no branches.
    Decision {
        /// React Flow node id (string).
        id: String,
        /// The processor configuration evaluated at this node.
        processor: ProcessorConfig,
        /// Canvas coordinates.
        position: Position,
    },
    /// A terminal node referencing an outcome row; has no outgoing edges.
    Outcome {
        /// React Flow node id (string).
        id: String,
        /// The referenced `rre.outcomes` row id for this version.
        outcome_id: Uuid,
        /// Canvas coordinates.
        position: Position,
    },
}

impl Node {
    /// The node's string id, regardless of variant.
    pub fn id(&self) -> &str {
        match self {
            Node::Decision { id, .. } => id,
            Node::Outcome { id, .. } => id,
        }
    }

    /// `true` for an outcome (terminal) node.
    pub fn is_outcome(&self) -> bool {
        matches!(self, Node::Outcome { .. })
    }
}

/// A decision processor: a generic, manifest-validated shape.
///
/// Round-trips the SAME wire JSON the frontend and proxy exchange:
/// `{ "type": "<kind>", "<field>": <value>, ... }`. `type` is the canonical
/// snake_case kind (e.g. `"article_url"`); the remaining fields are an open
/// `snake_case` map validated against the node-type manifest at the service
/// boundary (NOT by serde variants). Unknown extra fields are preserved on
/// round-trip and ignored by validation (forward-compatible).
#[derive(Deserialize, Serialize, Clone, Debug, PartialEq, ToSchema)]
pub struct ProcessorConfig {
    /// The canonical snake_case kind, e.g. `"article_url"`.
    pub r#type: String,
    /// The remaining processor fields as a flat snake_case map.
    #[serde(flatten)]
    #[schema(value_type = Object)]
    pub fields: serde_json::Map<String, serde_json::Value>,
}

/// A directed edge between two nodes, labelled with the branch it represents.
#[derive(Deserialize, Serialize, Clone, Debug, PartialEq, ToSchema)]
pub struct Edge {
    /// React Flow edge id (string).
    pub id: String,
    /// Source node id.
    pub source_node_id: String,
    /// Target node id.
    pub target_node_id: String,
    /// The branch (yes/no) this edge represents.
    pub branch: Branch,
}

/// The branch label on an [`Edge`].
#[derive(Deserialize, Serialize, Clone, Copy, Debug, PartialEq, Eq, Hash, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum Branch {
    /// The "true" branch out of a decision node.
    Yes,
    /// The "false" branch out of a decision node.
    No,
}

/// Canvas coordinates for a node.
#[derive(Deserialize, Serialize, Clone, Copy, Debug, PartialEq, ToSchema)]
pub struct Position {
    /// Horizontal coordinate.
    pub x: f64,
    /// Vertical coordinate.
    pub y: f64,
}
