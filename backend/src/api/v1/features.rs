//! Feature HTTP handlers (BACKEND CONTRACT §7).
//!
//! Routers map `FromRow` models to `*Read` DTOs via the service layer; they
//! never query the DB directly and never serialize a `FromRow` struct.

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    routing::post,
    Json, Router,
};
use serde::Deserialize;

use crate::{
    error::AppResult,
    schemas::{
        active_version::ActiveVersionRead,
        feature::{FeatureCreate, FeatureRead, FeatureUpdate},
        pagination::{Page, PageParams},
        version::PublishEnvironment,
    },
    services::{feature_service, version_service},
    state::AppState,
};

/// Build the feature sub-router.
pub fn router() -> Router<AppState> {
    Router::new()
        .route("/features", post(create).get(list))
        .route(
            "/features/:fid",
            axum::routing::get(get).patch(update).delete(delete),
        )
        .route(
            "/features/:fid/active-version",
            axum::routing::get(active_version),
        )
}

/// `POST /features` — create a feature.
#[utoipa::path(
    post,
    path = "/api/v1/features",
    request_body = FeatureCreate,
    responses(
        (status = 201, description = "Created", body = FeatureRead),
        (status = 409, description = "Slug conflict", body = crate::error::ErrorEnvelope),
        (status = 422, description = "Validation error", body = crate::error::ErrorEnvelope)
    ),
    tag = "features"
)]
pub async fn create(
    State(state): State<AppState>,
    Json(input): Json<FeatureCreate>,
) -> AppResult<impl IntoResponse> {
    let feature = feature_service::create(&state.pool, input).await?;
    Ok((StatusCode::CREATED, Json(feature)))
}

/// `GET /features` — paginated list.
#[utoipa::path(
    get,
    path = "/api/v1/features",
    params(PageParams),
    responses((status = 200, description = "Feature page", body = crate::schemas::pagination::PageFeatureRead)),
    tag = "features"
)]
pub async fn list(
    State(state): State<AppState>,
    Query(params): Query<PageParams>,
) -> AppResult<Json<Page<FeatureRead>>> {
    let page = feature_service::list(&state.pool, &params).await?;
    Ok(Json(page))
}

/// `GET /features/{fid}` — fetch one.
#[utoipa::path(
    get,
    path = "/api/v1/features/{fid}",
    params(("fid" = String, Path, description = "Feature slug")),
    responses(
        (status = 200, description = "Feature", body = FeatureRead),
        (status = 404, description = "Not found", body = crate::error::ErrorEnvelope)
    ),
    tag = "features"
)]
pub async fn get(
    State(state): State<AppState>,
    Path(fid): Path<String>,
) -> AppResult<Json<FeatureRead>> {
    let feature = feature_service::get(&state.pool, &fid).await?;
    Ok(Json(feature))
}

/// `PATCH /features/{fid}` — update name.
#[utoipa::path(
    patch,
    path = "/api/v1/features/{fid}",
    params(("fid" = String, Path, description = "Feature slug")),
    request_body = FeatureUpdate,
    responses(
        (status = 200, description = "Updated", body = FeatureRead),
        (status = 404, description = "Not found", body = crate::error::ErrorEnvelope),
        (status = 422, description = "Validation error", body = crate::error::ErrorEnvelope)
    ),
    tag = "features"
)]
pub async fn update(
    State(state): State<AppState>,
    Path(fid): Path<String>,
    Json(input): Json<FeatureUpdate>,
) -> AppResult<Json<FeatureRead>> {
    let feature = feature_service::update(&state.pool, &fid, input).await?;
    Ok(Json(feature))
}

/// `DELETE /features/{fid}` — delete (cascades).
#[utoipa::path(
    delete,
    path = "/api/v1/features/{fid}",
    params(("fid" = String, Path, description = "Feature slug")),
    responses(
        (status = 204, description = "Deleted"),
        (status = 404, description = "Not found", body = crate::error::ErrorEnvelope)
    ),
    tag = "features"
)]
pub async fn delete(
    State(state): State<AppState>,
    Path(fid): Path<String>,
) -> AppResult<impl IntoResponse> {
    feature_service::delete(&state.pool, &fid).await?;
    Ok(StatusCode::NO_CONTENT)
}

/// Query for the active-version endpoint: `?env=live|staging` (default `live`).
#[derive(Debug, Deserialize)]
pub struct ActiveVersionQuery {
    /// Target environment; defaults to `live`.
    #[serde(default)]
    pub env: Option<PublishEnvironment>,
}

/// `GET /features/{fid}/active-version?env=` — proxy-facing active version.
#[utoipa::path(
    get,
    path = "/api/v1/features/{fid}/active-version",
    params(
        ("fid" = String, Path, description = "Feature slug"),
        ("env" = Option<PublishEnvironment>, Query, description = "Environment (default live)")
    ),
    responses(
        (status = 200, description = "Active version payload", body = ActiveVersionRead),
        (status = 404, description = "No live/staging version", body = crate::error::ErrorEnvelope)
    ),
    tag = "features"
)]
pub async fn active_version(
    State(state): State<AppState>,
    Path(fid): Path<String>,
    Query(query): Query<ActiveVersionQuery>,
) -> AppResult<Json<ActiveVersionRead>> {
    let env = query.env.unwrap_or(PublishEnvironment::Live);
    let payload = version_service::active_version(&state.pool, &fid, env).await?;
    Ok(Json(payload))
}
