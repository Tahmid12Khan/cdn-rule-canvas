//! The `CanvasProcessor` trait, the `ProcessorRegistry`, and the shared
//! outcome/error types (BACKEND CONTRACT §8.3). Built once at startup and held
//! in `AppState` behind an `Arc`; immutable thereafter.

pub mod article_url;
pub mod device_type;
pub mod has_product;
pub mod json_expression;
pub mod logged_in;
pub mod meta_tags;
pub mod site_match;

use std::collections::HashMap;
use std::sync::Arc;

use zen_expression::variable::Variable;

use crate::context::EvaluationContext;

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
///
/// Deliberately NOT `#[derive(Default)]`: a derived Default yields an EMPTY
/// registry, and an empty registry makes every decision node fail to resolve
/// its processor, so a canvas silently never matches. `Default` therefore
/// delegates to [`default_registry`] and `new()` stays explicit for the rare
/// caller that genuinely wants an empty one.
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
    /// An EMPTY registry. Constructs the map directly rather than delegating to
    /// `Default`, which now builds the fully-populated one.
    pub fn new() -> Self {
        Self {
            map: HashMap::new(),
        }
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
impl Default for ProcessorRegistry {
    fn default() -> Self {
        default_registry()
    }
}

pub fn default_registry() -> ProcessorRegistry {
    let mut registry = ProcessorRegistry::new();
    registry.register(Arc::new(meta_tags::MetaTagsProcessor));
    registry.register(Arc::new(device_type::DeviceTypeProcessor));
    registry.register(Arc::new(article_url::ArticleUrlProcessor));
    registry.register(Arc::new(json_expression::JsonExpressionProcessor));
    registry.register(Arc::new(site_match::SiteMatchProcessor));
    registry.register(Arc::new(logged_in::LoggedInProcessor));
    registry.register(Arc::new(has_product::HasProductProcessor));
    registry
}

#[cfg(test)]
mod registry_tests {
    use super::*;

    /// Every kind the canvas editor can emit must resolve. An unregistered kind
    /// is not an error at eval time — the decision node just fails and the
    /// canvas takes no branch — so only a test like this catches a missed
    /// `register` line.
    #[test]
    fn default_registry_has_every_shipped_processor() {
        let registry = ProcessorRegistry::default();
        for kind in [
            "meta_tags",
            "device_type",
            "article_url",
            "json_expression",
            "site_match",
            "logged_in",
            "has_product",
        ] {
            assert!(
                registry.get(kind).is_some(),
                "processor `{kind}` is not registered"
            );
        }
    }

    /// The trap this type's doc comment warns about: `default()` must NOT be
    /// the derived, empty one.
    #[test]
    fn default_is_not_empty() {
        assert!(ProcessorRegistry::default().get("has_product").is_some());
        assert!(ProcessorRegistry::new().get("has_product").is_none());
    }
}
