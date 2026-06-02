//! RRE backend library crate.
//!
//! Layering: `api/v1` (routers) → `services` (business logic, tx boundary,
//! domain errors) → `repositories` (SQLx, returns models) → `models`
//! (`FromRow`). `schemas` holds serde DTOs.

#![forbid(unsafe_code)]

pub mod api;
pub mod config;
pub mod db;
pub mod error;
pub mod models;
pub mod openapi;
pub mod repositories;
pub mod schemas;
pub mod services;
pub mod state;
pub mod telemetry;

use axum::{http::HeaderValue, Router};
use tower_http::cors::{AllowHeaders, AllowMethods, CorsLayer};
use utoipa::OpenApi;
use utoipa_swagger_ui::SwaggerUi;

use crate::{openapi::ApiDoc, state::AppState};

/// Build the full application router: health routes, `/api/v1/*`, Swagger UI at
/// `/docs`, and a CORS layer scoped to `settings.frontend_origin`.
pub fn build_app(state: AppState) -> Router {
    let cors = build_cors(&state.settings.frontend_origin);

    Router::new()
        .merge(api::v1::health_router())
        .nest("/api/v1", api::v1::router())
        .merge(SwaggerUi::new("/docs").url("/api-docs/openapi.json", ApiDoc::openapi()))
        .layer(cors)
        .with_state(state)
}

/// Construct the CORS layer scoped to `frontend_origin`. Fails CLOSED: an
/// unparseable `FRONTEND_ORIGIN` is a fatal startup configuration error
/// (panics at build time, never at request time), rather than silently
/// downgrading to a permissive any-origin policy.
fn build_cors(frontend_origin: &str) -> CorsLayer {
    let layer = CorsLayer::new()
        .allow_methods(AllowMethods::any())
        .allow_headers(AllowHeaders::any());

    let origin = HeaderValue::from_str(frontend_origin).unwrap_or_else(|_| {
        panic!("invalid FRONTEND_ORIGIN '{frontend_origin}': not a valid header value")
    });
    layer.allow_origin(origin)
}
