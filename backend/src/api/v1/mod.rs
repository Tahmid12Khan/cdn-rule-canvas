//! API v1 router tree (BACKEND CONTRACT §7).
//!
//! Each domain handler module exposes `pub fn router() -> Router<AppState>`;
//! they are merged here and mounted under `/api/v1` (health lives at the root,
//! mounted in `lib.rs`). Routers receive services/pool via `State<AppState>`.

use axum::{routing::get, Router};

use crate::state::AppState;

pub mod health;

// Domain-owned handler modules (each exposes `pub fn router() -> Router<AppState>`):
pub mod component_templates;
pub mod components;
pub mod features;
pub mod node_types;
pub mod outcomes;
pub mod products;
pub mod saved_outcomes;
pub mod sites;
pub mod test_presets;
pub mod versions;

/// Root-level health routes (mounted outside the `/api/v1` prefix).
pub fn health_router() -> Router<AppState> {
    Router::new()
        .route("/health", get(health::health))
        .route("/healthz/db", get(health::healthz_db))
}

/// All `/api/v1/*` routes merged into one router.
pub fn router() -> Router<AppState> {
    Router::new()
        .merge(node_types::router())
        .merge(features::router())
        .merge(versions::router())
        .merge(outcomes::router())
        .merge(components::router())
        .merge(products::router())
        .merge(component_templates::router())
        .merge(saved_outcomes::router())
        .merge(sites::router())
        .merge(test_presets::router())
}
