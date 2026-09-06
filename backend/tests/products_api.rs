//! HTTP-level integration tests for the product endpoints. Drives the
//! `Router` with `tower::ServiceExt::oneshot` (no network bind), real
//! Postgres backing. Mirrors `sites_api.rs` (same harness, same `send` helper).

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

/// A valid `ProductCreate` body. Override fields per-test via the returned `Value`.
fn product_payload(label: &str, name: &str) -> Value {
    json!({
        "label": label,
        "name": name,
        "description": "A premium entitlement"
    })
}

#[tokio::test]
async fn create_product_returns_201_with_product_read_body() {
    let db = common::setup().await;
    let (status, body) = send(
        db.state.clone(),
        "POST",
        "/api/v1/products",
        Some(product_payload("premium", "Premium")),
    )
    .await;

    assert_eq!(status, StatusCode::CREATED);
    assert_eq!(body["label"], "premium");
    assert_eq!(body["name"], "Premium");
    assert_eq!(body["description"], "A premium entitlement");
    assert!(body["created_at"].is_string());
    assert!(body["updated_at"].is_string());
}

#[tokio::test]
async fn create_duplicate_label_returns_409() {
    let db = common::setup().await;
    let (first, _) = send(
        db.state.clone(),
        "POST",
        "/api/v1/products",
        Some(product_payload("premium", "Premium One")),
    )
    .await;
    assert_eq!(first, StatusCode::CREATED);

    let dup = product_payload("premium", "Premium Two");
    let (second, body) = send(db.state.clone(), "POST", "/api/v1/products", Some(dup)).await;
    assert_eq!(second, StatusCode::CONFLICT);
    assert_eq!(body["error"]["code"], "SLUG_CONFLICT");
}

#[tokio::test]
async fn create_duplicate_name_returns_409() {
    let db = common::setup().await;
    let (first, _) = send(
        db.state.clone(),
        "POST",
        "/api/v1/products",
        Some(product_payload("premium_one", "Shared Name")),
    )
    .await;
    assert_eq!(first, StatusCode::CREATED);

    let dup = product_payload("premium_two", "Shared Name");
    let (second, body) = send(db.state.clone(), "POST", "/api/v1/products", Some(dup)).await;
    assert_eq!(second, StatusCode::CONFLICT);
    assert_eq!(body["error"]["code"], "SLUG_CONFLICT");
}

#[tokio::test]
async fn create_uppercase_label_returns_422() {
    let db = common::setup().await;
    let (status, body) = send(
        db.state.clone(),
        "POST",
        "/api/v1/products",
        Some(product_payload("Premium", "Premium")),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(body["error"]["code"], "VALIDATION_ERROR");
}

#[tokio::test]
async fn create_kebab_case_label_returns_422() {
    let db = common::setup().await;
    let (status, body) = send(
        db.state.clone(),
        "POST",
        "/api/v1/products",
        Some(product_payload("premium-tier", "Premium")),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(body["error"]["code"], "VALIDATION_ERROR");
}

#[tokio::test]
async fn list_products_returns_page_envelope() {
    let db = common::setup().await;
    send(
        db.state.clone(),
        "POST",
        "/api/v1/products",
        Some(product_payload("premium", "Premium")),
    )
    .await;

    let (status, body) = send(
        db.state.clone(),
        "GET",
        "/api/v1/products?page=1&page_size=20",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["page"], 1);
    assert_eq!(body["page_size"], 20);
    assert_eq!(body["total"], 1);
    let items = body["items"].as_array().unwrap();
    assert_eq!(items.len(), 1);
    assert_eq!(items[0]["label"], "premium");
}

#[tokio::test]
async fn list_products_filters_by_q_name_substring() {
    let db = common::setup().await;
    send(
        db.state.clone(),
        "POST",
        "/api/v1/products",
        Some(product_payload("alpha", "Alpha Tier")),
    )
    .await;
    send(
        db.state.clone(),
        "POST",
        "/api/v1/products",
        Some(product_payload("beta", "Beta Tier")),
    )
    .await;

    let (status, body) = send(db.state.clone(), "GET", "/api/v1/products?q=alph", None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["total"], 1);
    let items = body["items"].as_array().unwrap();
    assert_eq!(items.len(), 1);
    assert_eq!(items[0]["label"], "alpha");
}

#[tokio::test]
async fn get_product_returns_200() {
    let db = common::setup().await;
    send(
        db.state.clone(),
        "POST",
        "/api/v1/products",
        Some(product_payload("premium", "Premium")),
    )
    .await;

    let (status, body) = send(db.state.clone(), "GET", "/api/v1/products/premium", None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["label"], "premium");
    assert_eq!(body["name"], "Premium");
}

#[tokio::test]
async fn get_missing_product_returns_404() {
    let db = common::setup().await;
    let (status, body) = send(db.state.clone(), "GET", "/api/v1/products/nope", None).await;

    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(body["error"]["code"], "PRODUCT_NOT_FOUND");
}

#[tokio::test]
async fn patch_product_partial_update_returns_200_and_label_immutable() {
    let db = common::setup().await;
    send(
        db.state.clone(),
        "POST",
        "/api/v1/products",
        Some(product_payload("premium", "Old Name")),
    )
    .await;

    let (status, body) = send(
        db.state.clone(),
        "PATCH",
        "/api/v1/products/premium",
        Some(json!({ "name": "New Name" })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["label"], "premium");
    assert_eq!(body["name"], "New Name");
    // Untouched field preserved.
    assert_eq!(body["description"], "A premium entitlement");

    // A `label` in the PATCH body is not an accepted field (ProductUpdate is
    // `deny_unknown_fields`), so the request is rejected before any mutation —
    // proving the label is immutable / not part of the update body.
    let (rejected, _) = send(
        db.state.clone(),
        "PATCH",
        "/api/v1/products/premium",
        Some(json!({ "label": "renamed" })),
    )
    .await;
    assert!(
        rejected.is_client_error(),
        "label in PATCH body must be rejected, got {rejected}"
    );

    let (still_there, still_body) =
        send(db.state.clone(), "GET", "/api/v1/products/premium", None).await;
    assert_eq!(still_there, StatusCode::OK);
    assert_eq!(still_body["label"], "premium");

    let (renamed, _) = send(db.state.clone(), "GET", "/api/v1/products/renamed", None).await;
    assert_eq!(renamed, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn delete_product_returns_204_then_get_404() {
    let db = common::setup().await;
    send(
        db.state.clone(),
        "POST",
        "/api/v1/products",
        Some(product_payload("premium", "Doomed")),
    )
    .await;

    let (status, _) = send(db.state.clone(), "DELETE", "/api/v1/products/premium", None).await;
    assert_eq!(status, StatusCode::NO_CONTENT);

    let (after, body) = send(db.state.clone(), "GET", "/api/v1/products/premium", None).await;
    assert_eq!(after, StatusCode::NOT_FOUND);
    assert_eq!(body["error"]["code"], "PRODUCT_NOT_FOUND");
}
