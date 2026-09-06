//! Domain layer: pure evaluation (graph mirror, translator, zen adapter,
//! evaluator, processors) and pure transformation (applier). No
//! network IO lives here — the forwarder ties domain to infra.

pub mod adapter;
pub mod applier;
pub mod context;
pub mod evaluator;
pub mod features_matched;
pub mod graph;
pub mod identity;
pub mod processors;
pub mod translator;

pub use context::{DeviceType, EvaluationContext, EvaluationContextParts};
