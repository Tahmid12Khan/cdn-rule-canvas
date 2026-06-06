//! HTTP-level integration tests for the site endpoints. Drives the `Router`
//! with `tower::ServiceExt::oneshot` (no network bind), real Postgres backing.
//! Mirrors `features_api.rs` (same harness, same `send` helper).

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

/// A valid `SiteCreate` body. Override fields per-test via the returned `Value`.
fn site_payload(slug: &str, name: &str) -> Value {
    json!({
        "slug": slug,
        "name": name,
        "source_protocol": "http",
        "source_host": "localhost",
        "source_port": 9000,
        "dest_protocol": "http",
        "dest_host": "demo-upstream",
        "dest_port": 8081
    })
}

#[tokio::test]
async fn create_site_returns_201_with_site_read_body() {
    let db = common::setup().await;
    let (status, body) = send(
        db.state.clone(),
        "POST",
        "/api/v1/sites",
        Some(site_payload("demo-localhost", "Demo (localhost:9000)")),
    )
    .await;

    assert_eq!(status, StatusCode::CREATED);
    assert_eq!(body["slug"], "demo-localhost");
    assert_eq!(body["name"], "Demo (localhost:9000)");
    assert_eq!(body["source_protocol"], "http");
    assert_eq!(body["source_host"], "localhost");
    assert_eq!(body["source_port"], 9000);
    assert_eq!(body["dest_protocol"], "http");
    assert_eq!(body["dest_host"], "demo-upstream");
    assert_eq!(body["dest_port"], 8081);
    assert!(body["created_at"].is_string());
    assert!(body["updated_at"].is_string());
}

#[tokio::test]
async fn create_duplicate_slug_returns_409() {
    let db = common::setup().await;
    let first_payload = site_payload("demo-localhost", "Demo One");

    let (first, _) = send(
        db.state.clone(),
        "POST",
        "/api/v1/sites",
        Some(first_payload),
    )
    .await;
    assert_eq!(first, StatusCode::CREATED);

    // Same slug, different name + source so only the PK collides.
    let mut dup = site_payload("demo-localhost", "Demo Two");
    dup["source_host"] = json!("other-host");
    dup["source_port"] = json!(9100);

    let (second, body) = send(db.state.clone(), "POST", "/api/v1/sites", Some(dup)).await;
    assert_eq!(second, StatusCode::CONFLICT);
    assert_eq!(body["error"]["code"], "SLUG_CONFLICT");
}

#[tokio::test]
async fn create_duplicate_name_returns_409() {
    let db = common::setup().await;
    let (first, _) = send(
        db.state.clone(),
        "POST",
        "/api/v1/sites",
        Some(site_payload("site-one", "Shared Name")),
    )
    .await;
    assert_eq!(first, StatusCode::CREATED);

    // Different slug + source, same name → name unique index collides.
    let mut dup = site_payload("site-two", "Shared Name");
    dup["source_host"] = json!("other-host");
    dup["source_port"] = json!(9100);

    let (second, body) = send(db.state.clone(), "POST", "/api/v1/sites", Some(dup)).await;
    assert_eq!(second, StatusCode::CONFLICT);
    assert_eq!(body["error"]["code"], "SLUG_CONFLICT");
}

#[tokio::test]
async fn create_duplicate_source_host_port_returns_409() {
    let db = common::setup().await;
    let (first, _) = send(
        db.state.clone(),
        "POST",
        "/api/v1/sites",
        Some(site_payload("site-one", "Name One")),
    )
    .await;
    assert_eq!(first, StatusCode::CREATED);

    // Different slug + name, same source_host:source_port → source unique index.
    let dup = site_payload("site-two", "Name Two");
    let (second, body) = send(db.state.clone(), "POST", "/api/v1/sites", Some(dup)).await;
    assert_eq!(second, StatusCode::CONFLICT);
    assert_eq!(body["error"]["code"], "SLUG_CONFLICT");
}

#[tokio::test]
async fn list_sites_returns_page_envelope() {
    let db = common::setup().await;
    send(
        db.state.clone(),
        "POST",
        "/api/v1/sites",
        Some(site_payload("demo-localhost", "Demo (localhost:9000)")),
    )
    .await;

    let (status, body) = send(
        db.state.clone(),
        "GET",
        "/api/v1/sites?page=1&page_size=20",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["page"], 1);
    assert_eq!(body["page_size"], 20);
    assert_eq!(body["total"], 1);
    let items = body["items"].as_array().unwrap();
    assert_eq!(items.len(), 1);
    assert_eq!(items[0]["slug"], "demo-localhost");
}

#[tokio::test]
async fn list_sites_filters_by_q_name_substring() {
    let db = common::setup().await;
    send(
        db.state.clone(),
        "POST",
        "/api/v1/sites",
        Some(site_payload("alpha-site", "Alpha Production")),
    )
    .await;
    let mut beta = site_payload("beta-site", "Beta Staging");
    beta["source_host"] = json!("beta-host");
    beta["source_port"] = json!(9100);
    send(db.state.clone(), "POST", "/api/v1/sites", Some(beta)).await;

    // `q` matches a substring of the name, case-insensitive.
    let (status, body) = send(db.state.clone(), "GET", "/api/v1/sites?q=alph", None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["total"], 1);
    let items = body["items"].as_array().unwrap();
    assert_eq!(items.len(), 1);
    assert_eq!(items[0]["slug"], "alpha-site");
}

#[tokio::test]
async fn get_site_returns_200() {
    let db = common::setup().await;
    send(
        db.state.clone(),
        "POST",
        "/api/v1/sites",
        Some(site_payload("demo-localhost", "Demo (localhost:9000)")),
    )
    .await;

    let (status, body) = send(
        db.state.clone(),
        "GET",
        "/api/v1/sites/demo-localhost",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["slug"], "demo-localhost");
    assert_eq!(body["name"], "Demo (localhost:9000)");
}

#[tokio::test]
async fn get_missing_site_returns_404() {
    let db = common::setup().await;
    let (status, body) = send(db.state.clone(), "GET", "/api/v1/sites/nope", None).await;

    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(body["error"]["code"], "SITE_NOT_FOUND");
}

#[tokio::test]
async fn patch_site_partial_update_returns_200_and_slug_immutable() {
    let db = common::setup().await;
    send(
        db.state.clone(),
        "POST",
        "/api/v1/sites",
        Some(site_payload("demo-localhost", "Old Name")),
    )
    .await;

    // Partial update: only `name` and `dest_port`; slug is path-derived, not in body.
    let (status, body) = send(
        db.state.clone(),
        "PATCH",
        "/api/v1/sites/demo-localhost",
        Some(json!({ "name": "New Name", "dest_port": 8082 })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["slug"], "demo-localhost");
    assert_eq!(body["name"], "New Name");
    assert_eq!(body["dest_port"], 8082);
    // Untouched fields are preserved.
    assert_eq!(body["source_host"], "localhost");
    assert_eq!(body["source_port"], 9000);

    // A `slug` in the PATCH body is not an accepted field (SiteUpdate is
    // `deny_unknown_fields`), so the request is rejected before any mutation —
    // proving the slug is immutable / not part of the update body.
    let (rejected, _) = send(
        db.state.clone(),
        "PATCH",
        "/api/v1/sites/demo-localhost",
        Some(json!({ "slug": "renamed-slug" })),
    )
    .await;
    assert!(
        rejected.is_client_error(),
        "slug in PATCH body must be rejected, got {rejected}"
    );

    // The slug is unchanged after the rejected body; the renamed slug does not exist.
    let (still_there, still_body) = send(
        db.state.clone(),
        "GET",
        "/api/v1/sites/demo-localhost",
        None,
    )
    .await;
    assert_eq!(still_there, StatusCode::OK);
    assert_eq!(still_body["slug"], "demo-localhost");

    let (renamed, _) = send(db.state.clone(), "GET", "/api/v1/sites/renamed-slug", None).await;
    assert_eq!(renamed, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn delete_site_returns_204_then_get_404() {
    let db = common::setup().await;
    send(
        db.state.clone(),
        "POST",
        "/api/v1/sites",
        Some(site_payload("demo-localhost", "Doomed")),
    )
    .await;

    let (status, _) = send(
        db.state.clone(),
        "DELETE",
        "/api/v1/sites/demo-localhost",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);

    let (after, body) = send(
        db.state.clone(),
        "GET",
        "/api/v1/sites/demo-localhost",
        None,
    )
    .await;
    assert_eq!(after, StatusCode::NOT_FOUND);
    assert_eq!(body["error"]["code"], "SITE_NOT_FOUND");
}
