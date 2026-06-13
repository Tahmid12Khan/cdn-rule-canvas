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
    seed_draft_version_typed(pool, "html").await
}

/// Insert a feature of the given `type` + a DRAFT version; return the version id.
async fn seed_draft_version_typed(pool: &PgPool, feature_type: &str) -> Uuid {
    let feature_id = format!("feat-{}", &Uuid::new_v4().to_string()[..8]);
    sqlx::query(
        "INSERT INTO rre.features (id, name, type, execution_order) \
         VALUES ($1, 'F', $2::rre.feature_type, \
                 (SELECT COALESCE(MAX(execution_order), 0) + 1 FROM rre.features WHERE type = $2::rre.feature_type))",
    )
        .bind(&feature_id)
        .bind(feature_type)
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

/// Create a library component template via the API; return its id (UUID string).
async fn create_library_component(state: &AppState, slug: &str) -> String {
    let (status, body) = send(
        state,
        "POST",
        "/api/v1/component-templates",
        Some(json!({
            "slug": slug,
            "name": "Lib CTA",
            "html_body": "<div>{{headline}}</div>",
            "variables": [ { "name": "headline", "title": "Headline" } ]
        })),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::CREATED,
        "create library component: {body}"
    );
    body["id"].as_str().unwrap().to_string()
}

/// Create an outcome under `version_id` and return its id (UUID string).
async fn create_outcome(state: &AppState, version_id: Uuid, title: &str) -> String {
    let (status, outcome) = send(
        state,
        "POST",
        &format!("/api/v1/versions/{version_id}/outcomes"),
        Some(json!({ "title": title })),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "create outcome: {outcome}");
    outcome["id"].as_str().unwrap().to_string()
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

// ---- component_ref / component_ref_json (library-Component reference) ----

#[tokio::test]
async fn add_component_ref_persists_and_reads_back() {
    let db = common::setup().await;
    let vid = seed_draft_version(&db.state.pool).await;
    let cid = create_library_component(&db.state, "ref-html-cta").await;
    let oid = create_outcome(&db.state, vid, "Paywall").await;

    let (status, c) = send(
        &db.state,
        "POST",
        &format!("/api/v1/outcomes/{oid}/components"),
        Some(json!({
            "slug": "cta",
            "type": "component_ref",
            "config": {
                "type": "component_ref",
                "component_id": cid,
                "version": "default",
                "variables": { "headline": "Subscribe now" },
                "target_selector": "main .article-body",
                "placement_mode": "append"
            },
            "placement": "inline"
        })),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "add component_ref: {c}");
    let comp_id = c["id"].as_str().unwrap();
    assert_eq!(c["type"], "component_ref");
    assert_eq!(c["config"]["type"], "component_ref");
    assert_eq!(c["config"]["component_id"], cid);
    assert_eq!(c["config"]["version"], "default");
    assert_eq!(c["config"]["variables"]["headline"], "Subscribe now");
    assert_eq!(c["config"]["target_selector"], "main .article-body");
    assert_eq!(c["config"]["placement_mode"], "append");

    // Reads back via the outcome detail endpoint.
    let (status, got) = send(&db.state, "GET", &format!("/api/v1/outcomes/{oid}"), None).await;
    assert_eq!(status, StatusCode::OK);
    let comps = got["components"].as_array().unwrap();
    assert_eq!(comps.len(), 1);
    assert_eq!(comps[0]["id"], comp_id);
    assert_eq!(comps[0]["config"]["type"], "component_ref");
    assert_eq!(comps[0]["config"]["component_id"], cid);
}

#[tokio::test]
async fn add_component_ref_json_persists_and_reads_back() {
    let db = common::setup().await;
    let vid = seed_draft_version_typed(&db.state.pool, "json").await;
    let cid = create_library_component(&db.state, "ref-json-cta").await;
    let oid = create_outcome(&db.state, vid, "JSON outcome").await;

    let (status, c) = send(
        &db.state,
        "POST",
        &format!("/api/v1/outcomes/{oid}/components"),
        Some(json!({
            "slug": "json-cta",
            "type": "component_ref_json",
            "config": {
                "type": "component_ref_json",
                "component_id": cid,
                "version": 1,
                "variables": { "headline": "Hi" },
                "target_path": "$.content.html"
            },
            "placement": "inline"
        })),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "add component_ref_json: {c}");
    assert_eq!(c["type"], "component_ref_json");
    assert_eq!(c["config"]["type"], "component_ref_json");
    assert_eq!(c["config"]["component_id"], cid);
    assert_eq!(c["config"]["version"], 1);
    assert_eq!(c["config"]["target_path"], "$.content.html");

    let (status, got) = send(&db.state, "GET", &format!("/api/v1/outcomes/{oid}"), None).await;
    assert_eq!(status, StatusCode::OK);
    let comps = got["components"].as_array().unwrap();
    assert_eq!(comps.len(), 1);
    assert_eq!(comps[0]["config"]["type"], "component_ref_json");
    assert_eq!(comps[0]["config"]["target_path"], "$.content.html");
}

#[tokio::test]
async fn add_component_ref_unknown_component_is_422_component_ref_exists() {
    let db = common::setup().await;
    let vid = seed_draft_version(&db.state.pool).await;
    let oid = create_outcome(&db.state, vid, "O").await;
    let ghost = Uuid::new_v4().to_string();

    let (status, body) = send(
        &db.state,
        "POST",
        &format!("/api/v1/outcomes/{oid}/components"),
        Some(json!({
            "slug": "cta",
            "type": "component_ref",
            "config": {
                "type": "component_ref",
                "component_id": ghost,
                "version": "default",
                "variables": {},
                "target_selector": "main",
                "placement_mode": "append"
            },
            "placement": "inline"
        })),
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
    assert!(rule_ids.contains(&"component_ref_exists"), "{rule_ids:?}");
    assert_eq!(body["error"]["details"][0]["loc"], "config.component_id");
}

#[tokio::test]
async fn add_component_ref_bad_version_is_422() {
    let db = common::setup().await;
    let vid = seed_draft_version(&db.state.pool).await;
    let cid = create_library_component(&db.state, "ref-badver-cta").await;
    let oid = create_outcome(&db.state, vid, "O").await;

    // version = 0 (not "default", not a positive integer).
    let (status, body) = send(
        &db.state,
        "POST",
        &format!("/api/v1/outcomes/{oid}/components"),
        Some(json!({
            "slug": "cta",
            "type": "component_ref",
            "config": {
                "type": "component_ref",
                "component_id": cid,
                "version": 0,
                "variables": {},
                "target_selector": "main",
                "placement_mode": "append"
            },
            "placement": "inline"
        })),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{body}");
    assert_eq!(body["error"]["code"], "VALIDATION_ERROR");

    // version = "latest" (non-"default" string).
    let (status2, _) = send(
        &db.state,
        "POST",
        &format!("/api/v1/outcomes/{oid}/components"),
        Some(json!({
            "slug": "cta2",
            "type": "component_ref",
            "config": {
                "type": "component_ref",
                "component_id": cid,
                "version": "latest",
                "variables": {},
                "target_selector": "main",
                "placement_mode": "append"
            },
            "placement": "inline"
        })),
    )
    .await;
    assert_eq!(status2, StatusCode::UNPROCESSABLE_ENTITY);

    // version = -3 (negative integer).
    let (status3, _) = send(
        &db.state,
        "POST",
        &format!("/api/v1/outcomes/{oid}/components"),
        Some(json!({
            "slug": "cta3",
            "type": "component_ref",
            "config": {
                "type": "component_ref",
                "component_id": cid,
                "version": -3,
                "variables": {},
                "target_selector": "main",
                "placement_mode": "append"
            },
            "placement": "inline"
        })),
    )
    .await;
    assert_eq!(status3, StatusCode::UNPROCESSABLE_ENTITY);
}
