//! Rule evaluation: translate a canvas to JDM, run zen's `DecisionEngine`, and
//! recover the matched expression actions in trace order from the `__expr`
//! markers.
//!
//! SYNCHRONOUS on purpose. zen's `evaluate` is `async` but performs no I/O for
//! the node kinds RRE emits (input / switch / custom / expression / output), so
//! it is driven with `futures::executor::block_on` and needs no runtime. That is
//! what lets this crate run inside a Fastly Compute Wasm guest, which has
//! neither tokio nor threads. The proxy keeps its own `spawn_blocking` wrapper
//! around these functions: zen's `Variable` and `scraper::Html` are `!Send` and
//! must not cross an await point.
//!
//! Routing stays in zen; BODY MUTATION is the caller's job — it folds the
//! returned `MatchedAction`s over the response body. Cycle/depth protection is
//! zen's via `EvaluationOptions.max_depth`. Any error or empty canvas yields an
//! empty `Vec` (fail-open).

use std::collections::HashMap;
use std::sync::Arc;

use zen_engine::model::DecisionContent;
use zen_engine::{DecisionEngine, EvaluationOptions};
use zen_expression::variable::Variable;

use crate::adapter::CanvasNodeAdapter;
use crate::context::{EvaluationContext, EvaluationContextParts};
use crate::graph::{CanvasGraph, Node};
use crate::processors::ProcessorRegistry;
use crate::translator::to_decision_content;

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

/// Translate a canvas to JDM and COMPILE it.
///
/// The `compile()` step is not optional: it builds the switch nodes' condition
/// expressions. Skip it and every decision node silently takes no branch, so a
/// canvas with a decision evaluates to "no match" while a linear one still
/// works — a failure mode that hides in exactly the rules that matter.
///
/// Caching is the CALLER's job: the proxy wraps this in its moka
/// `CompiledCache`; a Wasm guest calls it per request (microseconds, and a
/// Compute instance has no memory across requests anyway).
pub fn compile(canvas: &CanvasGraph) -> DecisionContent {
    let mut content = to_decision_content(canvas);
    content.compile();
    content
}

/// Evaluate one canvas. Returns the matched expression actions in trace order;
/// an empty vec on a dead end, an empty canvas, or an eval error (fail-open).
pub fn evaluate_sync(
    content: Arc<DecisionContent>,
    canvas: &CanvasGraph,
    ctx: EvaluationContextParts,
    registry: Arc<ProcessorRegistry>,
) -> Vec<MatchedAction> {
    let input_value = ctx.to_input_value();
    let action_map = expression_action_map(canvas);

    let rows: Vec<RawTraceRow> = futures::executor::block_on(async move {
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
    });

    ordered_actions(rows, &action_map)
}

/// Evaluate with the full traversal recovered, not just the matched actions.
/// Backs the `/__rre/eval` test endpoint; `content` is a one-shot translation of
/// user-supplied canvas data, so it is compiled here rather than cached.
pub fn evaluate_with_trace_sync(
    canvas: &CanvasGraph,
    content: DecisionContent,
    ctx: Arc<EvaluationContext>,
    registry: Arc<ProcessorRegistry>,
) -> Result<EvalTrace, String> {
    let input_value = ctx.to_input_value();
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
    let action_map = expression_action_map(canvas);

    let mut rows: Vec<RawTraceRow> = futures::executor::block_on(async move {
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
    })?;

    // The trace map returns rows in arbitrary order; `order` is the real one.
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

/// Canvas node id -> action config, for expression nodes only. Used to map a
/// recovered `__expr` trace marker back to the action it stands for.
fn expression_action_map(canvas: &CanvasGraph) -> HashMap<String, serde_json::Value> {
    canvas
        .nodes
        .iter()
        .filter_map(|n| match n {
            Node::Expression { id, action, .. } => Some((id.clone(), action_to_value(action))),
            _ => None,
        })
        .collect()
}

/// The expression `action` as a flat `{ "type": "<kind>", <fields…> }` JSON value.
fn action_to_value(action: &crate::graph::ProcessorRef) -> serde_json::Value {
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
