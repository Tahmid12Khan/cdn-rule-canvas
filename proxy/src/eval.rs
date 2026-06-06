//! `POST /__rre/eval` — test-eval endpoint.
//!
//! Accepts a canvas + context from the request body (no upstream fetch), runs
//! `GraphEvaluator::evaluate_with_trace`, and returns the ordered traversal
//! (node ids, edge ids, per-step detail) plus the matched expression actions and
//! a **Transformation Journey** (spec §5): the body after each node on the
//! matched path.
//!
//! CORS: handled at the router layer (see `lib.rs`). This handler is pure axum.

use std::collections::HashMap;
use std::sync::Arc;

use axum::extract::State;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::Json;
use http::HeaderMap;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::domain::applier::json_apply;
use crate::domain::context::{DeviceType, EvaluationContext};
use crate::domain::evaluator::{EvalTrace, GraphEvaluator};
use crate::domain::features_matched::{self, FeatureEntry, NodeTiming};
use crate::domain::graph::{CanvasGraph, Node};
use crate::domain::translator::to_decision_content;
use crate::state::AppState;

// ---------------------------------------------------------------------------
// Request / Response types
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
pub struct EvalRequest {
    pub canvas: CanvasGraph,
    pub context: EvalContext,
}

#[derive(Deserialize)]
pub struct EvalContext {
    pub device_type: Option<DeviceTypeInput>,
    pub user_agent: Option<String>,
    pub meta_tags: Option<HashMap<String, String>>,
    pub path: Option<String>,
    pub url: Option<String>,
    /// Response JSON body for testing `json_expression` nodes (JSON features).
    /// When present, the built context's `response_json` is populated so JSONPath
    /// queries resolve. Takes precedence over `response_body` when both are set.
    #[serde(default)]
    pub response_json: Option<serde_json::Value>,
    /// Raw response body string. Interpreted per `content_kind`: when
    /// `content_kind == "json"`, it is parsed into `response_json`.
    #[serde(default)]
    pub response_body: Option<String>,
    /// `"html"` (default) or `"json"`. Selects how `response_body` is interpreted.
    #[serde(default)]
    pub content_kind: Option<String>,
    /// Simulated matched Site slug for `site_match` nodes. `None` mirrors a
    /// fallback request (no Site matched).
    #[serde(default)]
    pub site: Option<String>,
}

/// The wire `device_type` field on the request (matches frontend enum).
#[derive(Deserialize, Clone, Copy)]
#[serde(rename_all = "lowercase")]
pub enum DeviceTypeInput {
    Mobile,
    Desktop,
    Tablet,
}

impl From<DeviceTypeInput> for DeviceType {
    fn from(d: DeviceTypeInput) -> Self {
        match d {
            DeviceTypeInput::Mobile => DeviceType::Mobile,
            DeviceTypeInput::Desktop => DeviceType::Desktop,
            DeviceTypeInput::Tablet => DeviceType::Tablet,
        }
    }
}

#[derive(Serialize)]
pub struct EvalResponse {
    pub matched_node_id: Option<String>,
    pub traversed_node_ids: Vec<String>,
    pub traversed_edge_ids: Vec<String>,
    pub steps: Vec<EvalStep>,
    /// Transformation Journey (spec §5): the body after each node on the matched
    /// path, in trace order.
    pub journey: Vec<JourneyEntry>,
    /// Timing summary for the single canvas under test (spec §5/§8) — built
    /// exactly like one feature's `features_matched` entry. `None` when no
    /// expression node matched (the path went straight to END).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub summary: Option<FeatureEntry>,
}

#[derive(Serialize)]
pub struct EvalStep {
    pub node_id: String,
    pub kind: String,
    pub branch: Option<String>,
    pub result: Option<bool>,
}

/// One step of the Transformation Journey.
#[derive(Serialize)]
pub struct JourneyEntry {
    pub index: usize,
    pub node_id: String,
    pub kind: String,
    pub label: String,
    pub branch: Option<bool>,
    /// The body AFTER this node's action is applied. JSON value for JSON
    /// features, a string for HTML.
    pub body_after: Value,
    /// Per-node apply time, `d.dd` (spec §5). Expression steps carry their own
    /// apply duration; start/decision/end are `"0.00"`.
    pub time_ms: String,
}

// ---------------------------------------------------------------------------
// Handler
// ---------------------------------------------------------------------------

pub async fn eval_handler(
    State(state): State<AppState>,
    Json(req): Json<EvalRequest>,
) -> impl IntoResponse {
    // Build EvaluationContext directly from the request body — no upstream fetch.
    let device = determine_device(&req.context);
    let meta_tags = req.context.meta_tags.clone().unwrap_or_default();
    let path = req.context.path.clone().unwrap_or_default();
    let response_json = determine_response_json(&req.context);

    let ctx = EvaluationContext {
        request_headers: HeaderMap::new(),
        request_path: path,
        request_cookies: HashMap::new(),
        device,
        meta_tags,
        response_json,
        site: req.context.site.clone(),
    };

    // Translate the canvas to JDM DecisionContent (same path as production eval).
    let content = match std::panic::catch_unwind(|| to_decision_content(&req.canvas)) {
        Ok(c) => c,
        Err(_) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({"error": "canvas translation failed"})),
            )
                .into_response();
        }
    };

    let evaluator = GraphEvaluator::new(state.registry.clone(), &state.compiled);
    let eval_start = std::time::Instant::now();
    let trace_result = evaluator
        .evaluate_with_trace(&req.canvas, content, Arc::new(ctx))
        .await;
    let eval_ms = eval_start.elapsed().as_secs_f64() * 1000.0;

    let trace_result = match trace_result {
        Ok(t) => t,
        Err(msg) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({"error": msg})),
            )
                .into_response();
        }
    };

    // Build response from trace.
    let response = build_response(&state, &req, trace_result, eval_ms);
    (
        StatusCode::OK,
        Json(serde_json::to_value(response).unwrap_or_default()),
    )
        .into_response()
}

// ---------------------------------------------------------------------------
// Context construction helpers
// ---------------------------------------------------------------------------

fn determine_device(ctx: &EvalContext) -> DeviceType {
    // Explicit device_type field wins; fall back to user_agent parsing.
    if let Some(dt) = ctx.device_type {
        return dt.into();
    }
    if let Some(ua) = &ctx.user_agent {
        return DeviceType::from_user_agent(ua);
    }
    DeviceType::Desktop
}

/// Resolve the response JSON body for `json_expression` tests. Precedence:
/// explicit `response_json` value > `response_body` parsed as JSON when
/// `content_kind == "json"`. Returns `None` for HTML / unparseable input
/// (json_expression then evaluates to No, mirroring an HTML response).
fn determine_response_json(ctx: &EvalContext) -> Option<serde_json::Value> {
    if let Some(v) = &ctx.response_json {
        return Some(v.clone());
    }
    let is_json = ctx
        .content_kind
        .as_deref()
        .map(|k| k.eq_ignore_ascii_case("json"))
        .unwrap_or(false);
    if is_json {
        if let Some(body) = &ctx.response_body {
            return serde_json::from_str(body).ok();
        }
    }
    None
}

// ---------------------------------------------------------------------------
// Response builder
// ---------------------------------------------------------------------------

fn build_response(
    state: &AppState,
    req: &EvalRequest,
    trace: EvalTrace,
    eval_ms: f64,
) -> EvalResponse {
    let canvas = &req.canvas;
    // trace.steps is ordered by zen's `order` field: each entry has the canvas
    // node id and the branch output.
    let traversed_node_ids: Vec<String> = trace.steps.iter().map(|s| s.node_id.clone()).collect();

    // Derive traversed edge ids from consecutive (source, target) pairs in the traversal.
    let traversed_edge_ids = derive_edge_ids(canvas, &traversed_node_ids, &trace.steps);

    // The terminal "matched" node is the last expression node on the path.
    let matched_node_id = trace
        .steps
        .iter()
        .rev()
        .find(|s| s.kind == "expression")
        .map(|s| s.node_id.clone());

    let (journey, timings) = build_journey(state, req, &trace);

    // Summary (spec §5/§8): built exactly like one feature's entry, for the
    // single canvas under test. `None` when no expression node matched.
    let summary = features_matched::build_entry(&timings, eval_ms);

    let steps = trace
        .steps
        .into_iter()
        .map(|s| {
            let branch = s.branch.map(|b| {
                if b {
                    "yes".to_string()
                } else {
                    "no".to_string()
                }
            });
            EvalStep {
                node_id: s.node_id,
                kind: s.kind,
                branch,
                result: s.branch,
            }
        })
        .collect();

    EvalResponse {
        matched_node_id,
        traversed_node_ids,
        traversed_edge_ids,
        steps,
        journey,
        summary,
    }
}

/// Compute the Transformation Journey (spec §5): start from the input body and,
/// at each expression node on the matched path, apply its action and snapshot the
/// resulting body. Decisions snapshot the unchanged running body. The start node
/// (synthetic) and the trailing end node bookend the journey. `body_after` is a
/// JSON value for JSON features, a string for HTML.
fn build_journey(
    state: &AppState,
    req: &EvalRequest,
    trace: &EvalTrace,
) -> (Vec<JourneyEntry>, Vec<NodeTiming>) {
    let canvas = &req.canvas;
    let is_json = req
        .context
        .content_kind
        .as_deref()
        .map(|k| k.eq_ignore_ascii_case("json"))
        .unwrap_or_else(|| req.context.response_json.is_some());

    // Running JSON body (only meaningful for JSON features).
    let mut json_body: Value = req
        .context
        .response_json
        .clone()
        .or_else(|| {
            req.context
                .response_body
                .as_deref()
                .and_then(|b| serde_json::from_str(b).ok())
        })
        .unwrap_or(Value::Null);
    // Running HTML body (only meaningful for HTML features).
    let mut html_body: String = req.context.response_body.clone().unwrap_or_default();

    // Action lookup: expression node_id -> applied action (in trace order).
    let action_for: HashMap<&str, &Value> = trace
        .actions
        .iter()
        .map(|a| (a.node_id.as_str(), &a.action))
        .collect();
    // Label lookup: canvas node id -> manifest/kind label.
    let label_for = |node_id: &str, kind: &str| -> String { label(canvas, state, node_id, kind) };

    let mut journey: Vec<JourneyEntry> = Vec::new();
    // Per-node expression apply timings, in trace order (spec §5 summary).
    let mut timings: Vec<NodeTiming> = Vec::new();
    let mut index = 0usize;

    // 1. Synthetic START entry (if the canvas has one), body unchanged.
    if let Some(start) = canvas.nodes.iter().find(|n| n.is_start()) {
        journey.push(JourneyEntry {
            index,
            node_id: start.id().to_string(),
            kind: "start".to_string(),
            label: "Start".to_string(),
            branch: None,
            body_after: snapshot(is_json, &json_body, &html_body),
            time_ms: features_matched::fmt_ms(0.0),
        });
        index += 1;
    }

    // 2. Each traversed node, in order. Expression nodes mutate the body (timed).
    for step in &trace.steps {
        // Expression steps carry their apply time; everything else is "0.00".
        let mut step_time_ms = 0.0;
        if step.kind == "expression" {
            if let Some(action) = action_for.get(step.node_id.as_str()) {
                let t_node = std::time::Instant::now();
                if is_json {
                    json_apply::apply_action_json(&mut json_body, action, &[]);
                } else {
                    let (next, _) =
                        json_apply::apply_action_html(html_body, action, &[], &state.sanitizer);
                    html_body = next;
                }
                step_time_ms = t_node.elapsed().as_secs_f64() * 1000.0;
                timings.push(NodeTiming {
                    node_id: step.node_id.clone(),
                    label: label_for(&step.node_id, &step.kind),
                    custom_label: expression_custom_label(canvas, &step.node_id),
                    time_ms: step_time_ms,
                });
            }
        }
        journey.push(JourneyEntry {
            index,
            node_id: step.node_id.clone(),
            kind: step.kind.clone(),
            label: label_for(&step.node_id, &step.kind),
            branch: step.branch,
            body_after: snapshot(is_json, &json_body, &html_body),
            time_ms: features_matched::fmt_ms(step_time_ms),
        });
        index += 1;
    }

    // 3. Trailing END entry: the matched path's terminal end node (the end node
    //    targeted by the last expression's outgoing edge), body unchanged.
    if let Some(end_id) = terminal_end(canvas, trace) {
        journey.push(JourneyEntry {
            index,
            node_id: end_id.clone(),
            kind: "end".to_string(),
            label: "End".to_string(),
            branch: None,
            body_after: snapshot(is_json, &json_body, &html_body),
            time_ms: features_matched::fmt_ms(0.0),
        });
    }

    (journey, timings)
}

/// Snapshot the running body as a journey `body_after` value (JSON or string).
fn snapshot(is_json: bool, json_body: &Value, html_body: &str) -> Value {
    if is_json {
        json_body.clone()
    } else {
        Value::String(html_body.to_string())
    }
}

/// Find the `end` node reached at the tail of the matched path: the target of the
/// last traversed node's outgoing edge that points at an `end` node.
fn terminal_end(canvas: &CanvasGraph, trace: &EvalTrace) -> Option<String> {
    let last = trace.steps.last()?;
    // Determine the branch the last node took (decisions branch; expression/start
    // have a single "yes"-by-convention outgoing edge).
    for edge in &canvas.edges {
        if edge.source_node_id != last.node_id {
            continue;
        }
        if let Some(target) = canvas.nodes.iter().find(|n| n.id() == edge.target_node_id) {
            if target.is_end() {
                // For a decision, respect the taken branch.
                if last.kind == "decision" {
                    let want = matches!(last.branch, Some(true));
                    let is_yes = matches!(edge.branch, crate::domain::graph::Branch::Yes);
                    if want != is_yes {
                        continue;
                    }
                }
                return Some(target.id().to_string());
            }
        }
    }
    None
}

/// Resolve a node's display label. Decisions/expressions use the manifest label
/// for their processor/action kind; start/end use a fixed label.
fn label(canvas: &CanvasGraph, _state: &AppState, node_id: &str, kind: &str) -> String {
    match kind {
        "start" => "Start".to_string(),
        "end" => "End".to_string(),
        _ => {
            let node = canvas.nodes.iter().find(|n| n.id() == node_id);
            match node {
                Some(Node::Decision { processor, .. }) => processor.kind.clone(),
                Some(Node::Expression { action, .. }) => action.kind.clone(),
                _ => kind.to_string(),
            }
        }
    }
}

/// The `custom_label` (spec v2.3) of an Expression node, if any. `None` for
/// non-expression nodes or when unset; carried through to `custom_expression_label`.
fn expression_custom_label(canvas: &CanvasGraph, node_id: &str) -> Option<String> {
    match canvas.nodes.iter().find(|n| n.id() == node_id) {
        Some(Node::Expression { custom_label, .. }) => custom_label.clone(),
        _ => None,
    }
}

/// Given an ordered list of traversed canvas node ids, find the canvas edges
/// that connect consecutive pairs.
fn derive_edge_ids(
    canvas: &CanvasGraph,
    node_ids: &[String],
    steps: &[crate::domain::evaluator::TraceStep],
) -> Vec<String> {
    // Build a map: (source_node_id, branch) -> edge id for fast lookup.
    let mut edge_map: HashMap<(&str, &str), &str> = HashMap::new();
    for edge in &canvas.edges {
        let branch_str = match edge.branch {
            crate::domain::graph::Branch::Yes => "yes",
            crate::domain::graph::Branch::No => "no",
        };
        edge_map.insert((edge.source_node_id.as_str(), branch_str), edge.id.as_str());
    }

    // Build a map from node_id -> branch taken (from trace steps).
    let branch_map: HashMap<&str, Option<bool>> = steps
        .iter()
        .map(|s| (s.node_id.as_str(), s.branch))
        .collect();

    let mut edges = Vec::new();
    for window in node_ids.windows(2) {
        let src = &window[0];
        let dst = &window[1];

        // Decisions branch yes/no; expression sources have a single outgoing edge
        // (wire value "yes" by convention).
        let branch_taken = branch_map.get(src.as_str()).and_then(|b| *b);
        let branch_str = match branch_taken {
            Some(true) => "yes",
            Some(false) => "no",
            None => "yes",
        };

        if let Some(edge_id) = edge_map.get(&(src.as_str(), branch_str)) {
            let target_matches = canvas
                .edges
                .iter()
                .any(|e| e.id == *edge_id && e.target_node_id == *dst);
            if target_matches {
                edges.push(edge_id.to_string());
            }
        }
    }
    edges
}
