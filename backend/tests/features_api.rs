//! HTTP-level integration tests for the feature endpoints. Drives the `Router`
//! with `tower::ServiceExt::oneshot` (no network bind), real Postgres backing.

mod common;

use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use http_body_util::BodyExt;
use rre_backend::build_app;
use serde_json::{json, Value};
use tower::ServiceExt;

/// Helper: send a JSON request and return `(status, body json)`.
async fn send(
    state: rre_backend::state::AppState,
    method: &str,
    uri: &str,
    body: Option<Value>,
) -> (StatusCode, Value) {
    let app = build_app(state);
    let req_body = match body {
        Some(v) => Body::from(serde_json::to_vec(&v).unwrap()),
        None => Body::empty(),
    };
    let res = app
        .oneshot(
            Request::builder()
                .method(method)
                .uri(uri)
                .header("content-type", "application/json")
                .body(req_body)
                .unwrap(),
        )
        .await
        .unwrap();

    let status = res.status();
    let bytes = res.into_body().collect().await.unwrap().to_bytes();
    let json = if bytes.is_empty() {
        Value::Null
    } else {
        serde_json::from_slice(&bytes).unwrap_or(Value::Null)
    };
    (status, json)
}

#[tokio::test]
async fn create_feature_returns_201() {
    let db = common::setup().await;
    let (status, body) = send(
        db.state.clone(),
        "POST",
        "/api/v1/features",
        Some(json!({ "id": "dn-article", "name": "DN Article", "type": "html" })),
    )
    .await;

    assert_eq!(status, StatusCode::CREATED);
    assert_eq!(body["id"], "dn-article");
    assert_eq!(body["type"], "html");
    assert!(body["live_version_id"].is_null());
}

#[tokio::test]
async fn create_duplicate_slug_returns_409() {
    let db = common::setup().await;
    let payload = json!({ "id": "dn-article", "name": "DN Article", "type": "html" });

    let (first, _) = send(
        db.state.clone(),
        "POST",
        "/api/v1/features",
        Some(payload.clone()),
    )
    .await;
    assert_eq!(first, StatusCode::CREATED);

    let (second, body) = send(db.state.clone(), "POST", "/api/v1/features", Some(payload)).await;
    assert_eq!(second, StatusCode::CONFLICT);
    assert_eq!(body["error"]["code"], "SLUG_CONFLICT");
}

#[tokio::test]
async fn create_invalid_slug_returns_422() {
    let db = common::setup().await;
    let (status, body) = send(
        db.state.clone(),
        "POST",
        "/api/v1/features",
        Some(json!({ "id": "Bad Slug", "name": "X", "type": "html" })),
    )
    .await;

    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(body["error"]["code"], "VALIDATION_ERROR");
    assert!(body["error"]["details"].is_array());
}

#[tokio::test]
async fn get_missing_feature_returns_404() {
    let db = common::setup().await;
    let (status, body) = send(db.state.clone(), "GET", "/api/v1/features/nope", None).await;

    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(body["error"]["code"], "FEATURE_NOT_FOUND");
}

#[tokio::test]
async fn list_features_returns_envelope() {
    let db = common::setup().await;
    send(
        db.state.clone(),
        "POST",
        "/api/v1/features",
        Some(json!({ "id": "dn-article", "name": "DN Article", "type": "html" })),
    )
    .await;

    let (status, body) = send(
        db.state.clone(),
        "GET",
        "/api/v1/features?page=1&page_size=20",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["page"], 1);
    assert_eq!(body["page_size"], 20);
    assert_eq!(body["total"], 1);
    assert_eq!(body["items"].as_array().unwrap().len(), 1);
}

#[tokio::test]
async fn patch_feature_updates_name() {
    let db = common::setup().await;
    send(
        db.state.clone(),
        "POST",
        "/api/v1/features",
        Some(json!({ "id": "dn-article", "name": "Old", "type": "html" })),
    )
    .await;

    let (status, body) = send(
        db.state.clone(),
        "PATCH",
        "/api/v1/features/dn-article",
        Some(json!({ "name": "New" })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["name"], "New");
}

#[tokio::test]
async fn delete_feature_returns_204() {
    let db = common::setup().await;
    send(
        db.state.clone(),
        "POST",
        "/api/v1/features",
        Some(json!({ "id": "dn-article", "name": "Doomed", "type": "html" })),
    )
    .await;

    let (status, _) = send(
        db.state.clone(),
        "DELETE",
        "/api/v1/features/dn-article",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);

    let (after, _) = send(db.state.clone(), "GET", "/api/v1/features/dn-article", None).await;
    assert_eq!(after, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn delete_missing_feature_returns_404() {
    let db = common::setup().await;
    let (status, body) = send(db.state.clone(), "DELETE", "/api/v1/features/nope", None).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(body["error"]["code"], "FEATURE_NOT_FOUND");
}
