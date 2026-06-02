//! `GraphEvaluator`: translate -> compile/cache -> zen `DecisionEngine` eval
//! (inside `spawn_blocking` + current-thread runtime, since `Variable` and
//! `scraper::Html` are `!Send`) -> recover the matched expression actions, in
//! trace order, from the routing trace.
//!
//! Routing stays in zen; BODY MUTATION moves to the forwarder, which folds the
//! returned `MatchedAction`s over the response body. The hot path runs zen with
//! `trace: true` (user-approved; small overhead) so the matched expression nodes
//! (and their order) can be recovered via the `__expr` suffix mapping.
//!
//! Cycle/depth protection is zen's via `EvaluationOptions.max_depth`. Any error /
//! empty canvas -> empty `Vec` (fail-open).

use std::collections::HashMap;
use std::sync::Arc;

use zen_engine::{DecisionEngine, EvaluationOptions};
use zen_expression::variable::Variable;

thread_local! {
    /// One current-thread Tokio runtime per blocking thread, built once and
    /// reused across `evaluate()` calls instead of rebuilt per feature/request.
    /// `enable_all` keeps the IO/timer drivers available, matching the
    /// previous per-call runtime exactly.
    static EVAL_RT: std::io::Result<tokio::runtime::Runtime> =
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build();
}

use crate::domain::adapter::CanvasNodeAdapter;
use crate::domain::context::{EvaluationContext, EvaluationContextParts};
use crate::domain::graph::{Canvas, CanvasGraph, Node};
use crate::domain::processors::ProcessorRegistry;
use crate::domain::translator::to_decision_content;
use crate::infra::compiled_cache::CompiledCache;
use zen_engine::model::DecisionContent;

pub struct GraphEvaluator<'a> {
    pub registry: Arc<ProcessorRegistry>,
    pub compiled: &'a CompiledCache,
}

/// One matched expression action on the routed path, in trace order. The
/// forwarder folds these over the response body (`json_apply::apply_action_json`
/// for JSON, `apply_action_html` for HTML).
#[derive(Debug, Clone)]
pub struct MatchedAction {
    /// Canvas node id of the expression node (e.g. `t_body`, `a_pw`).
    pub node_id: String,
    /// The expression's `action` config: `{ "type": "<kind>", <fields…> }`.
    pub action: serde_json::Value,
}

/// One step in the evaluation trace (canvas-level granularity, not JDM-level).
#[derive(Debug)]
pub struct TraceStep {
    /// Canvas node id (e.g. `n_meta`, `t_body`).
    pub node_id: String,
    /// `"decision"` or `"expression"`.
    pub kind: String,
    /// `Some(true)` = YES branch taken, `Some(false)` = NO branch taken.
    /// `None` for expression nodes.
    pub branch: Option<bool>,
    // Used for sorting during trace construction; not read afterward.
    #[allow(dead_code)]
    pub(crate) order: u32,
}

/// Full result of `evaluate_with_trace`.
#[derive(Debug)]
pub struct EvalTrace {
    pub steps: Vec<TraceStep>,
    /// Matched expression actions, in trace order (same as `evaluate()`).
    pub actions: Vec<MatchedAction>,
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

    /// Evaluate one canvas for a request. Returns the ordered list of matched
    /// expression actions (the expression nodes on the routed path, in trace
    /// order). Empty on dead-end / empty canvas / eval error (fail-open).
    pub async fn evaluate(
        &self,
        canvas: &CanvasGraph,
        ctx: EvaluationContextParts,
        feature_id: &str,
        version_number: i32,
        canvas_class: Canvas,
    ) -> Vec<MatchedAction> {
        // 1. Compiled DecisionContent (cache hit -> Arc clone; miss -> compile).
        let content =
            self.compiled
                .get_or_compile(feature_id, version_number, canvas_class, || {
                    to_decision_content(canvas)
                });

        // 2. Send-safe top-level input projection + the Arc'd registry.
        let input_value = ctx.to_input_value();
        let registry = self.registry.clone();

        // Canvas node id -> action config (only expression nodes). Used to map
        // recovered trace markers back to their action.
        let action_map: HashMap<String, serde_json::Value> = canvas
            .nodes
            .iter()
            .filter_map(|n| match n {
                Node::Expression { id, action, .. } => Some((id.clone(), action_to_value(action))),
                _ => None,
            })
            .collect();

        // 3. zen eval inside spawn_blocking + current-thread runtime. Only
        //    serde_json::Value crosses the boundary; scraper::Html + Variable are
        //    built INSIDE the closure. Trace is ON so we can recover the matched
        //    expression nodes (and their order) from the `__expr` markers.
        let rows: Vec<RawTraceRow> = tokio::task::spawn_blocking(move || {
            EVAL_RT.with(|rt| {
                let Ok(rt) = rt else {
                    return Vec::new();
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
                        trace: true,
                        max_depth: 10,
                    };
                    match decision
                        .evaluate_with_opts(Variable::from(input_value), opts)
                        .await
                    {
                        Ok(resp) => resp.trace.map_or_else(Vec::new, trace_rows),
                        Err(e) => {
                            tracing::warn!(error = %e, "eval=error");
                            Vec::new()
                        }
                    }
                })
            })
        })
        .await
        .unwrap_or_else(|e| {
            tracing::error!(error = %e, "eval_task_panic");
            Vec::new()
        });

        // 4. Recover matched expression actions, in trace order.
        ordered_actions(rows, &action_map)
    }

    /// Evaluate with trace enabled. Returns `EvalTrace` with the ordered traversal
    /// AND the matched expression actions. Used only by the `/__rre/eval` test
    /// endpoint — the hot proxy path uses `evaluate()` above.
    ///
    /// `content` is the already-translated `DecisionContent` (not cached, since
    /// this is a one-shot test eval against arbitrary user-supplied canvas data).
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
                    Node::Decision { .. } => "decision",
                    Node::Expression { .. } => "expression",
                    Node::Start { .. } => "start",
                    Node::End { .. } => "end",
                };
                (n.id().to_string(), kind)
            })
            .collect();

        // Canvas node id -> action config (only expression nodes).
        let action_map: HashMap<String, serde_json::Value> = canvas
            .nodes
            .iter()
            .filter_map(|n| match n {
                Node::Expression { id, action, .. } => Some((id.clone(), action_to_value(action))),
                _ => None,
            })
            .collect();

        // All crossing of the spawn_blocking boundary must be Send. DecisionGraphTrace
        // is !Send (contains Variable/Rc<str>), so we serialise to JSON inside.
        let rows: Result<Vec<RawTraceRow>, String> = tokio::task::spawn_blocking(move || {
            EVAL_RT.with(|rt| {
                let rt = rt.as_ref().map_err(|e| e.to_string())?;
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
                        Ok(resp) => Ok(resp.trace.map_or_else(Vec::new, trace_rows)),
                        Err(e) => Err(format!("eval error: {e}")),
                    }
                })
            })
        })
        .await
        .map_err(|e| format!("task panic: {e}"))
        .and_then(|r| r);

        let mut rows = rows?;

        // Sort rows by order (trace HashMap may return them in any order).
        rows.sort_by_key(|r| r.order);

        // Map JDM trace ids back to canvas ids.
        //   `<canvas_id>__proc` -> decision node  (carries branch output)
        //   `<canvas_id>__expr` -> expression node
        //   `<canvas_id>__switch`, `<canvas_id>__out`, `input` -> skipped
        let mut steps: Vec<TraceStep> = Vec::new();
        for row in &rows {
            let jdm_id = row.jdm_id.as_str();
            let (canvas_id, kind) = if let Some(s) = jdm_id.strip_suffix("__proc") {
                (s, "decision")
            } else if let Some(s) = jdm_id.strip_suffix("__expr") {
                (s, "expression")
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

        let actions = ordered_actions(rows, &action_map);

        Ok(EvalTrace { steps, actions })
    }
}

/// The expression `action` as a flat `{ "type": "<kind>", <fields…> }` JSON value.
fn action_to_value(action: &crate::domain::graph::ProcessorRef) -> serde_json::Value {
    let mut map = match action.config.clone() {
        serde_json::Value::Object(m) => m,
        _ => serde_json::Map::new(),
    };
    map.insert(
        "type".to_string(),
        serde_json::Value::String(action.kind.clone()),
    );
    serde_json::Value::Object(map)
}

/// Serialise zen trace entries into the Send-safe rows used to recover matched
/// expression nodes. Called INSIDE `spawn_blocking` before the thread boundary.
/// Generic over the map hasher (zen's trace uses `ahash::RandomState`).
fn trace_rows<S>(trace: HashMap<Arc<str>, zen_engine::DecisionGraphTrace, S>) -> Vec<RawTraceRow> {
    trace
        .into_values()
        .map(|entry| RawTraceRow {
            jdm_id: entry.id.as_ref().to_string(),
            order: entry.order,
            output_json: serde_json::to_value(&entry.output).ok(),
        })
        .collect()
}

/// Recover the matched expression actions, in trace order, from the trace rows.
/// Each `<canvas_id>__expr` row whose `<canvas_id>` resolves to an action becomes
/// a `MatchedAction`.
fn ordered_actions(
    mut rows: Vec<RawTraceRow>,
    action_map: &HashMap<String, serde_json::Value>,
) -> Vec<MatchedAction> {
    rows.sort_by_key(|r| r.order);
    let mut actions = Vec::new();
    for row in &rows {
        let Some(canvas_id) = row.jdm_id.strip_suffix("__expr") else {
            continue;
        };
        if let Some(action) = action_map.get(canvas_id) {
            actions.push(MatchedAction {
                node_id: canvas_id.to_string(),
                action: action.clone(),
            });
        }
    }
    actions
}
