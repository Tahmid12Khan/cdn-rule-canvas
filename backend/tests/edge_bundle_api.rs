//! HTTP-level integration tests for the edge-bundle export route. Drives the
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
use sqlx::PgPool;
use tower::ServiceExt;

/// Helper: send a request and return `(status, body json)`.
async fn send(state: rre_backend::state::AppState, uri: &str) -> (StatusCode, Value) {
    let app = build_app(state);
    let res = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(uri)
                .body(Body::empty())
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

async fn seed_site(pool: &PgPool, slug: &str, source_host: &str) {
    sqlx::query(
        "INSERT INTO rre.sites (slug, name, source_protocol, source_host, source_port, \
         dest_protocol, dest_host, dest_port, headers) \
         VALUES ($1, $2, 'https', $3, 443, 'https', 'origin.example.com', 443, '{}'::jsonb)",
    )
    .bind(slug)
    .bind(format!("Site {slug}"))
    .bind(source_host)
    .execute(pool)
    .await
    .expect("seed site");
}

/// The route is what the export script calls; it must return the bundle
/// verbatim with a JSON content type.
#[tokio::test]
async fn get_edge_bundle_returns_the_bundle() {
    let db = common::setup().await;
    seed_site(&db.state.pool, "intrafish-com", "test.intrafish.com").await;

    let (status, body) = send(
        db.state.clone(),
        "/api/v1/sites/intrafish-com/edge-bundle?env=live",
    )
    .await;

    assert_eq!(status, StatusCode::OK, "body: {body}");
    assert_eq!(body["schema_version"], json!(1));
    assert_eq!(body["site"]["slug"], json!("intrafish-com"));
    assert_eq!(body["site"]["source_host"], json!("test.intrafish.com"));
    assert_eq!(body["environment"], json!("live"));
    assert_eq!(body["features"], json!([]));
    assert!(body["generated_at"].is_string());
}

/// `env` is optional and defaults to `live`, matching the active-version route.
#[tokio::test]
async fn get_edge_bundle_defaults_to_live() {
    let db = common::setup().await;
    seed_site(&db.state.pool, "intrafish-com", "test.intrafish.com").await;

    let (status, body) = send(db.state.clone(), "/api/v1/sites/intrafish-com/edge-bundle").await;

    assert_eq!(status, StatusCode::OK, "body: {body}");
    assert_eq!(body["environment"], json!("live"));
}

#[tokio::test]
async fn get_edge_bundle_serves_staging() {
    let db = common::setup().await;
    seed_site(&db.state.pool, "intrafish-com", "test.intrafish.com").await;

    let (status, body) = send(
        db.state.clone(),
        "/api/v1/sites/intrafish-com/edge-bundle?env=staging",
    )
    .await;

    assert_eq!(status, StatusCode::OK, "body: {body}");
    assert_eq!(body["environment"], json!("staging"));
}

/// An unrecognised environment is rejected before the handler runs, so a typo in
/// an export script fails loudly instead of quietly exporting `live`.
#[tokio::test]
async fn get_edge_bundle_rejects_a_bad_env() {
    let db = common::setup().await;

    let (status, _) = send(db.state.clone(), "/api/v1/sites/x/edge-bundle?env=prod").await;

    assert_eq!(status, StatusCode::BAD_REQUEST);
}

/// The route must appear in the published OpenAPI document. Registering the
/// handler in the router but forgetting `openapi.rs` is invisible at runtime and
/// leaves the export undiscoverable, so it is asserted rather than reviewed.
#[test]
fn edge_bundle_route_is_documented() {
    use utoipa::OpenApi;

    let doc = serde_json::to_value(rre_backend::openapi::ApiDoc::openapi()).unwrap();
    let path = &doc["paths"]["/api/v1/sites/{slug}/edge-bundle"]["get"];
    assert!(!path.is_null(), "route missing from the OpenAPI document");
    assert_eq!(path["tags"], json!(["sites"]));
}

/// An unknown site is the uniform 404 envelope, not an empty bundle.
#[tokio::test]
async fn get_edge_bundle_404s_an_unknown_site() {
    let db = common::setup().await;

    let (status, body) = send(db.state.clone(), "/api/v1/sites/nope/edge-bundle?env=live").await;

    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(body["error"]["code"], json!("SITE_NOT_FOUND"));
}

/// The response must parse as the exact struct the proxy and the Fastly guest
/// deserialize — the whole point of the route.
#[tokio::test]
async fn get_edge_bundle_body_parses_as_an_rre_core_bundle() {
    let db = common::setup().await;
    seed_site(&db.state.pool, "intrafish-com", "test.intrafish.com").await;

    let (status, body) = send(
        db.state.clone(),
        "/api/v1/sites/intrafish-com/edge-bundle?env=live",
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    let bundle: rre_core::edge::EdgeBundle =
        serde_json::from_value(body).expect("response parses as an EdgeBundle");
    assert_eq!(bundle.schema_version, rre_core::edge::SCHEMA_VERSION);
}
