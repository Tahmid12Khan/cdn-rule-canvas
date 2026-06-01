//! `POST /__rre/eval` — test-eval endpoint.
//!
//! Accepts a canvas + context from the request body (no upstream fetch), runs
//! `GraphEvaluator::evaluate_with_trace`, and returns the matched outcome id
//! plus the ordered traversal (node ids, edge ids, per-step detail).
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

use crate::domain::context::{DeviceType, EvaluationContext};
use crate::domain::evaluator::{EvalTrace, GraphEvaluator};
use crate::domain::graph::CanvasGraph;
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
    pub matched_outcome_id: Option<String>,
    pub matched_node_id: Option<String>,
    pub traversed_node_ids: Vec<String>,
    pub traversed_edge_ids: Vec<String>,
    pub steps: Vec<EvalStep>,
}

#[derive(Serialize)]
pub struct EvalStep {
    pub node_id: String,
    pub kind: String,
    pub branch: Option<String>,
    pub result: Option<bool>,
}

// ---------------------------------------------------------------------------
// Handler
// ---------------------------------------------------------------------------

pub async fn eval_handler(
    State(state): State<AppState>,
    Json(req): Json<EvalRequest>,
) -> impl IntoResponse {
    // Validate: canvas must have at least one node for a meaningful eval.
    // Empty canvases return a well-formed 200 (dead-end).

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
    let trace_result = evaluator
        .evaluate_with_trace(&req.canvas, content, Arc::new(ctx))
        .await;

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
    let response = build_response(&req.canvas, trace_result);
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

fn build_response(canvas: &CanvasGraph, trace: EvalTrace) -> EvalResponse {
    // trace.steps is ordered by zen's `order` field: each entry has the canvas
    // node id and the branch output.
    let traversed_node_ids: Vec<String> = trace.steps.iter().map(|s| s.node_id.clone()).collect();

    // Derive traversed edge ids from consecutive (source, target) pairs in the traversal.
    let traversed_edge_ids = derive_edge_ids(canvas, &traversed_node_ids, &trace.steps);

    // The terminal node is the last step that is an outcome node.
    let terminal = trace.steps.iter().rev().find(|s| s.kind == "outcome");

    let matched_node_id = terminal.map(|s| s.node_id.clone());
    let matched_outcome_id = trace.matched_outcome_id.map(|u| u.to_string());

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
        matched_outcome_id,
        matched_node_id,
        traversed_node_ids,
        traversed_edge_ids,
        steps,
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

        // Find the edge from src that points to dst with the branch taken.
        // Look at src's branch result first.
        let branch_taken = branch_map.get(src.as_str()).and_then(|b| *b);
        let branch_str = match branch_taken {
            Some(true) => "yes",
            Some(false) => "no",
            None => continue,
        };

        // Verify the edge actually connects src->dst (it should, but be safe).
        if let Some(edge_id) = edge_map.get(&(src.as_str(), branch_str)) {
            // Verify target matches.
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
