//! Proxy-side serde mirror of the backend `rule_graph` schema
//! (BACKEND CONTRACT §6), VERBATIM. `Deserialize + Serialize`.
//!
//! IMPORTANT: the canvas `ProcessorConfig` discriminator `type` is snake_case
//! (`meta_tags` / `device_type`); the JDM `CustomNode` kind is camelCase
//! (`metaTags` / `deviceType`). `ProcessorConfig::kind_key` bridges the two.

use serde::{Deserialize, Serialize};
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
        processor: ProcessorConfig,
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

/// Internally-tagged by `type`.
#[derive(Deserialize, Serialize, Clone, Debug)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ProcessorConfig {
    MetaTags {
        tag_name: String,
        operator: MetaTagsOperator,
        #[serde(default)]
        value: Option<String>,
    },
    DeviceType {
        operator: DeviceOperator,
        value: DeviceValue,
    },
    ArticleUrl {
        operator: ArticleUrlOperator,
        value: String,
    },
}

impl ProcessorConfig {
    /// The JDM `CustomNode` content kind (camelCase). Equals the registry key.
    pub fn kind_key(&self) -> &'static str {
        match self {
            ProcessorConfig::MetaTags { .. } => "metaTags",
            ProcessorConfig::DeviceType { .. } => "deviceType",
            ProcessorConfig::ArticleUrl { .. } => "articleUrl",
        }
    }

    /// The full processor config serialized as JSON (`{"type":"meta_tags",...}`),
    /// stored on the JDM `CustomNode` content.config and read by the processor.
    pub fn to_config_value(&self) -> serde_json::Value {
        serde_json::to_value(self).unwrap_or(serde_json::Value::Null)
    }
}

#[derive(Deserialize, Serialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum MetaTagsOperator {
    Contains,
    Equals,
    Exists,
}

#[derive(Deserialize, Serialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DeviceOperator {
    Equals,
    Contains,
}

#[derive(Deserialize, Serialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ArticleUrlOperator {
    Contains,
    Matches,
    StartsWith,
    Equals,
}

#[derive(Deserialize, Serialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DeviceValue {
    Mobile,
    Desktop,
    Tablet,
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
