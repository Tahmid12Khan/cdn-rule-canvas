//! Outcomes Library HTTP handlers.
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
use uuid::Uuid;

use crate::{
    error::AppResult,
    schemas::{
        pagination::{Page, PageParams},
        saved_outcome::{
            ResolvedSavedOutcomeRead, SavedOutcomeCreate, SavedOutcomeRead, SavedOutcomeUpdate,
        },
    },
    services::saved_outcome_service,
    state::AppState,
};

/// Build the saved-outcome sub-router.
pub fn router() -> Router<AppState> {
    Router::new()
        .route("/saved-outcomes", post(create).get(list))
        .route(
            "/saved-outcomes/{id}",
            axum::routing::get(get).patch(update).delete(delete),
        )
        .route("/saved-outcomes/{id}/resolve", axum::routing::get(resolve))
}

/// Optional `q` filter for the list endpoint (case-insensitive name search).
#[derive(Debug, Default, Deserialize)]
pub struct SavedOutcomeListQuery {
    /// Case-insensitive `name ILIKE '%q%'` filter.
    #[serde(default)]
    pub q: Option<String>,
}

/// `POST /saved-outcomes` — create a saved outcome.
#[utoipa::path(
    post,
    path = "/api/v1/saved-outcomes",
    request_body = SavedOutcomeCreate,
    responses(
        (status = 201, description = "Created", body = SavedOutcomeRead),
        (status = 404, description = "Referenced component not found", body = crate::error::ErrorEnvelope),
        (status = 409, description = "Slug/name conflict", body = crate::error::ErrorEnvelope),
        (status = 422, description = "Validation error", body = crate::error::ErrorEnvelope)
    ),
    tag = "saved-outcomes"
)]
pub async fn create(
    State(state): State<AppState>,
    Json(input): Json<SavedOutcomeCreate>,
) -> AppResult<impl IntoResponse> {
    let outcome = saved_outcome_service::create(&state.pool, input).await?;
    Ok((StatusCode::CREATED, Json(outcome)))
}

/// `GET /saved-outcomes?page&page_size&q` — paginated list.
#[utoipa::path(
    get,
    path = "/api/v1/saved-outcomes",
    params(PageParams, ("q" = Option<String>, Query, description = "Case-insensitive name filter")),
    responses((status = 200, description = "Saved outcome page", body = inline(Page<SavedOutcomeRead>))),
    tag = "saved-outcomes"
)]
pub async fn list(
    State(state): State<AppState>,
    Query(params): Query<PageParams>,
    Query(filter): Query<SavedOutcomeListQuery>,
) -> AppResult<Json<Page<SavedOutcomeRead>>> {
    let page = saved_outcome_service::list(&state.pool, &params, filter.q.as_deref()).await?;
    Ok(Json(page))
}

/// `GET /saved-outcomes/{id}` — fetch one.
#[utoipa::path(
    get,
    path = "/api/v1/saved-outcomes/{id}",
    params(("id" = Uuid, Path, description = "Saved outcome id")),
    responses(
        (status = 200, description = "Saved outcome", body = SavedOutcomeRead),
        (status = 404, description = "Not found", body = crate::error::ErrorEnvelope)
    ),
    tag = "saved-outcomes"
)]
pub async fn get(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> AppResult<Json<SavedOutcomeRead>> {
    let outcome = saved_outcome_service::get(&state.pool, id).await?;
    Ok(Json(outcome))
}

/// `PATCH /saved-outcomes/{id}` — partial update.
#[utoipa::path(
    patch,
    path = "/api/v1/saved-outcomes/{id}",
    params(("id" = Uuid, Path, description = "Saved outcome id")),
    request_body = SavedOutcomeUpdate,
    responses(
        (status = 200, description = "Updated", body = SavedOutcomeRead),
        (status = 404, description = "Not found", body = crate::error::ErrorEnvelope),
        (status = 409, description = "Name conflict", body = crate::error::ErrorEnvelope),
        (status = 422, description = "Validation error", body = crate::error::ErrorEnvelope)
    ),
    tag = "saved-outcomes"
)]
pub async fn update(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Json(input): Json<SavedOutcomeUpdate>,
) -> AppResult<Json<SavedOutcomeRead>> {
    let outcome = saved_outcome_service::update(&state.pool, id, input).await?;
    Ok(Json(outcome))
}

/// `DELETE /saved-outcomes/{id}` — delete.
#[utoipa::path(
    delete,
    path = "/api/v1/saved-outcomes/{id}",
    params(("id" = Uuid, Path, description = "Saved outcome id")),
    responses(
        (status = 204, description = "Deleted"),
        (status = 404, description = "Not found", body = crate::error::ErrorEnvelope)
    ),
    tag = "saved-outcomes"
)]
pub async fn delete(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> AppResult<impl IntoResponse> {
    saved_outcome_service::delete(&state.pool, id).await?;
    Ok(StatusCode::NO_CONTENT)
}

/// `GET /saved-outcomes/{id}/resolve` — proxy-facing: render the referenced
/// component version's `html_body` against this saved outcome's `variables`.
#[utoipa::path(
    get,
    path = "/api/v1/saved-outcomes/{id}/resolve",
    params(("id" = Uuid, Path, description = "Saved outcome id")),
    responses(
        (status = 200, description = "Resolved and rendered HTML", body = ResolvedSavedOutcomeRead),
        (status = 404, description = "Saved outcome or component not found", body = crate::error::ErrorEnvelope)
    ),
    tag = "saved-outcomes"
)]
pub async fn resolve(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> AppResult<Json<ResolvedSavedOutcomeRead>> {
    let resolved = saved_outcome_service::resolve(&state.pool, id).await?;
    Ok(Json(resolved))
}
