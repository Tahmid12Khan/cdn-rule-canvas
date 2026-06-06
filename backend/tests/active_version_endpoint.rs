//! Tests for the active-version read (`version_service::active_version`) and the
//! `GET /api/v1/features/{fid}/active-version` endpoint.
//!
//! The HTTP handler (`features::active_version`) is owned by the features
//! module and delegates to `version_service::active_version`; these tests
//! exercise the service directly (canonical payload assembly) plus an HTTP
//! smoke test of the routed endpoint.

mod common;

use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use http_body_util::BodyExt;
use rre_backend::{
    build_app,
    error::AppError,
    schemas::version::{PublishEnvironment, VersionCreate},
    services::version_service,
    state::AppState,
};
use serde_json::Value;
use sqlx::PgPool;
use tower::ServiceExt;
use uuid::Uuid;

const FID: &str = "demo-article";

async fn seed_feature(pool: &PgPool, id: &str) {
    sqlx::query("INSERT INTO rre.features (id, name, type) VALUES ($1, $2, 'html')")
        .bind(id)
        .bind(format!("Feature {id}"))
        .execute(pool)
        .await
        .expect("seed feature");
}

/// Create + publish a version to the given environment; returns its number.
async fn publish_new(state: &AppState, env: PublishEnvironment) -> (Uuid, i32) {
    let body = VersionCreate {
        description: None,
        ..Default::default()
    };
    let v = version_service::create_version(&state.pool, FID, body, &state.node_manifest)
        .await
        .unwrap();
    version_service::publish(&state.pool, FID, v.version_number, env)
        .await
        .unwrap();
    (v.id, v.version_number)
}

/// Insert an outcome row for a version and return its id.
async fn seed_outcome(pool: &PgPool, version_id: Uuid, title: &str, order: i32) -> Uuid {
    sqlx::query_scalar(
        "INSERT INTO rre.outcomes (version_id, title, is_builtin, order_index) \
         VALUES ($1, $2, false, $3) RETURNING id",
    )
    .bind(version_id)
    .bind(title)
    .bind(order)
    .fetch_one(pool)
    .await
    .expect("seed outcome")
}

/// Insert a component row for an outcome.
async fn seed_component(pool: &PgPool, outcome_id: Uuid, slug: &str, order: i32) {
    let config = serde_json::json!({
        "type": "html_injection",
        "target_selector": "article",
        "placement_mode": "append",
        "html_body": "<p>hi</p>"
    });
    sqlx::query(
        "INSERT INTO rre.components (outcome_id, slug, type, config, placement, order_index) \
         VALUES ($1, $2, 'html_injection', $3, 'inline', $4)",
    )
    .bind(outcome_id)
    .bind(slug)
    .bind(config)
    .bind(order)
    .execute(pool)
    .await
    .expect("seed component");
}

#[tokio::test]
async fn active_version_returns_outcomes_with_components_ordered() {
    let db = common::setup().await;
    let pool = &db.state.pool;
    seed_feature(pool, FID).await;

    let (vid, vnum) = publish_new(&db.state, PublishEnvironment::Live).await;

    // The builtin outcome is at order 0; add a second outcome with components.
    let o2 = seed_outcome(pool, vid, "Paywall", 1).await;
    seed_component(pool, o2, "comp-b", 1).await;
    seed_component(pool, o2, "comp-a", 0).await;

    let av = version_service::active_version(pool, FID, PublishEnvironment::Live)
        .await
        .expect("active version");

    assert_eq!(av.version_number, vnum);
    assert_eq!(av.outcomes.len(), 2);

    // Outcomes ordered by order_index ASC: builtin (0) then Paywall (1).
    assert!(av.outcomes[0].is_builtin);
    assert_eq!(av.outcomes[0].title, "Show Content");
    assert_eq!(av.outcomes[1].title, "Paywall");

    // Components ordered by order_index ASC.
    let comps = &av.outcomes[1].components;
    assert_eq!(comps.len(), 2);
    assert_eq!(comps[0].slug, "comp-a");
    assert_eq!(comps[1].slug, "comp-b");
    assert_eq!(comps[0].order_index, 0);
}

#[tokio::test]
async fn active_version_staging_environment() {
    let db = common::setup().await;
    let pool = &db.state.pool;
    seed_feature(pool, FID).await;

    let (_vid, vnum) = publish_new(&db.state, PublishEnvironment::Staging).await;

    let av = version_service::active_version(pool, FID, PublishEnvironment::Staging)
        .await
        .expect("active staging version");
    assert_eq!(av.version_number, vnum);

    // No live version → 404 for the live environment.
    let err = version_service::active_version(pool, FID, PublishEnvironment::Live)
        .await
        .unwrap_err();
    assert!(matches!(err, AppError::NoLiveVersion(_)));
}

#[tokio::test]
async fn active_version_unknown_feature_is_404() {
    let db = common::setup().await;
    let err = version_service::active_version(&db.state.pool, "nope", PublishEnvironment::Live)
        .await
        .unwrap_err();
    assert!(matches!(err, AppError::FeatureNotFound(_)));
}

#[tokio::test]
async fn active_version_http_endpoint_returns_payload() {
    let db = common::setup().await;
    seed_feature(&db.state.pool, FID).await;
    let (_vid, vnum) = publish_new(&db.state, PublishEnvironment::Live).await;

    let app = build_app(db.state.clone());
    let res = app
        .oneshot(
            Request::builder()
                .uri("/api/v1/features/demo-article/active-version?env=live")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(res.status(), StatusCode::OK);
    let bytes = res.into_body().collect().await.unwrap().to_bytes();
    let body: Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(body["version_number"], vnum);
    assert!(body["rule_graph"]["anonymous"].is_object());
    assert!(body["outcomes"].is_array());
}
