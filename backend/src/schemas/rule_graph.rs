//! Rule-graph DTOs (BACKEND CONTRACT §6).
//!
//! `RuleGraph` is the top-level shape of `versions.rule_graph` JSONB: one
//! [`CanvasGraph`] per user class (anonymous / registered / customer). Each
//! canvas is a directed graph of [`Node`]s connected by [`Edge`]s. Decision
//! nodes carry a [`ProcessorConfig`]; outcome nodes reference an outcome row.
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

/// A decision processor. Internally tagged by `type`.
#[derive(Deserialize, Serialize, Clone, Debug, PartialEq, ToSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ProcessorConfig {
    /// Match against an HTML `<meta>` tag's `content` attribute.
    MetaTags {
        /// The `name` of the meta tag to read.
        tag_name: String,
        /// How to compare the tag content.
        operator: MetaTagsOperator,
        /// Comparison operand (unused for `exists`).
        #[serde(default)]
        value: Option<String>,
    },
    /// Match against the request device class derived from the user agent.
    DeviceType {
        /// How to compare the device.
        operator: DeviceOperator,
        /// Target device class.
        value: DeviceValue,
    },
    /// Match against the request article URL (path).
    ArticleUrl {
        /// How to compare the URL.
        operator: ArticleUrlOperator,
        /// Comparison operand (regex pattern when `operator` is `matches`).
        value: String,
    },
}

/// Comparison operator for the meta-tags processor.
#[derive(Deserialize, Serialize, Clone, Copy, Debug, PartialEq, Eq, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum MetaTagsOperator {
    /// Substring match.
    Contains,
    /// Exact match.
    Equals,
    /// Presence check (value ignored).
    Exists,
}

/// Comparison operator for the device-type processor.
#[derive(Deserialize, Serialize, Clone, Copy, Debug, PartialEq, Eq, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum DeviceOperator {
    /// Exact match.
    Equals,
    /// Substring match.
    Contains,
}

/// Comparison operator for the article-url processor.
#[derive(Deserialize, Serialize, Clone, Copy, Debug, PartialEq, Eq, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum ArticleUrlOperator {
    /// Substring match.
    Contains,
    /// Regular-expression match.
    Matches,
    /// Prefix match.
    StartsWith,
    /// Exact match.
    Equals,
}

/// Device class operand.
#[derive(Deserialize, Serialize, Clone, Copy, Debug, PartialEq, Eq, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum DeviceValue {
    /// Mobile phone.
    Mobile,
    /// Desktop / laptop.
    Desktop,
    /// Tablet.
    Tablet,
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
