//! Proxy-side async wrapper around `rre-core`'s synchronous evaluator.
//!
//! Evaluation itself lives in `rre_core::evaluate_sync`. What stays here is the
//! proxy's THREADING model, which the edge service has no equivalent of:
//! `spawn_blocking` keeps a multi-millisecond evaluation off the async workers,
//! and zen's `Variable` plus `scraper::Html` are `!Send`, so they are built and
//! dropped entirely inside the closure and never cross an await point.

use std::sync::Arc;

use rre_core::context::{EvaluationContext, EvaluationContextParts};
use rre_core::graph::CanvasGraph;
use rre_core::processors::ProcessorRegistry;
// Re-exported so `crate::domain::evaluator::{EvalTrace, MatchedAction,
// TraceStep}` keeps resolving for the forwarder and the eval endpoint.
pub use rre_core::{EvalTrace, MatchedAction, TraceStep};
use zen_engine::model::DecisionContent;

use crate::infra::compiled_cache::CompiledCache;

pub struct GraphEvaluator<'a> {
    pub registry: Arc<ProcessorRegistry>,
    pub compiled: &'a CompiledCache,
}

impl<'a> GraphEvaluator<'a> {
    pub fn new(registry: Arc<ProcessorRegistry>, compiled: &'a CompiledCache) -> Self {
        Self { registry, compiled }
    }

    /// Evaluate one canvas for a request. Returns the matched expression actions
    /// in trace order; empty on a dead end, an empty canvas or an eval error
    /// (fail-open).
    pub async fn evaluate(
        &self,
        canvas: &CanvasGraph,
        ctx: EvaluationContextParts,
        feature_id: &str,
        version_number: i32,
    ) -> Vec<MatchedAction> {
        // Cache hit -> Arc clone; miss -> translate + compile once per version.
        let content = self
            .compiled
            .get_or_compile(feature_id, version_number, || rre_core::compile(canvas));
        let registry = self.registry.clone();
        let canvas = canvas.clone();
        tokio::task::spawn_blocking(move || {
            rre_core::evaluate_sync(content, &canvas, ctx, registry)
        })
        .await
        .unwrap_or_else(|e| {
            tracing::error!(error = %e, "eval_task_panic");
            Vec::new()
        })
    }

    /// Evaluate with the full traversal recovered. Used only by the
    /// `/__rre/eval` test endpoint: `content` is a one-shot translation of
    /// user-supplied canvas data, so it is not cached.
    pub async fn evaluate_with_trace(
        &self,
        canvas: &CanvasGraph,
        content: DecisionContent,
        ctx: Arc<EvaluationContext>,
    ) -> Result<EvalTrace, String> {
        let registry = self.registry.clone();
        let canvas = canvas.clone();
        tokio::task::spawn_blocking(move || {
            rre_core::evaluate_with_trace_sync(&canvas, content, ctx, registry)
        })
        .await
        .map_err(|e| format!("task panic: {e}"))?
    }
}
