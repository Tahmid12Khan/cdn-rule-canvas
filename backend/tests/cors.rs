//! CORS preflight is answered for the configured frontend origin.

mod common;

use axum::{
    body::Body,
    http::{header, Method, Request, StatusCode},
};
use rre_backend::build_app;
use tower::ServiceExt;

#[tokio::test]
async fn cors_preflight_allows_frontend_origin() {
    let db = common::setup().await;
    let origin = db.state.settings.frontend_origin.clone();
    let app = build_app(db.state.clone());

    let res = app
        .oneshot(
            Request::builder()
                .method(Method::OPTIONS)
                .uri("/api/v1/features")
                .header(header::ORIGIN, &origin)
                .header(header::ACCESS_CONTROL_REQUEST_METHOD, "GET")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    // Preflight should not be rejected and should echo the allow-origin header.
    assert_ne!(res.status(), StatusCode::INTERNAL_SERVER_ERROR);
    let allow_origin = res
        .headers()
        .get(header::ACCESS_CONTROL_ALLOW_ORIGIN)
        .expect("allow-origin header present");
    assert_eq!(allow_origin.to_str().unwrap(), origin);
}
