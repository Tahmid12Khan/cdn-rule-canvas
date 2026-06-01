//! `GraphEvaluator`: translate -> compile/cache -> zen `DecisionEngine` eval
//! (inside `spawn_blocking` + current-thread runtime, since `Variable` and
//! `scraper::Html` are `!Send`) -> extract `outcomeId`.
//!
//! Cycle/depth protection is zen's via `EvaluationOptions.max_depth` — no second
//! guard. Any error / absent outcome -> `None` (fail-open).

use std::collections::HashMap;
use std::sync::Arc;

use uuid::Uuid;
use zen_engine::{DecisionEngine, EvaluationOptions};
use zen_expression::variable::Variable;

use crate::domain::adapter::CanvasNodeAdapter;
use crate::domain::context::{EvaluationContext, EvaluationContextParts};
use crate::domain::graph::{Canvas, CanvasGraph};
use crate::domain::processors::ProcessorRegistry;
use crate::domain::translator::to_decision_content;
use crate::infra::compiled_cache::CompiledCache;
use zen_engine::model::DecisionContent;

pub struct GraphEvaluator<'a> {
    pub registry: Arc<ProcessorRegistry>,
    pub compiled: &'a CompiledCache,
}

/// One step in the evaluation trace (canvas-level granularity, not JDM-level).
#[derive(Debug)]
pub struct TraceStep {
    /// Canvas node id (e.g. `n_meta`, `n_paywall`).
    pub node_id: String,
    /// `"decision"` or `"outcome"`.
    pub kind: String,
    /// `Some(true)` = YES branch taken, `Some(false)` = NO branch taken.
    /// `None` for outcome nodes.
    pub branch: Option<bool>,
    // Used for sorting during trace construction; not read afterward.
    #[allow(dead_code)]
    pub(crate) order: u32,
}

/// Full result of `evaluate_with_trace`.
#[derive(Debug)]
pub struct EvalTrace {
    pub matched_outcome_id: Option<Uuid>,
    pub steps: Vec<TraceStep>,
}

/// Send-safe trace row extracted inside `spawn_blocking` before the boundary.
struct RawTraceRow {
    jdm_id: String,
    order: u32,
    /// The processor's output, serialized to JSON inside the closure.
    output_json: Option<serde_json::Value>,
}

impl<'a> GraphEvaluator<'a> {
    pub fn new(registry: Arc<ProcessorRegistry>, compiled: &'a CompiledCache) -> Self {
        Self { registry, compiled }
    }

    /// Evaluate one canvas for a request. Returns the resolved outcome id, or
    /// `None` on dead-end / empty canvas / eval error (fail-open).
    pub async fn evaluate(
        &self,
        canvas: &CanvasGraph,
        ctx: EvaluationContextParts,
        feature_id: &str,
        version_number: i32,
        canvas_class: Canvas,
    ) -> Option<Uuid> {
        // 1. Compiled DecisionContent (cache hit -> Arc clone; miss -> compile).
        let content =
            self.compiled
                .get_or_compile(feature_id, version_number, canvas_class, || {
                    to_decision_content(canvas)
                });

        // 2. Send-safe top-level input projection + the Arc'd registry.
        let input_value = ctx.to_input_value();
        let registry = self.registry.clone();

        // 3. zen eval inside spawn_blocking + current-thread runtime. Only
        //    serde_json::Value crosses the boundary; scraper::Html + Variable are
        //    built INSIDE the closure.
        let result: Option<serde_json::Value> = tokio::task::spawn_blocking(move || {
            let rt = match tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
            {
                Ok(rt) => rt,
                Err(_) => return None,
            };

            rt.block_on(async move {
                let eval_ctx = ctx.into_context();
                let adapter = CanvasNodeAdapter {
                    registry,
                    ctx: Arc::new(eval_ctx),
                };
                let engine = DecisionEngine::default().with_adapter(Arc::new(adapter));
                let decision = engine.create_decision(content);
                let opts = EvaluationOptions {
                    trace: false,
                    max_depth: 10,
                };
                match decision
                    .evaluate_with_opts(Variable::from(input_value), opts)
                    .await
                {
                    Ok(resp) => serde_json::to_value(&resp.result).ok(),
                    Err(e) => {
                        tracing::warn!(error = %e, "eval=error");
                        None
                    }
                }
            })
        })
        .await
        .unwrap_or_else(|e| {
            tracing::error!(error = %e, "eval_task_panic");
            None
        });

        // 4. Extract outcomeId -> Uuid.
        result
            .as_ref()
            .and_then(|v| v.get("outcomeId"))
            .and_then(|v| v.as_str())
            .and_then(|s| Uuid::parse_str(s).ok())
    }

    /// Evaluate with trace enabled. Returns `EvalTrace` containing the matched
    /// outcome id and the ordered traversal. Used only by the `/__rre/eval`
    /// test endpoint — the hot proxy path uses `evaluate()` above (no trace overhead).
    ///
    /// `content` is the already-translated `DecisionContent` (not cached, since
    /// this is a one-shot test eval against arbitrary user-supplied canvas data).
    ///
    /// Note: `DecisionGraphTrace.output` is `Variable` which is `!Send`. We
    /// serialise every trace entry to `serde_json::Value` INSIDE `spawn_blocking`
    /// before crossing the thread boundary.
    pub async fn evaluate_with_trace(
        &self,
        canvas: &CanvasGraph,
        content: DecisionContent,
        ctx: Arc<EvaluationContext>,
    ) -> Result<EvalTrace, String> {
        let input_value = ctx.to_input_value();
        let registry = self.registry.clone();

        // Build a map of canvas node_id -> kind for post-processing.
        let canvas_kind_map: HashMap<String, &str> = canvas
            .nodes
            .iter()
            .map(|n| {
                let kind = match n {
                    crate::domain::graph::Node::Decision { .. } => "decision",
                    crate::domain::graph::Node::Outcome { .. } => "outcome",
                };
                (n.id().to_string(), kind)
            })
            .collect();

        // All crossing of the spawn_blocking boundary must be Send. DecisionGraphTrace
        // is !Send (contains Variable/Rc<str>), so we serialise to JSON inside.
        let result: Result<(Option<serde_json::Value>, Vec<RawTraceRow>), String> =
            tokio::task::spawn_blocking(move || {
                let rt = tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                    .map_err(|e| e.to_string())?;

                rt.block_on(async move {
                    let adapter = CanvasNodeAdapter { registry, ctx };
                    let engine = DecisionEngine::default().with_adapter(Arc::new(adapter));

                    let mut compiled_content = content;
                    compiled_content.compile();
                    let decision = engine.create_decision(Arc::new(compiled_content));
                    let opts = EvaluationOptions {
                        trace: true,
                        max_depth: 10,
                    };
                    match decision
                        .evaluate_with_opts(Variable::from(input_value), opts)
                        .await
                    {
                        Ok(resp) => {
                            let result_json = serde_json::to_value(&resp.result).ok();
                            // Serialise trace entries to JSON before the thread boundary.
                            let rows: Vec<RawTraceRow> = resp.trace.map_or_else(Vec::new, |t| {
                                t.into_values()
                                    .map(|entry| RawTraceRow {
                                        jdm_id: entry.id.as_ref().to_string(),
                                        order: entry.order,
                                        output_json: serde_json::to_value(&entry.output).ok(),
                                    })
                                    .collect()
                            });
                            Ok((result_json, rows))
                        }
                        Err(e) => Err(format!("eval error: {e}")),
                    }
                })
            })
            .await
            .map_err(|e| format!("task panic: {e}"))
            .and_then(|r| r);

        let (result_json, mut rows) = result?;

        let matched_outcome_id = result_json
            .as_ref()
            .and_then(|v| v.get("outcomeId"))
            .and_then(|v| v.as_str())
            .and_then(|s| Uuid::parse_str(s).ok());

        // Sort rows by order (trace HashMap may return them in any order).
        rows.sort_by_key(|r| r.order);

        // Map JDM trace ids back to canvas ids.
        // JDM id suffixes:
        //   `<canvas_id>__proc`  -> decision node  (carries branch output)
        //   `<canvas_id>__expr`  -> outcome node
        //   `<canvas_id>__switch`, `<canvas_id>__out`, `input` -> skipped
        let mut steps: Vec<TraceStep> = Vec::new();
        for row in &rows {
            let jdm_id = row.jdm_id.as_str();
            let (canvas_id, kind) = if let Some(s) = jdm_id.strip_suffix("__proc") {
                (s, "decision")
            } else if let Some(s) = jdm_id.strip_suffix("__expr") {
                (s, "outcome")
            } else {
                continue; // __switch, __out, input
            };

            if !canvas_kind_map.contains_key(canvas_id) {
                continue;
            }

            let branch = if kind == "decision" {
                row.output_json
                    .as_ref()
                    .and_then(|v| v.get("branch"))
                    .and_then(|v| v.as_str())
                    .map(|s| s == "yes")
            } else {
                None
            };

            steps.push(TraceStep {
                node_id: canvas_id.to_string(),
                kind: kind.to_string(),
                branch,
                order: row.order,
            });
        }

        Ok(EvalTrace {
            matched_outcome_id,
            steps,
        })
    }
}
