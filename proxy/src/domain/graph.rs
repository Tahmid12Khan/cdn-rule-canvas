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

#[derive(Deserialize, Serialize, Clone, Debug, Default)]
pub struct RuleGraph {
    pub canvas: CanvasGraph,
}

#[derive(Deserialize, Serialize, Clone, Debug, Default)]
pub struct CanvasGraph {
    pub nodes: Vec<Node>,
    pub edges: Vec<Edge>,
    #[serde(default)]
    pub root_node_id: Option<String>,
}

/// Internally-tagged by `kind`. Mirrors the backend `rule_graph.rs` `Node` enum
/// BYTE-IDENTICALLY (minus `ToSchema`/doc): `start` / `decision` / `expression`
/// / `end`. The `ProcessorRef` shape is REUSED for the expression `action`.
#[derive(Deserialize, Serialize, Clone, Debug)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Node {
    Start {
        id: String,
        position: Position,
    },
    Decision {
        id: String,
        processor: ProcessorRef,
        position: Position,
    },
    Expression {
        id: String,
        action: ProcessorRef,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        custom_label: Option<String>,
        position: Position,
    },
    End {
        id: String,
        position: Position,
    },
}

impl Node {
    /// The node id, regardless of variant.
    pub fn id(&self) -> &str {
        match self {
            Node::Start { id, .. } => id,
            Node::Decision { id, .. } => id,
            Node::Expression { id, .. } => id,
            Node::End { id, .. } => id,
        }
    }

    pub fn is_start(&self) -> bool {
        matches!(self, Node::Start { .. })
    }

    pub fn is_end(&self) -> bool {
        matches!(self, Node::End { .. })
    }

    pub fn is_expression(&self) -> bool {
        matches!(self, Node::Expression { .. })
    }

    pub fn is_decision(&self) -> bool {
        matches!(self, Node::Decision { .. })
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
