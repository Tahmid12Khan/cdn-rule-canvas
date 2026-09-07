//! Domain layer.
//!
//! The rule model, evaluator and applier now live in the shared `rre-core`
//! crate so the proxy, the Test panel and the Fastly edge service run identical
//! code. What remains proxy-specific is the async wrapper around evaluation
//! (`evaluator`) and the matched-feature reporting (`features_matched`); the
//! re-exports below keep the rest of the proxy's import paths unchanged.

pub mod evaluator;
pub mod features_matched;

pub use rre_core::{adapter, applier, bundle, context, graph, identity, processors, translator};

pub use context::{DeviceType, EvaluationContext, EvaluationContextParts};
