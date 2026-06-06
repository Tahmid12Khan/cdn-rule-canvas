//! The `CanvasProcessor` trait, the `ProcessorRegistry`, and the shared
//! outcome/error types (BACKEND CONTRACT §8.3). Built once at startup and held
//! in `AppState` behind an `Arc`; immutable thereafter.

pub mod article_url;
pub mod device_type;
pub mod json_expression;
pub mod meta_tags;
pub mod site_match;

use std::collections::HashMap;
use std::sync::Arc;

use zen_expression::variable::Variable;

use crate::domain::context::EvaluationContext;

/// Branch result of a decision processor.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Branch {
    Yes,
    No,
}

impl Branch {
    fn as_str(&self) -> &'static str {
        match self {
            Branch::Yes => "yes",
            Branch::No => "no",
        }
    }
}

/// A processor's output. Converted into the zen `NodeResponse.output` Variable
/// `{ "branch": "yes" | "no" }`, consumed by the downstream SwitchNode (`$.branch`).
#[derive(Debug)]
pub struct ProcessorOutcome {
    pub branch: Branch,
}

impl ProcessorOutcome {
    pub fn into_variable(self) -> Variable {
        let value = serde_json::json!({ "branch": self.branch.as_str() });
        Variable::from(value)
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ProcessorError {
    #[error("unknown processor kind: {0}")]
    UnknownKind(String),
    #[error("invalid config: {0}")]
    Config(String),
}

/// One decision processor (e.g. meta_tags, device_type). Pure + sync; CPU-only.
/// No globals; reads `ctx` read-only.
pub trait CanvasProcessor: Send + Sync {
    /// Stable key = JDM CustomNode content.kind = registry key.
    fn kind(&self) -> &'static str;

    /// `config` = the processor's flat config map as JSON. `ctx` is read-only.
    fn evaluate(
        &self,
        config: &serde_json::Value,
        ctx: &EvaluationContext,
    ) -> Result<ProcessorOutcome, ProcessorError>;
}

/// Registry of processors keyed by `CanvasProcessor::kind()`.
#[derive(Default)]
pub struct ProcessorRegistry {
    map: HashMap<&'static str, Arc<dyn CanvasProcessor>>,
}

// `dyn CanvasProcessor` is not `Debug`; the adapter requires `Debug`, so we
// implement it manually (listing the registered kinds, never secrets).
impl std::fmt::Debug for ProcessorRegistry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ProcessorRegistry")
            .field("kinds", &self.map.keys().collect::<Vec<_>>())
            .finish()
    }
}

impl ProcessorRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a processor, keyed off `p.kind()`.
    pub fn register(&mut self, p: Arc<dyn CanvasProcessor>) {
        self.map.insert(p.kind(), p);
    }

    /// Look up a processor by kind.
    pub fn get(&self, kind: &str) -> Option<Arc<dyn CanvasProcessor>> {
        self.map.get(kind).cloned()
    }
}

/// Built once at startup: registers the MetaTags + DeviceType processors.
/// Adding a processor = `impl CanvasProcessor` + one `register` line here.
pub fn default_registry() -> ProcessorRegistry {
    let mut registry = ProcessorRegistry::new();
    registry.register(Arc::new(meta_tags::MetaTagsProcessor));
    registry.register(Arc::new(device_type::DeviceTypeProcessor));
    registry.register(Arc::new(article_url::ArticleUrlProcessor));
    registry.register(Arc::new(json_expression::JsonExpressionProcessor));
    registry.register(Arc::new(site_match::SiteMatchProcessor));
    registry
}
