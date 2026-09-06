//! HTTP-level integration tests for the Outcomes Library endpoints
//! (`/api/v1/saved-outcomes`). Drives the real `Router` over a testcontainers
//! Postgres via `tower::ServiceExt::oneshot` (no network bind). Mirrors
//! `products_api.rs` / `component_templates.rs`.

mod common;

use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use http_body_util::BodyExt;
use rre_backend::{build_app, state::AppState};
use serde_json::{json, Value};
use tower::ServiceExt;

/// Send a JSON request and return `(status, body json)`.
async fn send(
    state: AppState,
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

/// Create a component with a mustache body referencing `{{headline}}`, returning its read body.
async fn create_component(state: AppState, slug: &str) -> Value {
    let (status, body) = send(
        state,
        "POST",
        "/api/v1/component-templates",
        Some(json!({
            "slug": slug,
            "name": "Promo Banner",
            "html_body": "<div>{{headline}}</div>",
            "variables": [
                { "name": "headline", "title": "Headline" }
            ]
        })),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "create component: {body}");
    body
}

/// A valid `SavedOutcomeCreate` body pointing at `component_id`.
fn saved_outcome_payload(slug: &str, name: &str, component_id: &str) -> Value {
    json!({
        "slug": slug,
        "name": name,
        "component_id": component_id,
        "variables": { "headline": "Hello there" }
    })
}

#[tokio::test]
async fn create_and_get_saved_outcome() {
    let db = common::setup().await;
    let comp = create_component(db.state.clone(), "promo-banner").await;
    let cid = comp["id"].as_str().unwrap();

    let (status, body) = send(
        db.state.clone(),
        "POST",
        "/api/v1/saved-outcomes",
        Some(saved_outcome_payload(
            "promo-default",
            "Promo (Default)",
            cid,
        )),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "create should 201: {body}");
    assert_eq!(body["slug"], "promo-default");
    assert_eq!(body["name"], "Promo (Default)");
    assert_eq!(body["component_id"], cid);
    assert_eq!(body["component_name"], "Promo Banner");
    assert!(body["version_number"].is_null());
    assert_eq!(body["variables"]["headline"], "Hello there");

    let id = body["id"].as_str().unwrap();
    let (gstatus, gbody) = send(
        db.state.clone(),
        "GET",
        &format!("/api/v1/saved-outcomes/{id}"),
        None,
    )
    .await;
    assert_eq!(gstatus, StatusCode::OK);
    assert_eq!(gbody["slug"], "promo-default");
}

#[tokio::test]
async fn create_duplicate_slug_returns_409() {
    let db = common::setup().await;
    let comp = create_component(db.state.clone(), "dup-slug-cta").await;
    let cid = comp["id"].as_str().unwrap();

    let (first, _) = send(
        db.state.clone(),
        "POST",
        "/api/v1/saved-outcomes",
        Some(saved_outcome_payload("dup-slug", "One", cid)),
    )
    .await;
    assert_eq!(first, StatusCode::CREATED);

    let (second, body) = send(
        db.state.clone(),
        "POST",
        "/api/v1/saved-outcomes",
        Some(saved_outcome_payload("dup-slug", "Two", cid)),
    )
    .await;
    assert_eq!(second, StatusCode::CONFLICT);
    assert_eq!(body["error"]["code"], "SLUG_CONFLICT");
}

#[tokio::test]
async fn create_duplicate_name_returns_409() {
    let db = common::setup().await;
    let comp = create_component(db.state.clone(), "dup-name-cta").await;
    let cid = comp["id"].as_str().unwrap();

    let (first, _) = send(
        db.state.clone(),
        "POST",
        "/api/v1/saved-outcomes",
        Some(saved_outcome_payload("slug-one", "Shared Name", cid)),
    )
    .await;
    assert_eq!(first, StatusCode::CREATED);

    let (second, body) = send(
        db.state.clone(),
        "POST",
        "/api/v1/saved-outcomes",
        Some(saved_outcome_payload("slug-two", "Shared Name", cid)),
    )
    .await;
    assert_eq!(second, StatusCode::CONFLICT);
    assert_eq!(body["error"]["code"], "SLUG_CONFLICT");
}

#[tokio::test]
async fn create_with_unknown_component_id_returns_422() {
    let db = common::setup().await;
    let ghost = uuid::Uuid::new_v4().to_string();

    let (status, body) = send(
        db.state.clone(),
        "POST",
        "/api/v1/saved-outcomes",
        Some(saved_outcome_payload("ghost-outcome", "Ghost", &ghost)),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{body}");
    assert_eq!(body["error"]["code"], "VALIDATION_ERROR");
    let rule_ids: Vec<&str> = body["error"]["details"]
        .as_array()
        .unwrap()
        .iter()
        .map(|d| d["rule_id"].as_str().unwrap())
        .collect();
    assert!(rule_ids.contains(&"component_ref_exists"), "{rule_ids:?}");
}

#[tokio::test]
async fn list_saved_outcomes_returns_page_envelope_and_q_filter() {
    let db = common::setup().await;
    let comp = create_component(db.state.clone(), "list-cta").await;
    let cid = comp["id"].as_str().unwrap();

    send(
        db.state.clone(),
        "POST",
        "/api/v1/saved-outcomes",
        Some(saved_outcome_payload("list-alpha", "Alpha Outcome", cid)),
    )
    .await;
    send(
        db.state.clone(),
        "POST",
        "/api/v1/saved-outcomes",
        Some(saved_outcome_payload("list-beta", "Beta Outcome", cid)),
    )
    .await;

    let (status, body) = send(
        db.state.clone(),
        "GET",
        "/api/v1/saved-outcomes?page=1&page_size=20",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["page"], 1);
    assert_eq!(body["page_size"], 20);
    assert_eq!(body["total"], 2);
    let items = body["items"].as_array().unwrap();
    assert_eq!(items.len(), 2);
    assert_eq!(items[0]["component_name"], "Promo Banner");

    let (qs, qb) = send(
        db.state.clone(),
        "GET",
        "/api/v1/saved-outcomes?q=alpha",
        None,
    )
    .await;
    assert_eq!(qs, StatusCode::OK);
    assert_eq!(qb["total"], 1);
    assert_eq!(qb["items"][0]["slug"], "list-alpha");
}

#[tokio::test]
async fn patch_variables_only_and_pins_specific_version() {
    let db = common::setup().await;
    let comp = create_component(db.state.clone(), "patch-vars-cta").await;
    let cid = comp["id"].as_str().unwrap();
    // Add v2 so pinning to 2 is a real, distinct version.
    let (vstatus, _) = send(
        db.state.clone(),
        "POST",
        &format!("/api/v1/component-templates/{cid}/versions"),
        Some(json!({})),
    )
    .await;
    assert_eq!(vstatus, StatusCode::CREATED);

    let (status, so) = send(
        db.state.clone(),
        "POST",
        "/api/v1/saved-outcomes",
        Some(saved_outcome_payload(
            "patch-vars-outcome",
            "Patch Vars",
            cid,
        )),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    let id = so["id"].as_str().unwrap();

    // Variables-only update leaves name/version_number untouched.
    let (s1, b1) = send(
        db.state.clone(),
        "PATCH",
        &format!("/api/v1/saved-outcomes/{id}"),
        Some(json!({ "variables": { "headline": "Updated Value" } })),
    )
    .await;
    assert_eq!(s1, StatusCode::OK, "{b1}");
    assert_eq!(b1["name"], "Patch Vars");
    assert_eq!(b1["variables"]["headline"], "Updated Value");

    // Pinning a specific version number (double-Option `Some(Some(n))`).
    let (s2, b2) = send(
        db.state.clone(),
        "PATCH",
        &format!("/api/v1/saved-outcomes/{id}"),
        Some(json!({ "version_number": 2 })),
    )
    .await;
    assert_eq!(s2, StatusCode::OK, "{b2}");
    assert_eq!(b2["version_number"], 2);
}

#[tokio::test]
async fn delete_component_referenced_by_saved_outcome_returns_409() {
    let db = common::setup().await;
    let comp = create_component(db.state.clone(), "referenced-cta").await;
    let cid = comp["id"].as_str().unwrap();

    let (status, _) = send(
        db.state.clone(),
        "POST",
        "/api/v1/saved-outcomes",
        Some(saved_outcome_payload(
            "referenced-outcome",
            "Referenced",
            cid,
        )),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);

    let (dstatus, dbody) = send(
        db.state.clone(),
        "DELETE",
        &format!("/api/v1/component-templates/{cid}"),
        None,
    )
    .await;
    assert_eq!(dstatus, StatusCode::CONFLICT, "delete should 409: {dbody}");
    assert_eq!(dbody["error"]["code"], "COMPONENT_IN_USE");
}

#[tokio::test]
async fn resolve_with_default_version_uses_component_default() {
    let db = common::setup().await;
    let comp = create_component(db.state.clone(), "resolve-default-cta").await;
    let cid = comp["id"].as_str().unwrap();

    let (status, so) = send(
        db.state.clone(),
        "POST",
        "/api/v1/saved-outcomes",
        Some(saved_outcome_payload(
            "resolve-default",
            "Resolve Default",
            cid,
        )),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    let id = so["id"].as_str().unwrap();

    let (rstatus, rbody) = send(
        db.state.clone(),
        "GET",
        &format!("/api/v1/saved-outcomes/{id}/resolve"),
        None,
    )
    .await;
    assert_eq!(rstatus, StatusCode::OK, "resolve should 200: {rbody}");
    assert_eq!(rbody["html_body"], "<div>Hello there</div>");
}

#[tokio::test]
async fn resolve_with_pinned_version_uses_that_version() {
    let db = common::setup().await;
    let comp = create_component(db.state.clone(), "resolve-pinned-cta").await;
    let cid = comp["id"].as_str().unwrap();

    // Add v2 with a distinct body.
    let (vstatus, _) = send(
        db.state.clone(),
        "POST",
        &format!("/api/v1/component-templates/{cid}/versions"),
        Some(json!({
            "html_body": "<section>{{headline}}</section>",
            "variables": [ { "name": "headline", "title": "Headline" } ]
        })),
    )
    .await;
    assert_eq!(vstatus, StatusCode::CREATED);

    let mut payload = saved_outcome_payload("resolve-pinned", "Resolve Pinned", cid);
    payload["version_number"] = json!(1);
    let (status, so) = send(
        db.state.clone(),
        "POST",
        "/api/v1/saved-outcomes",
        Some(payload),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    let id = so["id"].as_str().unwrap();

    let (rstatus, rbody) = send(
        db.state.clone(),
        "GET",
        &format!("/api/v1/saved-outcomes/{id}/resolve"),
        None,
    )
    .await;
    assert_eq!(rstatus, StatusCode::OK, "resolve should 200: {rbody}");
    // Pinned at v1, whose body is the original `<div>{{headline}}</div>`.
    assert_eq!(rbody["html_body"], "<div>Hello there</div>");
}

#[tokio::test]
async fn resolve_renders_mustache_against_variables() {
    let db = common::setup().await;
    let comp = create_component(db.state.clone(), "resolve-vars-cta").await;
    let cid = comp["id"].as_str().unwrap();

    let mut payload = saved_outcome_payload("resolve-vars", "Resolve Vars", cid);
    payload["variables"] = json!({ "headline": "Custom Value 42" });
    let (status, so) = send(
        db.state.clone(),
        "POST",
        "/api/v1/saved-outcomes",
        Some(payload),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    let id = so["id"].as_str().unwrap();

    let (rstatus, rbody) = send(
        db.state.clone(),
        "GET",
        &format!("/api/v1/saved-outcomes/{id}/resolve"),
        None,
    )
    .await;
    assert_eq!(rstatus, StatusCode::OK);
    assert_eq!(rbody["html_body"], "<div>Custom Value 42</div>");
}

#[tokio::test]
async fn get_unknown_saved_outcome_returns_404() {
    let db = common::setup().await;
    let ghost = uuid::Uuid::new_v4();
    let (status, body) = send(
        db.state.clone(),
        "GET",
        &format!("/api/v1/saved-outcomes/{ghost}"),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(body["error"]["code"], "SAVED_OUTCOME_NOT_FOUND");
}

#[tokio::test]
async fn resolve_unknown_saved_outcome_returns_404() {
    let db = common::setup().await;
    let ghost = uuid::Uuid::new_v4();
    let (status, body) = send(
        db.state.clone(),
        "GET",
        &format!("/api/v1/saved-outcomes/{ghost}/resolve"),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(body["error"]["code"], "SAVED_OUTCOME_NOT_FOUND");
}

#[tokio::test]
async fn patch_updates_name_and_clears_version_via_null() {
    let db = common::setup().await;
    let comp = create_component(db.state.clone(), "patch-cta").await;
    let cid = comp["id"].as_str().unwrap();

    let mut payload = saved_outcome_payload("patch-outcome", "Original", cid);
    payload["version_number"] = json!(1);
    let (status, so) = send(
        db.state.clone(),
        "POST",
        "/api/v1/saved-outcomes",
        Some(payload),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    assert_eq!(so["version_number"], 1);
    let id = so["id"].as_str().unwrap();

    // Name-only update leaves version_number untouched (field omitted).
    let (s1, b1) = send(
        db.state.clone(),
        "PATCH",
        &format!("/api/v1/saved-outcomes/{id}"),
        Some(json!({ "name": "Renamed" })),
    )
    .await;
    assert_eq!(s1, StatusCode::OK, "{b1}");
    assert_eq!(b1["name"], "Renamed");
    assert_eq!(b1["version_number"], 1);

    // Explicit JSON null clears the pin back to "Latest".
    let (s2, b2) = send(
        db.state.clone(),
        "PATCH",
        &format!("/api/v1/saved-outcomes/{id}"),
        Some(json!({ "version_number": null })),
    )
    .await;
    assert_eq!(s2, StatusCode::OK, "{b2}");
    assert!(b2["version_number"].is_null());
}

#[tokio::test]
async fn delete_saved_outcome_returns_204_then_404() {
    let db = common::setup().await;
    let comp = create_component(db.state.clone(), "delete-cta").await;
    let cid = comp["id"].as_str().unwrap();

    let (status, so) = send(
        db.state.clone(),
        "POST",
        "/api/v1/saved-outcomes",
        Some(saved_outcome_payload("delete-outcome", "Doomed", cid)),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    let id = so["id"].as_str().unwrap();

    let (dstatus, _) = send(
        db.state.clone(),
        "DELETE",
        &format!("/api/v1/saved-outcomes/{id}"),
        None,
    )
    .await;
    assert_eq!(dstatus, StatusCode::NO_CONTENT);

    let (gstatus, gbody) = send(
        db.state.clone(),
        "GET",
        &format!("/api/v1/saved-outcomes/{id}"),
        None,
    )
    .await;
    assert_eq!(gstatus, StatusCode::NOT_FOUND);
    assert_eq!(gbody["error"]["code"], "SAVED_OUTCOME_NOT_FOUND");
}
