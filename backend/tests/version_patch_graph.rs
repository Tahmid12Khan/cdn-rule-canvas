//! End-to-end coverage for `PATCH /api/v1/features/{fid}/versions/{vnum}` with a
//! `rule_graph` body (BACKEND CONTRACT §6/§7).
//!
//! Drives the real router over a testcontainers Postgres via
//! `tower::ServiceExt::oneshot` (no network bind). Exercises the validation
//! hook wired into the version-update path: a graph that references the version's
//! builtin outcome must succeed (200); a graph with a dangling edge / unknown
//! outcome must fail with 422 `VALIDATION_ERROR` and a `details` list.

mod common;

use axum::{
    body::Body,
    http::{Request, StatusCode},
    Router,
};
use http_body_util::BodyExt;
use rre_backend::{build_app, state::AppState};
use serde_json::{json, Value};
use tower::ServiceExt;

/// `oneshot` consumes the router, so build a fresh one per request.
fn app(state: &AppState) -> Router {
    build_app(state.clone())
}

async fn send(state: &AppState, req: Request<Body>) -> (StatusCode, Value) {
    let res = app(state).oneshot(req).await.unwrap();
    let status = res.status();
    let bytes = res.into_body().collect().await.unwrap().to_bytes();
    let body: Value = if bytes.is_empty() {
        Value::Null
    } else {
        serde_json::from_slice(&bytes).unwrap_or(Value::Null)
    };
    (status, body)
}

fn post(uri: &str, body: Value) -> Request<Body> {
    Request::builder()
        .method("POST")
        .uri(uri)
        .header("content-type", "application/json")
        .body(Body::from(body.to_string()))
        .unwrap()
}

fn patch(uri: &str, body: Value) -> Request<Body> {
    Request::builder()
        .method("PATCH")
        .uri(uri)
        .header("content-type", "application/json")
        .body(Body::from(body.to_string()))
        .unwrap()
}

fn get(uri: &str) -> Request<Body> {
    Request::builder()
        .method("GET")
        .uri(uri)
        .body(Body::empty())
        .unwrap()
}

/// Create a feature + DRAFT version, returning `(version_number, version_id,
/// builtin_outcome_id)`.
async fn seed_feature_version(state: &AppState, slug: &str) -> (i64, String, String) {
    let (status, _) = send(
        state,
        post(
            "/api/v1/features",
            json!({ "id": slug, "name": "Test Feature", "type": "html" }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "feature create should 201");

    let (status, version) = send(
        state,
        post(
            &format!("/api/v1/features/{slug}/versions"),
            json!({ "description": "initial" }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "version create should 201");

    let vnum = version["version_number"].as_i64().expect("version_number");
    let vid = version["id"].as_str().expect("version id").to_string();

    // The version create seeds a builtin ShowContent outcome.
    let (status, outcomes) = send(state, get(&format!("/api/v1/versions/{vid}/outcomes"))).await;
    assert_eq!(status, StatusCode::OK, "list outcomes should 200");
    let outcome_id = outcomes
        .as_array()
        .and_then(|a| a.first())
        .and_then(|o| o["id"].as_str())
        .expect("at least one seeded outcome")
        .to_string();

    (vnum, vid, outcome_id)
}

#[tokio::test]
async fn patch_valid_rule_graph_succeeds() {
    let db = common::setup().await;
    let (vnum, _vid, outcome_id) = seed_feature_version(&db.state, "patch-valid").await;

    let rule_graph = json!({
        "canvas": {
            "root_node_id": "start",
            "nodes": [
                { "kind": "start", "id": "start", "position": { "x": -120.0, "y": 0.0 } },
                { "kind": "decision", "id": "d1",
                  "processor": { "type": "device_type", "operator": "equals", "value": "mobile" },
                  "position": { "x": 0.0, "y": 0.0 } },
                { "kind": "expression", "id": "a1",
                  "action": { "type": "apply_outcome", "outcome_id": outcome_id },
                  "position": { "x": 200.0, "y": 0.0 } },
                { "kind": "end", "id": "end", "position": { "x": 400.0, "y": 0.0 } }
            ],
            "edges": [
                { "id": "e0", "source_node_id": "start", "target_node_id": "d1", "branch": "yes" },
                { "id": "e1", "source_node_id": "d1", "target_node_id": "a1", "branch": "yes" },
                { "id": "e2", "source_node_id": "d1", "target_node_id": "end", "branch": "no" },
                { "id": "e3", "source_node_id": "a1", "target_node_id": "end", "branch": "yes" }
            ]
        },
    });

    let (status, body) = send(
        &db.state,
        patch(
            &format!("/api/v1/features/patch-valid/versions/{vnum}"),
            json!({ "rule_graph": rule_graph }),
        ),
    )
    .await;

    assert_eq!(
        status,
        StatusCode::OK,
        "valid graph PATCH should 200: {body}"
    );
    // The persisted graph should echo the start node back.
    assert_eq!(
        body["rule_graph"]["canvas"]["root_node_id"], "start",
        "rule_graph should round-trip"
    );
}

#[tokio::test]
async fn patch_dangling_edge_rejected() {
    let db = common::setup().await;
    let (vnum, _vid, outcome_id) = seed_feature_version(&db.state, "patch-dangling").await;

    let rule_graph = json!({
        "canvas": {
            "root_node_id": null,
            "nodes": [
                { "kind": "start", "id": "start", "position": { "x": -120.0, "y": 0.0 } },
                { "kind": "decision", "id": "d1",
                  "processor": { "type": "device_type", "operator": "equals", "value": "mobile" },
                  "position": { "x": 0.0, "y": 0.0 } },
                { "kind": "expression", "id": "a1",
                  "action": { "type": "apply_outcome", "outcome_id": outcome_id },
                  "position": { "x": 200.0, "y": 0.0 } },
                { "kind": "end", "id": "end", "position": { "x": 400.0, "y": 0.0 } }
            ],
            "edges": [
                { "id": "e0", "source_node_id": "start", "target_node_id": "d1", "branch": "yes" },
                { "id": "e1", "source_node_id": "d1", "target_node_id": "ghost", "branch": "yes" }
            ]
        },
    });

    let (status, body) = send(
        &db.state,
        patch(
            &format!("/api/v1/features/patch-dangling/versions/{vnum}"),
            json!({ "rule_graph": rule_graph }),
        ),
    )
    .await;

    assert_eq!(
        status,
        StatusCode::UNPROCESSABLE_ENTITY,
        "dangling edge should 422: {body}"
    );
    assert_eq!(body["error"]["code"], "VALIDATION_ERROR");
    let details = body["error"]["details"]
        .as_array()
        .expect("details array present on 422");
    assert!(
        details
            .iter()
            .any(|d| d["rule_id"] == "edge_endpoint_exists"),
        "expected edge_endpoint_exists detail: {body}"
    );
}

#[tokio::test]
async fn patch_unknown_outcome_rejected() {
    let db = common::setup().await;
    let (vnum, _vid, _outcome_id) = seed_feature_version(&db.state, "patch-unknown-outcome").await;

    // An apply_outcome expression referencing a UUID that is not an outcome of
    // this version.
    let rule_graph = json!({
        "canvas": {
            "root_node_id": "start",
            "nodes": [
                { "kind": "start", "id": "start", "position": { "x": -120.0, "y": 0.0 } },
                { "kind": "expression", "id": "a1",
                  "action": { "type": "apply_outcome", "outcome_id": "99999999-9999-9999-9999-999999999999" },
                  "position": { "x": 0.0, "y": 0.0 } },
                { "kind": "end", "id": "end", "position": { "x": 200.0, "y": 0.0 } }
            ],
            "edges": [
                { "id": "e0", "source_node_id": "start", "target_node_id": "a1", "branch": "yes" },
                { "id": "e1", "source_node_id": "a1", "target_node_id": "end", "branch": "yes" }
            ]
        },
    });

    let (status, body) = send(
        &db.state,
        patch(
            &format!("/api/v1/features/patch-unknown-outcome/versions/{vnum}"),
            json!({ "rule_graph": rule_graph }),
        ),
    )
    .await;

    assert_eq!(
        status,
        StatusCode::UNPROCESSABLE_ENTITY,
        "unknown outcome ref should 422: {body}"
    );
    assert_eq!(body["error"]["code"], "VALIDATION_ERROR");
    let details = body["error"]["details"]
        .as_array()
        .expect("details array present on 422");
    assert!(
        details
            .iter()
            .any(|d| d["rule_id"] == "apply_outcome_ref_exists"),
        "expected apply_outcome_ref_exists detail: {body}"
    );
}
