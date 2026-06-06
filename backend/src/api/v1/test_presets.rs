//! Test-preset HTTP handlers (CONTRACT §7).
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
        pagination::{Page, PageParams},
        test_preset::{TestPresetCreate, TestPresetRead, TestPresetUpdate},
    },
    services::test_preset_service,
    state::AppState,
};

/// Build the test-preset sub-router.
pub fn router() -> Router<AppState> {
    Router::new()
        .route("/test-presets", post(create).get(list))
        .route(
            "/test-presets/{slug}",
            axum::routing::get(get).patch(update).delete(delete),
        )
}

/// Optional filters for the list endpoint.
#[derive(Debug, Default, Deserialize)]
pub struct TestPresetListQuery {
    /// Case-insensitive `name ILIKE '%q%'` filter.
    #[serde(default)]
    pub q: Option<String>,
    /// Filter to a single kind (`rule` | `url`).
    #[serde(default)]
    pub kind: Option<String>,
}

/// `POST /test-presets` — create a preset.
#[utoipa::path(
    post,
    path = "/api/v1/test-presets",
    request_body = TestPresetCreate,
    responses(
        (status = 201, description = "Created", body = TestPresetRead),
        (status = 409, description = "Slug/name conflict", body = crate::error::ErrorEnvelope),
        (status = 422, description = "Validation error", body = crate::error::ErrorEnvelope)
    ),
    tag = "test_presets"
)]
pub async fn create(
    State(state): State<AppState>,
    Json(input): Json<TestPresetCreate>,
) -> AppResult<impl IntoResponse> {
    let preset = test_preset_service::create(&state.pool, input).await?;
    Ok((StatusCode::CREATED, Json(preset)))
}

/// `GET /test-presets?page&page_size&q&kind` — paginated list.
#[utoipa::path(
    get,
    path = "/api/v1/test-presets",
    params(
        PageParams,
        ("q" = Option<String>, Query, description = "Case-insensitive name filter"),
        ("kind" = Option<String>, Query, description = "Filter by kind (rule|url)")
    ),
    responses((status = 200, description = "Test-preset page", body = inline(Page<TestPresetRead>))),
    tag = "test_presets"
)]
pub async fn list(
    State(state): State<AppState>,
    Query(params): Query<PageParams>,
    Query(filter): Query<TestPresetListQuery>,
) -> AppResult<Json<Page<TestPresetRead>>> {
    let page = test_preset_service::list(
        &state.pool,
        &params,
        filter.q.as_deref(),
        filter.kind.as_deref(),
    )
    .await?;
    Ok(Json(page))
}

/// `GET /test-presets/{slug}` — fetch one.
#[utoipa::path(
    get,
    path = "/api/v1/test-presets/{slug}",
    params(("slug" = String, Path, description = "Test-preset slug")),
    responses(
        (status = 200, description = "Test preset", body = TestPresetRead),
        (status = 404, description = "Not found", body = crate::error::ErrorEnvelope)
    ),
    tag = "test_presets"
)]
pub async fn get(
    State(state): State<AppState>,
    Path(slug): Path<String>,
) -> AppResult<Json<TestPresetRead>> {
    let preset = test_preset_service::get(&state.pool, &slug).await?;
    Ok(Json(preset))
}

/// `PATCH /test-presets/{slug}` — partial update (slug + kind immutable).
#[utoipa::path(
    patch,
    path = "/api/v1/test-presets/{slug}",
    params(("slug" = String, Path, description = "Test-preset slug")),
    request_body = TestPresetUpdate,
    responses(
        (status = 200, description = "Updated", body = TestPresetRead),
        (status = 404, description = "Not found", body = crate::error::ErrorEnvelope),
        (status = 409, description = "Name conflict", body = crate::error::ErrorEnvelope),
        (status = 422, description = "Validation error", body = crate::error::ErrorEnvelope)
    ),
    tag = "test_presets"
)]
pub async fn update(
    State(state): State<AppState>,
    Path(slug): Path<String>,
    Json(input): Json<TestPresetUpdate>,
) -> AppResult<Json<TestPresetRead>> {
    let preset = test_preset_service::update(&state.pool, &slug, input).await?;
    Ok(Json(preset))
}

/// `DELETE /test-presets/{slug}` — delete.
#[utoipa::path(
    delete,
    path = "/api/v1/test-presets/{slug}",
    params(("slug" = String, Path, description = "Test-preset slug")),
    responses(
        (status = 204, description = "Deleted"),
        (status = 404, description = "Not found", body = crate::error::ErrorEnvelope)
    ),
    tag = "test_presets"
)]
pub async fn delete(
    State(state): State<AppState>,
    Path(slug): Path<String>,
) -> AppResult<impl IntoResponse> {
    test_preset_service::delete(&state.pool, &slug).await?;
    Ok(StatusCode::NO_CONTENT)
}
