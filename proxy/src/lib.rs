//! RRE proxy library crate. Declares the module tree and the `build_app`
//! constructor used by both `main.rs` and the integration tests.

pub mod config;
pub mod domain;
pub mod error;
pub mod eval;
pub mod forwarder;
pub mod full_journey;
pub mod infra;
pub mod middleware;
pub mod observability;
pub mod state;
pub mod telemetry;

use axum::http::HeaderValue;
use axum::routing::{get, post};
use axum::Router;
use tower_http::cors::{AllowHeaders, AllowMethods, CorsLayer};

use crate::state::AppState;

/// Build the proxy router: health + metrics + `/__rre/*` management routes +
/// the catch-all forwarder, wrapped in the trace-id middleware layer.
///
/// CORS: a permissive-for-dev layer is applied only to the `/__rre/*` sub-router
/// so it does not weaken the main proxy pass-through behaviour. It allows
/// `http://localhost:3000` (the frontend dev origin) with GET/POST and
/// Content-Type, and correctly handles OPTIONS preflight.
pub fn build_app(state: AppState) -> Router {
    // /__rre/* sub-router with dev CORS.
    let rre_router = Router::new()
        .route("/health", get(|| async { "ok" }))
        .route("/metrics", get(observability::metrics_handler))
        .route("/__rre/eval", post(eval::eval_handler))
        .route("/__rre/eval-url", post(eval::eval_url_handler))
        .route(
            "/__rre/eval-full-journey",
            post(full_journey::full_journey_handler),
        )
        .layer(dev_cors())
        .with_state(state.clone());

    // Catch-all forwarder (no CORS — proxy behaviour unchanged).
    let proxy_router = Router::new().fallback(forwarder::forward).with_state(state);

    rre_router
        .merge(proxy_router)
        .layer(middleware::trace::layer())
}

/// CORS layer for the `/__rre/*` management endpoints.
/// Allows the frontend dev origin (localhost:3000) with GET/POST + Content-Type.
/// OPTIONS preflights are handled automatically by `tower-http`.
fn dev_cors() -> CorsLayer {
    CorsLayer::new()
        .allow_origin(
            "http://localhost:3000"
                .parse::<HeaderValue>()
                .expect("static origin is valid"),
        )
        .allow_methods(AllowMethods::list([
            axum::http::Method::GET,
            axum::http::Method::POST,
            axum::http::Method::OPTIONS,
        ]))
        .allow_headers(AllowHeaders::list([
            axum::http::header::CONTENT_TYPE,
            axum::http::header::ACCEPT,
        ]))
}
