//! HTTP-level integration tests for the test-preset endpoints. Drives the
//! `Router` with `tower::ServiceExt::oneshot` (no network bind), real Postgres
//! backing. Mirrors `sites_api.rs` (same harness, same `send` helper).

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

/// A valid `TestPresetCreate` body. Override fields per-test via the returned `Value`.
fn preset_payload(slug: &str, name: &str, kind: &str) -> Value {
    json!({
        "slug": slug,
        "name": name,
        "kind": kind,
        "payload": { "feature_type": "html", "device_type": "mobile" }
    })
}

#[tokio::test]
async fn create_preset_returns_201_with_read_body() {
    let db = common::setup().await;
    let (status, body) = send(
        db.state.clone(),
        "POST",
        "/api/v1/test-presets",
        Some(preset_payload("mobile-paywall", "Mobile paywall", "rule")),
    )
    .await;

    assert_eq!(status, StatusCode::CREATED);
    assert_eq!(body["slug"], "mobile-paywall");
    assert_eq!(body["name"], "Mobile paywall");
    assert_eq!(body["kind"], "rule");
    assert_eq!(body["payload"]["feature_type"], "html");
    assert_eq!(body["payload"]["device_type"], "mobile");
    assert!(body["created_at"].is_string());
    assert!(body["updated_at"].is_string());
}

#[tokio::test]
async fn create_duplicate_slug_returns_409() {
    let db = common::setup().await;
    let (first, _) = send(
        db.state.clone(),
        "POST",
        "/api/v1/test-presets",
        Some(preset_payload("dup-slug", "First", "rule")),
    )
    .await;
    assert_eq!(first, StatusCode::CREATED);

    // Same slug, different name → only the PK collides.
    let dup = preset_payload("dup-slug", "Second", "rule");
    let (second, body) = send(db.state.clone(), "POST", "/api/v1/test-presets", Some(dup)).await;
    assert_eq!(second, StatusCode::CONFLICT);
    assert_eq!(body["error"]["code"], "SLUG_CONFLICT");
}

#[tokio::test]
async fn create_duplicate_name_returns_409() {
    let db = common::setup().await;
    let (first, _) = send(
        db.state.clone(),
        "POST",
        "/api/v1/test-presets",
        Some(preset_payload("preset-one", "Shared Name", "rule")),
    )
    .await;
    assert_eq!(first, StatusCode::CREATED);

    // Different slug, same name → the name unique index collides.
    let dup = preset_payload("preset-two", "Shared Name", "rule");
    let (second, body) = send(db.state.clone(), "POST", "/api/v1/test-presets", Some(dup)).await;
    assert_eq!(second, StatusCode::CONFLICT);
    assert_eq!(body["error"]["code"], "SLUG_CONFLICT");
}

#[tokio::test]
async fn list_presets_returns_page_envelope_and_filters() {
    let db = common::setup().await;
    send(
        db.state.clone(),
        "POST",
        "/api/v1/test-presets",
        Some(preset_payload("alpha-rule", "Alpha rule preset", "rule")),
    )
    .await;
    send(
        db.state.clone(),
        "POST",
        "/api/v1/test-presets",
        Some(json!({
            "slug": "beta-url",
            "name": "Beta url preset",
            "kind": "url",
            "payload": { "url": "https://example.com" }
        })),
    )
    .await;

    // Full page envelope.
    let (status, body) = send(
        db.state.clone(),
        "GET",
        "/api/v1/test-presets?page=1&page_size=20",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["page"], 1);
    assert_eq!(body["page_size"], 20);
    assert_eq!(body["total"], 2);
    assert_eq!(body["items"].as_array().unwrap().len(), 2);

    // `q` matches a substring of the name, case-insensitive.
    let (qstatus, qbody) = send(db.state.clone(), "GET", "/api/v1/test-presets?q=alph", None).await;
    assert_eq!(qstatus, StatusCode::OK);
    assert_eq!(qbody["total"], 1);
    assert_eq!(qbody["items"][0]["slug"], "alpha-rule");

    // `kind` filter narrows to a single kind.
    let (kstatus, kbody) = send(
        db.state.clone(),
        "GET",
        "/api/v1/test-presets?kind=url",
        None,
    )
    .await;
    assert_eq!(kstatus, StatusCode::OK);
    assert_eq!(kbody["total"], 1);
    assert_eq!(kbody["items"][0]["slug"], "beta-url");
    assert_eq!(kbody["items"][0]["kind"], "url");
}

#[tokio::test]
async fn get_preset_returns_200() {
    let db = common::setup().await;
    send(
        db.state.clone(),
        "POST",
        "/api/v1/test-presets",
        Some(preset_payload("mobile-paywall", "Mobile paywall", "rule")),
    )
    .await;

    let (status, body) = send(
        db.state.clone(),
        "GET",
        "/api/v1/test-presets/mobile-paywall",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["slug"], "mobile-paywall");
    assert_eq!(body["name"], "Mobile paywall");
}

#[tokio::test]
async fn get_missing_preset_returns_404() {
    let db = common::setup().await;
    let (status, body) = send(db.state.clone(), "GET", "/api/v1/test-presets/nope", None).await;

    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(body["error"]["code"], "TEST_PRESET_NOT_FOUND");
}

#[tokio::test]
async fn patch_preset_name_only_returns_200() {
    let db = common::setup().await;
    send(
        db.state.clone(),
        "POST",
        "/api/v1/test-presets",
        Some(preset_payload("mobile-paywall", "Old Name", "rule")),
    )
    .await;

    let (status, body) = send(
        db.state.clone(),
        "PATCH",
        "/api/v1/test-presets/mobile-paywall",
        Some(json!({ "name": "New Name" })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["slug"], "mobile-paywall");
    assert_eq!(body["name"], "New Name");
    // Untouched payload preserved.
    assert_eq!(body["payload"]["feature_type"], "html");
}

#[tokio::test]
async fn patch_preset_payload_only_replaces_payload() {
    let db = common::setup().await;
    send(
        db.state.clone(),
        "POST",
        "/api/v1/test-presets",
        Some(preset_payload("mobile-paywall", "Keep", "rule")),
    )
    .await;

    let (status, body) = send(
        db.state.clone(),
        "PATCH",
        "/api/v1/test-presets/mobile-paywall",
        Some(json!({ "payload": { "path": "/article" } })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["name"], "Keep", "name unchanged");
    assert_eq!(body["payload"], json!({ "path": "/article" }));
}

#[tokio::test]
async fn patch_preset_slug_and_kind_are_immutable() {
    let db = common::setup().await;
    send(
        db.state.clone(),
        "POST",
        "/api/v1/test-presets",
        Some(preset_payload("mobile-paywall", "Demo", "rule")),
    )
    .await;

    // `slug`/`kind` in the PATCH body are not accepted fields (TestPresetUpdate
    // is `deny_unknown_fields`), so the request is rejected before any mutation.
    let (rejected, _) = send(
        db.state.clone(),
        "PATCH",
        "/api/v1/test-presets/mobile-paywall",
        Some(json!({ "slug": "renamed", "kind": "url" })),
    )
    .await;
    assert!(
        rejected.is_client_error(),
        "slug/kind in PATCH body must be rejected, got {rejected}"
    );

    // The original is unchanged; the kind is still `rule`.
    let (still, still_body) = send(
        db.state.clone(),
        "GET",
        "/api/v1/test-presets/mobile-paywall",
        None,
    )
    .await;
    assert_eq!(still, StatusCode::OK);
    assert_eq!(still_body["kind"], "rule");
}

#[tokio::test]
async fn delete_preset_returns_204_then_get_404() {
    let db = common::setup().await;
    send(
        db.state.clone(),
        "POST",
        "/api/v1/test-presets",
        Some(preset_payload("mobile-paywall", "Doomed", "rule")),
    )
    .await;

    let (status, _) = send(
        db.state.clone(),
        "DELETE",
        "/api/v1/test-presets/mobile-paywall",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);

    let (after, body) = send(
        db.state.clone(),
        "GET",
        "/api/v1/test-presets/mobile-paywall",
        None,
    )
    .await;
    assert_eq!(after, StatusCode::NOT_FOUND);
    assert_eq!(body["error"]["code"], "TEST_PRESET_NOT_FOUND");
}

#[tokio::test]
async fn create_with_non_object_payload_returns_422() {
    let db = common::setup().await;
    let mut payload = preset_payload("mobile-paywall", "Demo", "rule");
    payload["payload"] = json!([1, 2, 3]);

    let (status, body) = send(
        db.state.clone(),
        "POST",
        "/api/v1/test-presets",
        Some(payload),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(body["error"]["code"], "VALIDATION_ERROR");
    let details = body["error"]["details"].as_array().unwrap();
    assert_eq!(details[0]["rule_id"], "payload_not_object");
    assert_eq!(details[0]["loc"], "payload");
}

#[tokio::test]
async fn create_with_oversized_payload_returns_422() {
    let db = common::setup().await;
    let mut payload = preset_payload("mobile-paywall", "Demo", "rule");
    // > 16 KB serialized.
    payload["payload"] = json!({ "blob": "a".repeat(16_385) });

    let (status, body) = send(
        db.state.clone(),
        "POST",
        "/api/v1/test-presets",
        Some(payload),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(body["error"]["code"], "VALIDATION_ERROR");
    let details = body["error"]["details"].as_array().unwrap();
    assert_eq!(details[0]["rule_id"], "payload_too_large");
}

#[tokio::test]
async fn create_with_invalid_kind_returns_422() {
    let db = common::setup().await;
    let payload = preset_payload("mobile-paywall", "Demo", "other");

    let (status, body) = send(
        db.state.clone(),
        "POST",
        "/api/v1/test-presets",
        Some(payload),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(body["error"]["code"], "VALIDATION_ERROR");
    let details = body["error"]["details"].as_array().unwrap();
    assert_eq!(details[0]["rule_id"], "kind_invalid");
}

#[tokio::test]
async fn create_with_invalid_slug_returns_422() {
    let db = common::setup().await;
    let payload = preset_payload("AB", "Demo", "rule");

    let (status, body) = send(
        db.state.clone(),
        "POST",
        "/api/v1/test-presets",
        Some(payload),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(body["error"]["code"], "VALIDATION_ERROR");
}
