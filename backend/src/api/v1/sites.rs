//! Site HTTP handlers (sites host-config design §4).
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
        site::{SiteCreate, SiteRead, SiteUpdate},
    },
    services::site_service,
    state::AppState,
};

/// Build the site sub-router.
pub fn router() -> Router<AppState> {
    Router::new().route("/sites", post(create).get(list)).route(
        "/sites/{slug}",
        axum::routing::get(get).patch(update).delete(delete),
    )
}

/// Optional `q` filter for the list endpoint (case-insensitive name search).
#[derive(Debug, Default, Deserialize)]
pub struct SiteListQuery {
    /// Case-insensitive `name ILIKE '%q%'` filter for the site picker.
    #[serde(default)]
    pub q: Option<String>,
}

/// `POST /sites` — create a site.
#[utoipa::path(
    post,
    path = "/api/v1/sites",
    request_body = SiteCreate,
    responses(
        (status = 201, description = "Created", body = SiteRead),
        (status = 409, description = "Slug/name/source conflict", body = crate::error::ErrorEnvelope),
        (status = 422, description = "Validation error", body = crate::error::ErrorEnvelope)
    ),
    tag = "sites"
)]
pub async fn create(
    State(state): State<AppState>,
    Json(input): Json<SiteCreate>,
) -> AppResult<impl IntoResponse> {
    let site = site_service::create(&state.pool, input).await?;
    Ok((StatusCode::CREATED, Json(site)))
}

/// `GET /sites?page&page_size&q` — paginated list.
#[utoipa::path(
    get,
    path = "/api/v1/sites",
    params(PageParams, ("q" = Option<String>, Query, description = "Case-insensitive name filter")),
    responses((status = 200, description = "Site page", body = inline(Page<SiteRead>))),
    tag = "sites"
)]
pub async fn list(
    State(state): State<AppState>,
    Query(params): Query<PageParams>,
    Query(filter): Query<SiteListQuery>,
) -> AppResult<Json<Page<SiteRead>>> {
    let page = site_service::list(&state.pool, &params, filter.q.as_deref()).await?;
    Ok(Json(page))
}

/// `GET /sites/{slug}` — fetch one.
#[utoipa::path(
    get,
    path = "/api/v1/sites/{slug}",
    params(("slug" = String, Path, description = "Site slug")),
    responses(
        (status = 200, description = "Site", body = SiteRead),
        (status = 404, description = "Not found", body = crate::error::ErrorEnvelope)
    ),
    tag = "sites"
)]
pub async fn get(
    State(state): State<AppState>,
    Path(slug): Path<String>,
) -> AppResult<Json<SiteRead>> {
    let site = site_service::get(&state.pool, &slug).await?;
    Ok(Json(site))
}

/// `PATCH /sites/{slug}` — partial update.
#[utoipa::path(
    patch,
    path = "/api/v1/sites/{slug}",
    params(("slug" = String, Path, description = "Site slug")),
    request_body = SiteUpdate,
    responses(
        (status = 200, description = "Updated", body = SiteRead),
        (status = 404, description = "Not found", body = crate::error::ErrorEnvelope),
        (status = 409, description = "Name/source conflict", body = crate::error::ErrorEnvelope),
        (status = 422, description = "Validation error", body = crate::error::ErrorEnvelope)
    ),
    tag = "sites"
)]
pub async fn update(
    State(state): State<AppState>,
    Path(slug): Path<String>,
    Json(input): Json<SiteUpdate>,
) -> AppResult<Json<SiteRead>> {
    let site = site_service::update(&state.pool, &slug, input).await?;
    Ok(Json(site))
}

/// `DELETE /sites/{slug}` — delete.
#[utoipa::path(
    delete,
    path = "/api/v1/sites/{slug}",
    params(("slug" = String, Path, description = "Site slug")),
    responses(
        (status = 204, description = "Deleted"),
        (status = 404, description = "Not found", body = crate::error::ErrorEnvelope)
    ),
    tag = "sites"
)]
pub async fn delete(
    State(state): State<AppState>,
    Path(slug): Path<String>,
) -> AppResult<impl IntoResponse> {
    site_service::delete(&state.pool, &slug).await?;
    Ok(StatusCode::NO_CONTENT)
}
