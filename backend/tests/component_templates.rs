//! HTTP-level integration tests for the Component Editor endpoints
//! (`/api/v1/component-templates`) plus the new rule_graph validation rule_ids
//! (`apply_component_ref_exists` / `apply_component_version_valid`). Drives the
//! real `Router` over a testcontainers Postgres via `tower::ServiceExt::oneshot`
//! (no network bind). Mirrors `test_presets_api.rs` / `version_patch_graph.rs`.

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

/// Create a component, returning its read body.
async fn create_component(state: AppState, slug: &str) -> Value {
    let (status, body) = send(
        state,
        "POST",
        "/api/v1/component-templates",
        Some(json!({
            "slug": slug,
            "name": "Paywall CTA",
            "description": "a reusable cta",
            "html_body": "<div>{{headline}}</div>",
            "variables": [
                { "name": "headline", "title": "Headline", "description": "the big text" }
            ]
        })),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "create should 201: {body}");
    body
}

#[tokio::test]
async fn create_returns_201_with_v1_default_latest() {
    let db = common::setup().await;
    let body = create_component(db.state.clone(), "paywall-cta").await;

    assert_eq!(body["slug"], "paywall-cta");
    assert_eq!(body["name"], "Paywall CTA");
    assert_eq!(body["default_mode"], "latest");
    assert_eq!(body["default_version_number"], 1);
    assert_eq!(body["latest_version_number"], 1);
    let versions = body["versions"].as_array().unwrap();
    assert_eq!(versions.len(), 1);
    assert_eq!(versions[0]["version_number"], 1);
    assert_eq!(versions[0]["is_default"], true);
}

#[tokio::test]
async fn create_duplicate_slug_returns_409() {
    let db = common::setup().await;
    create_component(db.state.clone(), "dup-cta").await;

    let (status, body) = send(
        db.state.clone(),
        "POST",
        "/api/v1/component-templates",
        Some(json!({ "slug": "dup-cta", "name": "Other" })),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(body["error"]["code"], "SLUG_CONFLICT");
}

#[tokio::test]
async fn create_rejects_bad_slug_with_422() {
    let db = common::setup().await;
    let (status, body) = send(
        db.state.clone(),
        "POST",
        "/api/v1/component-templates",
        Some(json!({ "slug": "AB", "name": "x" })),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(body["error"]["code"], "VALIDATION_ERROR");
}

#[tokio::test]
async fn list_returns_page_envelope_and_q_filter() {
    let db = common::setup().await;
    create_component(db.state.clone(), "alpha-cta").await;
    let (s, _) = send(
        db.state.clone(),
        "POST",
        "/api/v1/component-templates",
        Some(json!({ "slug": "beta-banner", "name": "Beta Banner" })),
    )
    .await;
    assert_eq!(s, StatusCode::CREATED);

    let (status, body) = send(
        db.state.clone(),
        "GET",
        "/api/v1/component-templates?page=1&page_size=20",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["page"], 1);
    assert_eq!(body["total"], 2);
    assert_eq!(body["items"].as_array().unwrap().len(), 2);
    // Summary carries the version numbers.
    let item = &body["items"][0];
    assert!(item["latest_version_number"].is_number());
    assert!(item["default_version_number"].is_number());

    // `q` filters case-insensitively on name.
    let (qs, qb) = send(
        db.state.clone(),
        "GET",
        "/api/v1/component-templates?q=beta",
        None,
    )
    .await;
    assert_eq!(qs, StatusCode::OK);
    assert_eq!(qb["total"], 1);
    assert_eq!(qb["items"][0]["slug"], "beta-banner");
}

#[tokio::test]
async fn get_unknown_component_returns_404() {
    let db = common::setup().await;
    let ghost = uuid::Uuid::new_v4();
    let (status, body) = send(
        db.state.clone(),
        "GET",
        &format!("/api/v1/component-templates/{ghost}"),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(body["error"]["code"], "COMPONENT_NOT_FOUND");
}

#[tokio::test]
async fn get_by_slug_returns_component() {
    let db = common::setup().await;
    let created = create_component(db.state.clone(), "by-slug-cta").await;
    let cid = created["id"].as_str().unwrap();

    let (status, body) = send(
        db.state.clone(),
        "GET",
        "/api/v1/component-templates/by-slug/by-slug-cta",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "by-slug should 200: {body}");
    assert_eq!(body["id"], cid);
    assert_eq!(body["slug"], "by-slug-cta");
    assert_eq!(body["name"], "Paywall CTA");
    assert_eq!(body["versions"].as_array().unwrap().len(), 1);

    // The UUID param route still resolves (by-slug must not shadow it).
    let (us, ub) = send(
        db.state.clone(),
        "GET",
        &format!("/api/v1/component-templates/{cid}"),
        None,
    )
    .await;
    assert_eq!(us, StatusCode::OK, "uuid route still works: {ub}");
    assert_eq!(ub["slug"], "by-slug-cta");
}

#[tokio::test]
async fn get_by_slug_unknown_returns_404() {
    let db = common::setup().await;
    let (status, body) = send(
        db.state.clone(),
        "GET",
        "/api/v1/component-templates/by-slug/does-not-exist",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(body["error"]["code"], "COMPONENT_NOT_FOUND");
}

#[tokio::test]
async fn patch_metadata_updates_name_and_description() {
    let db = common::setup().await;
    let comp = create_component(db.state.clone(), "edit-cta").await;
    let cid = comp["id"].as_str().unwrap();

    let (status, body) = send(
        db.state.clone(),
        "PATCH",
        &format!("/api/v1/component-templates/{cid}"),
        Some(json!({ "name": "Renamed CTA", "description": "new desc" })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["name"], "Renamed CTA");
    assert_eq!(body["description"], "new desc");
}

#[tokio::test]
async fn create_version_clones_default_and_advances_latest() {
    let db = common::setup().await;
    let comp = create_component(db.state.clone(), "ver-cta").await;
    let cid = comp["id"].as_str().unwrap();

    // No body supplied → clones v1's html_body + variables.
    let (status, v2) = send(
        db.state.clone(),
        "POST",
        &format!("/api/v1/component-templates/{cid}/versions"),
        Some(json!({ "description": "iteration" })),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::CREATED,
        "create_version should 201: {v2}"
    );
    assert_eq!(v2["version_number"], 2);
    assert_eq!(v2["html_body"], "<div>{{headline}}</div>");
    assert_eq!(v2["variables"][0]["name"], "headline");
    // default_mode latest → the new version is the default.
    assert_eq!(v2["is_default"], true);

    // The component now reports latest = 2 and default = 2.
    let (_, comp2) = send(
        db.state.clone(),
        "GET",
        &format!("/api/v1/component-templates/{cid}"),
        None,
    )
    .await;
    assert_eq!(comp2["latest_version_number"], 2);
    assert_eq!(comp2["default_version_number"], 2);
}

#[tokio::test]
async fn create_version_with_supplied_body_does_not_clone() {
    let db = common::setup().await;
    let comp = create_component(db.state.clone(), "supplied-cta").await;
    let cid = comp["id"].as_str().unwrap();

    let (status, v2) = send(
        db.state.clone(),
        "POST",
        &format!("/api/v1/component-templates/{cid}/versions"),
        Some(json!({
            "html_body": "<section>{{cta}}</section>",
            "variables": [ { "name": "cta", "title": "CTA" } ]
        })),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    assert_eq!(v2["html_body"], "<section>{{cta}}</section>");
    assert_eq!(v2["variables"][0]["name"], "cta");
}

#[tokio::test]
async fn update_version_edits_in_place() {
    let db = common::setup().await;
    let comp = create_component(db.state.clone(), "inplace-cta").await;
    let cid = comp["id"].as_str().unwrap();

    let (status, v1) = send(
        db.state.clone(),
        "PATCH",
        &format!("/api/v1/component-templates/{cid}/versions/1"),
        Some(json!({ "html_body": "<p>{{updated}}</p>", "description": "edited" })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(v1["html_body"], "<p>{{updated}}</p>");
    assert_eq!(v1["description"], "edited");
}

#[tokio::test]
async fn delete_last_version_returns_409_last_version_protected() {
    let db = common::setup().await;
    let comp = create_component(db.state.clone(), "last-ver-cta").await;
    let cid = comp["id"].as_str().unwrap();

    let (status, body) = send(
        db.state.clone(),
        "DELETE",
        &format!("/api/v1/component-templates/{cid}/versions/1"),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(body["error"]["code"], "LAST_VERSION_PROTECTED");
}

#[tokio::test]
async fn delete_non_default_version_allowed() {
    let db = common::setup().await;
    let comp = create_component(db.state.clone(), "del-nondefault").await;
    let cid = comp["id"].as_str().unwrap();

    // Add v2 (becomes default under latest).
    send(
        db.state.clone(),
        "POST",
        &format!("/api/v1/component-templates/{cid}/versions"),
        Some(json!({})),
    )
    .await;

    // Delete v1 (not the default) → 204.
    let (status, _) = send(
        db.state.clone(),
        "DELETE",
        &format!("/api/v1/component-templates/{cid}/versions/1"),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);

    let (_, comp2) = send(
        db.state.clone(),
        "GET",
        &format!("/api/v1/component-templates/{cid}"),
        None,
    )
    .await;
    assert_eq!(comp2["versions"].as_array().unwrap().len(), 1);
    assert_eq!(comp2["versions"][0]["version_number"], 2);
}

#[tokio::test]
async fn make_default_pins_version_then_resolve_returns_it() {
    let db = common::setup().await;
    let comp = create_component(db.state.clone(), "pin-cta").await;
    let cid = comp["id"].as_str().unwrap();

    // Add v2 with a distinct body.
    send(
        db.state.clone(),
        "POST",
        &format!("/api/v1/component-templates/{cid}/versions"),
        Some(json!({ "html_body": "<b>v2</b>" })),
    )
    .await;

    // Pin v1 as default.
    let (status, comp2) = send(
        db.state.clone(),
        "POST",
        &format!("/api/v1/component-templates/{cid}/versions/1/make-default"),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(comp2["default_mode"], "pinned");
    assert_eq!(comp2["default_version_number"], 1);

    // resolve?version=default → v1.
    let (rs, resolved) = send(
        db.state.clone(),
        "GET",
        &format!("/api/v1/component-templates/{cid}/resolve?version=default"),
        None,
    )
    .await;
    assert_eq!(rs, StatusCode::OK);
    assert_eq!(resolved["version_number"], 1);
    assert_eq!(resolved["html_body"], "<div>{{headline}}</div>");
    assert!(resolved["variables"].is_array());
}

#[tokio::test]
async fn delete_pinned_default_repoints_to_newest_remaining() {
    let db = common::setup().await;
    let comp = create_component(db.state.clone(), "repoint-cta").await;
    let cid = comp["id"].as_str().unwrap();

    // Add v2 and v3.
    send(
        db.state.clone(),
        "POST",
        &format!("/api/v1/component-templates/{cid}/versions"),
        Some(json!({})),
    )
    .await;
    send(
        db.state.clone(),
        "POST",
        &format!("/api/v1/component-templates/{cid}/versions"),
        Some(json!({})),
    )
    .await;

    // Pin v2 as default.
    let (s, _) = send(
        db.state.clone(),
        "POST",
        &format!("/api/v1/component-templates/{cid}/versions/2/make-default"),
        None,
    )
    .await;
    assert_eq!(s, StatusCode::OK);

    // Delete v2 (the pinned default) → 204, default re-points to v3 (newest remaining).
    let (status, _) = send(
        db.state.clone(),
        "DELETE",
        &format!("/api/v1/component-templates/{cid}/versions/2"),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);

    let (_, comp2) = send(
        db.state.clone(),
        "GET",
        &format!("/api/v1/component-templates/{cid}"),
        None,
    )
    .await;
    assert_eq!(comp2["default_mode"], "pinned");
    assert_eq!(comp2["default_version_number"], 3);
}

#[tokio::test]
async fn resolve_pinned_then_missing_version_falls_back_to_default() {
    let db = common::setup().await;
    let comp = create_component(db.state.clone(), "fallback-cta").await;
    let cid = comp["id"].as_str().unwrap();

    // Add v2 and pin it as the default.
    send(
        db.state.clone(),
        "POST",
        &format!("/api/v1/component-templates/{cid}/versions"),
        Some(json!({ "html_body": "<b>v2 default</b>" })),
    )
    .await;
    send(
        db.state.clone(),
        "POST",
        &format!("/api/v1/component-templates/{cid}/versions/2/make-default"),
        None,
    )
    .await;

    // resolve?version=99 (missing) → falls back to the current default (v2).
    let (status, resolved) = send(
        db.state.clone(),
        "GET",
        &format!("/api/v1/component-templates/{cid}/resolve?version=99"),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(resolved["version_number"], 2);
    assert_eq!(resolved["html_body"], "<b>v2 default</b>");
}

#[tokio::test]
async fn resolve_specific_existing_version_returns_it() {
    let db = common::setup().await;
    let comp = create_component(db.state.clone(), "specific-cta").await;
    let cid = comp["id"].as_str().unwrap();
    send(
        db.state.clone(),
        "POST",
        &format!("/api/v1/component-templates/{cid}/versions"),
        Some(json!({ "html_body": "<b>v2</b>" })),
    )
    .await;

    let (status, resolved) = send(
        db.state.clone(),
        "GET",
        &format!("/api/v1/component-templates/{cid}/resolve?version=1"),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(resolved["version_number"], 1);
    assert_eq!(resolved["html_body"], "<div>{{headline}}</div>");
}

#[tokio::test]
async fn switch_default_to_pinned_without_number_returns_422() {
    let db = common::setup().await;
    let comp = create_component(db.state.clone(), "badpin-cta").await;
    let cid = comp["id"].as_str().unwrap();

    let (status, body) = send(
        db.state.clone(),
        "PATCH",
        &format!("/api/v1/component-templates/{cid}"),
        Some(json!({ "default_mode": "pinned" })),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(body["error"]["code"], "VALIDATION_ERROR");
}

#[tokio::test]
async fn switch_default_back_to_latest_clears_pin() {
    let db = common::setup().await;
    let comp = create_component(db.state.clone(), "tolatest-cta").await;
    let cid = comp["id"].as_str().unwrap();
    send(
        db.state.clone(),
        "POST",
        &format!("/api/v1/component-templates/{cid}/versions"),
        Some(json!({})),
    )
    .await;
    // Pin v1.
    send(
        db.state.clone(),
        "POST",
        &format!("/api/v1/component-templates/{cid}/versions/1/make-default"),
        None,
    )
    .await;
    // Back to latest → default tracks v2.
    let (status, body) = send(
        db.state.clone(),
        "PATCH",
        &format!("/api/v1/component-templates/{cid}"),
        Some(json!({ "default_mode": "latest" })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["default_mode"], "latest");
    assert_eq!(body["default_version_number"], 2);
}

#[tokio::test]
async fn delete_component_returns_204_then_404() {
    let db = common::setup().await;
    let comp = create_component(db.state.clone(), "gone-cta").await;
    let cid = comp["id"].as_str().unwrap();

    let (status, _) = send(
        db.state.clone(),
        "DELETE",
        &format!("/api/v1/component-templates/{cid}"),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);

    let (status, body) = send(
        db.state.clone(),
        "GET",
        &format!("/api/v1/component-templates/{cid}"),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(body["error"]["code"], "COMPONENT_NOT_FOUND");
}

// ---- rule_graph validation: apply_component rule_ids ----

/// Create an HTML feature + DRAFT version; returns `(version_number)`.
async fn seed_html_version(state: AppState, slug: &str) -> i64 {
    let (s, _) = send(
        state.clone(),
        "POST",
        "/api/v1/features",
        Some(json!({ "id": slug, "name": "F", "type": "html" })),
    )
    .await;
    assert_eq!(s, StatusCode::CREATED);
    let (s, v) = send(
        state.clone(),
        "POST",
        &format!("/api/v1/features/{slug}/versions"),
        Some(json!({ "description": "init" })),
    )
    .await;
    assert_eq!(s, StatusCode::CREATED);
    v["version_number"].as_i64().unwrap()
}

fn apply_component_graph(component_id: &str, version: Value) -> Value {
    json!({
        "canvas": {
            "root_node_id": "start",
            "nodes": [
                { "kind": "start", "id": "start", "position": { "x": 0.0, "y": 0.0 } },
                { "kind": "expression", "id": "a1",
                  "action": {
                      "type": "apply_component",
                      "component_id": component_id,
                      "version": version,
                      "target_selector": "main",
                      "placement_mode": "append"
                  },
                  "position": { "x": 200.0, "y": 0.0 } },
                { "kind": "end", "id": "end", "position": { "x": 400.0, "y": 0.0 } }
            ],
            "edges": [
                { "id": "e0", "source_node_id": "start", "target_node_id": "a1", "branch": "yes" },
                { "id": "e1", "source_node_id": "a1", "target_node_id": "end", "branch": "yes" }
            ]
        }
    })
}

#[tokio::test]
async fn patch_apply_component_with_existing_component_succeeds() {
    let db = common::setup().await;
    let comp = create_component(db.state.clone(), "graph-ok-cta").await;
    let cid = comp["id"].as_str().unwrap();
    let vnum = seed_html_version(db.state.clone(), "graph-ok-feat").await;

    let (status, body) = send(
        db.state.clone(),
        "PATCH",
        &format!("/api/v1/features/graph-ok-feat/versions/{vnum}"),
        Some(json!({ "rule_graph": apply_component_graph(cid, json!("default")) })),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::OK,
        "valid apply_component graph: {body}"
    );
}

#[tokio::test]
async fn patch_apply_component_unknown_component_rejected() {
    let db = common::setup().await;
    let vnum = seed_html_version(db.state.clone(), "graph-ghost-feat").await;
    let ghost = uuid::Uuid::new_v4().to_string();

    let (status, body) = send(
        db.state.clone(),
        "PATCH",
        &format!("/api/v1/features/graph-ghost-feat/versions/{vnum}"),
        Some(json!({ "rule_graph": apply_component_graph(&ghost, json!("default")) })),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(body["error"]["code"], "VALIDATION_ERROR");
    let rule_ids: Vec<&str> = body["error"]["details"]
        .as_array()
        .unwrap()
        .iter()
        .map(|d| d["rule_id"].as_str().unwrap())
        .collect();
    assert!(
        rule_ids.contains(&"apply_component_ref_exists"),
        "{rule_ids:?}"
    );
}

#[tokio::test]
async fn patch_apply_component_bad_version_rejected() {
    let db = common::setup().await;
    let comp = create_component(db.state.clone(), "graph-badver-cta").await;
    let cid = comp["id"].as_str().unwrap();
    let vnum = seed_html_version(db.state.clone(), "graph-badver-feat").await;

    // version = 0 is not "default" nor a positive integer.
    let (status, body) = send(
        db.state.clone(),
        "PATCH",
        &format!("/api/v1/features/graph-badver-feat/versions/{vnum}"),
        Some(json!({ "rule_graph": apply_component_graph(cid, json!(0)) })),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    let rule_ids: Vec<&str> = body["error"]["details"]
        .as_array()
        .unwrap()
        .iter()
        .map(|d| d["rule_id"].as_str().unwrap())
        .collect();
    assert!(
        rule_ids.contains(&"apply_component_version_valid"),
        "{rule_ids:?}"
    );
}

#[tokio::test]
async fn patch_apply_component_pinned_number_is_well_formed() {
    let db = common::setup().await;
    let comp = create_component(db.state.clone(), "graph-pin-cta").await;
    let cid = comp["id"].as_str().unwrap();
    let vnum = seed_html_version(db.state.clone(), "graph-pin-feat").await;

    // A pinned version number that does not exist on the component is NOT a
    // save-time error (drift is fail-open at the proxy) — only well-formedness.
    let (status, body) = send(
        db.state.clone(),
        "PATCH",
        &format!("/api/v1/features/graph-pin-feat/versions/{vnum}"),
        Some(json!({ "rule_graph": apply_component_graph(cid, json!(7)) })),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::OK,
        "pinned version number is well-formed: {body}"
    );
}
