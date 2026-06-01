//! HTTP coverage for `GET /api/v1/node-types` (BACKEND CONTRACT §6/§7).
//!
//! Drives the real router via `tower::ServiceExt::oneshot` (no network bind) and
//! asserts the manifest is served verbatim with snake_case keys and the
//! expected kinds.

mod common;

use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use http_body_util::BodyExt;
use rre_backend::build_app;
use serde_json::Value;
use tower::ServiceExt;

#[tokio::test]
async fn node_types_endpoint_serves_manifest_verbatim() {
    let db = common::setup().await;
    let app = build_app(db.state.clone());

    let res = app
        .oneshot(
            Request::builder()
                .uri("/api/v1/node-types")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(res.status(), StatusCode::OK);
    let bytes = res.into_body().collect().await.unwrap().to_bytes();
    let body: Value = serde_json::from_slice(&bytes).unwrap();

    // Top-level snake_case keys.
    assert!(body["categories"].is_array());
    assert!(body["node_types"].is_array());

    // The ported kinds plus json_expression, in palette order.
    let kinds: Vec<&str> = body["node_types"]
        .as_array()
        .unwrap()
        .iter()
        .map(|n| n["kind"].as_str().unwrap())
        .collect();
    assert_eq!(
        kinds,
        ["meta_tags", "device_type", "article_url", "json_expression"]
    );

    // coming_soon flag is present on a disabled category.
    let user = body["categories"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["id"] == "user")
        .unwrap();
    assert_eq!(user["coming_soon"], true);

    // The served body equals the AppState's raw manifest JSON (verbatim).
    assert_eq!(&body, &*db.state.node_manifest_json);
}
