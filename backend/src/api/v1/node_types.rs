//! Node-type manifest handler (BACKEND CONTRACT §6/§7).
//!
//! `GET /api/v1/node-types` serves the backend-owned node-type manifest
//! verbatim (the raw parsed JSON), so adding a node type is a JSON-only change.

use axum::{extract::State, routing::get, Json, Router};
use serde_json::Value;

use crate::state::AppState;

/// `GET /api/v1/node-types` — serve the node-type manifest verbatim.
///
/// Cheap and cacheable; the frontend fetches this once per session to build the
/// palette, node titles, tooltips, and config forms.
#[utoipa::path(
    get,
    path = "/api/v1/node-types",
    responses((status = 200, description = "Node-type manifest", body = crate::schemas::node_type::NodeManifest)),
    tag = "node-types"
)]
pub async fn list(State(state): State<AppState>) -> Json<Value> {
    Json((*state.node_manifest_json).clone())
}

/// Router exposing the node-type manifest route.
pub fn router() -> Router<AppState> {
    Router::new().route("/node-types", get(list))
}
