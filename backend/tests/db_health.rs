//! `/healthz/db` pings the pool and returns 200 when reachable.

mod common;

use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use http_body_util::BodyExt;
use rre_backend::build_app;
use tower::ServiceExt;

#[tokio::test]
async fn db_health_returns_ok() {
    let db = common::setup().await;
    let app = build_app(db.state.clone());

    let res = app
        .oneshot(
            Request::builder()
                .uri("/healthz/db")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(res.status(), StatusCode::OK);

    let bytes = res.into_body().collect().await.unwrap().to_bytes();
    let json: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(json["status"], "ok");
}
