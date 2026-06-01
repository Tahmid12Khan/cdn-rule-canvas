//! HTTP-level tests for the outcome + nested-component routes, driving the
//! `Router` via `tower::ServiceExt::oneshot` against a real Postgres.

mod common;

use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use http_body_util::BodyExt;
use rre_backend::{build_app, state::AppState};
use serde_json::{json, Value};
use sqlx::PgPool;
use tower::ServiceExt;
use uuid::Uuid;

/// Insert a feature + a DRAFT version directly; return the version id.
async fn seed_draft_version(pool: &PgPool) -> Uuid {
    let feature_id = format!("feat-{}", &Uuid::new_v4().to_string()[..8]);
    sqlx::query("INSERT INTO rre.features (id, name, type) VALUES ($1, 'F', 'html')")
        .bind(&feature_id)
        .execute(pool)
        .await
        .unwrap();
    let version_id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO rre.versions (id, feature_id, version_number, status) \
         VALUES ($1, $2, 1, 'draft')",
    )
    .bind(version_id)
    .bind(&feature_id)
    .execute(pool)
    .await
    .unwrap();
    version_id
}

async fn send(
    state: &AppState,
    method: &str,
    uri: &str,
    body: Option<Value>,
) -> (StatusCode, Value) {
    let app = build_app(state.clone());
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
    let value = if bytes.is_empty() {
        Value::Null
    } else {
        serde_json::from_slice(&bytes).unwrap_or(Value::Null)
    };
    (status, value)
}

#[tokio::test]
async fn create_list_get_outcome_via_http() {
    let db = common::setup().await;
    let vid = seed_draft_version(&db.state.pool).await;

    // Create.
    let (status, created) = send(
        &db.state,
        "POST",
        &format!("/api/v1/versions/{vid}/outcomes"),
        Some(json!({ "title": "Paywall", "description": "hard" })),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    assert_eq!(created["title"], "Paywall");
    let oid = created["id"].as_str().unwrap();

    // List.
    let (status, list) = send(
        &db.state,
        "GET",
        &format!("/api/v1/versions/{vid}/outcomes"),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(list.as_array().unwrap().len(), 1);

    // Get.
    let (status, got) = send(&db.state, "GET", &format!("/api/v1/outcomes/{oid}"), None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(got["id"], oid);
    assert!(got["components"].as_array().unwrap().is_empty());
}

#[tokio::test]
async fn create_outcome_validation_error_is_422() {
    let db = common::setup().await;
    let vid = seed_draft_version(&db.state.pool).await;

    let (status, body) = send(
        &db.state,
        "POST",
        &format!("/api/v1/versions/{vid}/outcomes"),
        Some(json!({ "title": "" })), // too short
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(body["error"]["code"], "VALIDATION_ERROR");
}

#[tokio::test]
async fn get_missing_outcome_is_404() {
    let db = common::setup().await;
    let oid = Uuid::new_v4();
    let (status, body) = send(&db.state, "GET", &format!("/api/v1/outcomes/{oid}"), None).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(body["error"]["code"], "OUTCOME_NOT_FOUND");
}

#[tokio::test]
async fn add_component_and_reorder_via_http() {
    let db = common::setup().await;
    let vid = seed_draft_version(&db.state.pool).await;

    let (_, outcome) = send(
        &db.state,
        "POST",
        &format!("/api/v1/versions/{vid}/outcomes"),
        Some(json!({ "title": "O" })),
    )
    .await;
    let oid = outcome["id"].as_str().unwrap().to_string();

    let make_component = |slug: &str, order: i64| {
        json!({
            "slug": slug,
            "type": "html_injection",
            "config": {
                "type": "html_injection",
                "target_selector": ".x",
                "placement_mode": "append",
                "html_body": "<p>p</p>"
            },
            "placement": "inline",
            "order_index": order
        })
    };

    let (status, c1) = send(
        &db.state,
        "POST",
        &format!("/api/v1/outcomes/{oid}/components"),
        Some(make_component("a", 0)),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    let c1_id = c1["id"].as_str().unwrap().to_string();
    assert_eq!(c1["config"]["type"], "html_injection");

    let (_, c2) = send(
        &db.state,
        "POST",
        &format!("/api/v1/outcomes/{oid}/components"),
        Some(make_component("b", 1)),
    )
    .await;
    let c2_id = c2["id"].as_str().unwrap().to_string();

    // Reorder: c2 first.
    let (status, reordered) = send(
        &db.state,
        "POST",
        &format!("/api/v1/outcomes/{oid}/reorder"),
        Some(json!([
            { "id": c2_id, "order_index": 0 },
            { "id": c1_id, "order_index": 1 }
        ])),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let arr = reordered.as_array().unwrap();
    assert_eq!(arr[0]["id"], c2_id);
    assert_eq!(arr[1]["id"], c1_id);
}

#[tokio::test]
async fn delete_outcome_returns_204() {
    let db = common::setup().await;
    let vid = seed_draft_version(&db.state.pool).await;
    let (_, outcome) = send(
        &db.state,
        "POST",
        &format!("/api/v1/versions/{vid}/outcomes"),
        Some(json!({ "title": "Bye" })),
    )
    .await;
    let oid = outcome["id"].as_str().unwrap();

    let (status, _) = send(
        &db.state,
        "DELETE",
        &format!("/api/v1/outcomes/{oid}"),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
}

#[tokio::test]
async fn clone_outcome_via_http_returns_201() {
    let db = common::setup().await;
    let vid = seed_draft_version(&db.state.pool).await;
    let (_, outcome) = send(
        &db.state,
        "POST",
        &format!("/api/v1/versions/{vid}/outcomes"),
        Some(json!({ "title": "Src" })),
    )
    .await;
    let oid = outcome["id"].as_str().unwrap();

    let (status, cloned) = send(
        &db.state,
        "POST",
        &format!("/api/v1/outcomes/{oid}/clone"),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    assert_eq!(cloned["title"], "Src (copy)");
    assert_eq!(cloned["is_builtin"], false);
}

#[tokio::test]
async fn add_component_type_mismatch_is_422() {
    let db = common::setup().await;
    let vid = seed_draft_version(&db.state.pool).await;
    let (_, outcome) = send(
        &db.state,
        "POST",
        &format!("/api/v1/versions/{vid}/outcomes"),
        Some(json!({ "title": "O" })),
    )
    .await;
    let oid = outcome["id"].as_str().unwrap();

    let (status, body) = send(
        &db.state,
        "POST",
        &format!("/api/v1/outcomes/{oid}/components"),
        Some(json!({
            "slug": "bad",
            "type": "content_truncation",
            "config": {
                "type": "html_injection",
                "target_selector": ".x",
                "placement_mode": "append",
                "html_body": "<p>p</p>"
            },
            "placement": "inline"
        })),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(body["error"]["code"], "VALIDATION_ERROR");
}
