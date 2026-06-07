//! HTTP-level tests for the version routes, driving the `Router` via
//! `tower::ServiceExt::oneshot` (no network bind).

mod common;

use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use http_body_util::BodyExt;
use rre_backend::build_app;
use serde_json::{json, Value};
use sqlx::PgPool;
use tower::ServiceExt;

async fn seed_feature(pool: &PgPool, id: &str) {
    sqlx::query(
        "INSERT INTO rre.features (id, name, type, execution_order) \
         VALUES ($1, $2, 'html', \
                 (SELECT COALESCE(MAX(execution_order), 0) + 1 FROM rre.features WHERE type = 'html'))",
    )
        .bind(id)
        .bind(format!("Feature {id}"))
        .execute(pool)
        .await
        .expect("seed feature");
}

fn post(uri: &str, body: Value) -> Request<Body> {
    Request::builder()
        .method("POST")
        .uri(uri)
        .header("content-type", "application/json")
        .body(Body::from(body.to_string()))
        .unwrap()
}

async fn body_json(res: axum::response::Response) -> Value {
    let bytes = res.into_body().collect().await.unwrap().to_bytes();
    serde_json::from_slice(&bytes).unwrap()
}

#[tokio::test]
async fn create_version_returns_201() {
    let db = common::setup().await;
    seed_feature(&db.state.pool, "demo-article").await;
    let app = build_app(db.state.clone());

    let res = app
        .oneshot(post(
            "/api/v1/features/demo-article/versions",
            json!({ "description": "hello" }),
        ))
        .await
        .unwrap();

    assert_eq!(res.status(), StatusCode::CREATED);
    let body = body_json(res).await;
    assert_eq!(body["version_number"], 1);
    assert_eq!(body["status"], "draft");
    assert_eq!(body["description"], "hello");
}

#[tokio::test]
async fn create_on_missing_feature_returns_404() {
    let db = common::setup().await;
    let app = build_app(db.state.clone());

    let res = app
        .oneshot(post("/api/v1/features/does-not-exist/versions", json!({})))
        .await
        .unwrap();

    assert_eq!(res.status(), StatusCode::NOT_FOUND);
    let body = body_json(res).await;
    assert_eq!(body["error"]["code"], "FEATURE_NOT_FOUND");
}

#[tokio::test]
async fn list_versions_returns_paginated_envelope() {
    let db = common::setup().await;
    seed_feature(&db.state.pool, "demo-article").await;
    let app = build_app(db.state.clone());

    // Create two versions.
    for _ in 0..2 {
        app.clone()
            .oneshot(post("/api/v1/features/demo-article/versions", json!({})))
            .await
            .unwrap();
    }

    let res = app
        .oneshot(
            Request::builder()
                .uri("/api/v1/features/demo-article/versions?page=1&page_size=20")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(res.status(), StatusCode::OK);
    let body = body_json(res).await;
    assert_eq!(body["total"], 2);
    assert_eq!(body["items"].as_array().unwrap().len(), 2);
}

#[tokio::test]
async fn get_version_returns_full_read() {
    let db = common::setup().await;
    seed_feature(&db.state.pool, "demo-article").await;
    let app = build_app(db.state.clone());

    app.clone()
        .oneshot(post("/api/v1/features/demo-article/versions", json!({})))
        .await
        .unwrap();

    let res = app
        .oneshot(
            Request::builder()
                .uri("/api/v1/features/demo-article/versions/1")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(res.status(), StatusCode::OK);
    let body = body_json(res).await;
    assert_eq!(body["version_number"], 1);
    assert!(body["rule_graph"]["anonymous"].is_object());
}

#[tokio::test]
async fn get_unknown_version_returns_404() {
    let db = common::setup().await;
    seed_feature(&db.state.pool, "demo-article").await;
    let app = build_app(db.state.clone());

    let res = app
        .oneshot(
            Request::builder()
                .uri("/api/v1/features/demo-article/versions/42")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(res.status(), StatusCode::NOT_FOUND);
    let body = body_json(res).await;
    assert_eq!(body["error"]["code"], "VERSION_NOT_FOUND");
}

#[tokio::test]
async fn publish_then_unpublish_via_http() {
    let db = common::setup().await;
    seed_feature(&db.state.pool, "demo-article").await;
    let app = build_app(db.state.clone());

    app.clone()
        .oneshot(post("/api/v1/features/demo-article/versions", json!({})))
        .await
        .unwrap();

    let res = app
        .clone()
        .oneshot(post(
            "/api/v1/features/demo-article/versions/1/publish",
            json!({ "environment": "live" }),
        ))
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let body = body_json(res).await;
    assert_eq!(body["status"], "live");

    let res = app
        .oneshot(post(
            "/api/v1/features/demo-article/versions/1/unpublish",
            json!({ "environment": "live" }),
        ))
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let body = body_json(res).await;
    assert_eq!(body["status"], "prev");
}

#[tokio::test]
async fn delete_live_version_returns_409() {
    let db = common::setup().await;
    seed_feature(&db.state.pool, "demo-article").await;
    let app = build_app(db.state.clone());

    app.clone()
        .oneshot(post("/api/v1/features/demo-article/versions", json!({})))
        .await
        .unwrap();
    app.clone()
        .oneshot(post(
            "/api/v1/features/demo-article/versions/1/publish",
            json!({ "environment": "live" }),
        ))
        .await
        .unwrap();

    let res = app
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri("/api/v1/features/demo-article/versions/1")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(res.status(), StatusCode::CONFLICT);
    let body = body_json(res).await;
    assert_eq!(body["error"]["code"], "INVALID_STATUS_TRANSITION");
}

#[tokio::test]
async fn delete_draft_version_returns_204() {
    let db = common::setup().await;
    seed_feature(&db.state.pool, "demo-article").await;
    let app = build_app(db.state.clone());

    app.clone()
        .oneshot(post("/api/v1/features/demo-article/versions", json!({})))
        .await
        .unwrap();

    let res = app
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri("/api/v1/features/demo-article/versions/1")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(res.status(), StatusCode::NO_CONTENT);
}
