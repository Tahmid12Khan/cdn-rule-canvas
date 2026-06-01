//! Proxy-side serde mirror of the backend `rule_graph` schema
//! (BACKEND CONTRACT §6, §7), VERBATIM. `Deserialize + Serialize`.
//!
//! A decision node's `processor` is an OPEN object: one snake_case `type`
//! discriminator (the canonical kind — `meta_tags` / `device_type` /
//! `article_url`) plus the remaining config fields as a flat snake_case map.
//! The `type` is used DIRECTLY as the JDM `CustomNode` kind == registry key ==
//! manifest `kind` (no camelCase mapping). Any node type — including ones added
//! later — deserializes without a graph.rs edit.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

/// One `CanvasGraph` per user class.
#[derive(Deserialize, Serialize, Clone, Debug, Default)]
pub struct RuleGraph {
    pub anonymous: CanvasGraph,
    pub registered: CanvasGraph,
    pub customer: CanvasGraph,
}

#[derive(Deserialize, Serialize, Clone, Debug, Default)]
pub struct CanvasGraph {
    pub nodes: Vec<Node>,
    pub edges: Vec<Edge>,
    #[serde(default)]
    pub root_node_id: Option<String>,
}

/// Internally-tagged by `kind`.
#[derive(Deserialize, Serialize, Clone, Debug)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Node {
    Decision {
        id: String,
        processor: ProcessorRef,
        position: Position,
    },
    Outcome {
        id: String,
        outcome_id: Uuid,
        position: Position,
    },
}

impl Node {
    /// The node id, regardless of variant.
    pub fn id(&self) -> &str {
        match self {
            Node::Decision { id, .. } => id,
            Node::Outcome { id, .. } => id,
        }
    }
}

/// Generic, open processor reference. The `type` field is the canonical
/// snake_case kind (used directly as the JDM `CustomNode` kind / registry key);
/// every other field is captured flat in `config` and passed through to the
/// processor unchanged. No fixed enum — new node types deserialize as-is.
#[derive(Deserialize, Serialize, Clone, Debug)]
pub struct ProcessorRef {
    /// The canonical kind discriminator (`meta_tags`, `device_type`, ...).
    #[serde(rename = "type")]
    pub kind: String,
    /// Remaining processor config fields (operator, value, tag_name, ...).
    #[serde(flatten)]
    pub config: Value,
}

#[derive(Deserialize, Serialize, Clone, Debug)]
pub struct Edge {
    pub id: String,
    pub source_node_id: String,
    pub target_node_id: String,
    pub branch: Branch,
}

#[derive(Deserialize, Serialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Branch {
    Yes,
    No,
}

#[derive(Deserialize, Serialize, Clone, Copy, Debug)]
pub struct Position {
    pub x: f64,
    pub y: f64,
}

/// User-class selector. Part of the compiled-cache key (canvas isolation).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Canvas {
    Anonymous,
    Registered,
    Customer,
}

impl RuleGraph {
    /// The `CanvasGraph` for the given user class.
    pub fn canvas(&self, c: Canvas) -> &CanvasGraph {
        match c {
            Canvas::Anonymous => &self.anonymous,
            Canvas::Registered => &self.registered,
            Canvas::Customer => &self.customer,
        }
    }
}
