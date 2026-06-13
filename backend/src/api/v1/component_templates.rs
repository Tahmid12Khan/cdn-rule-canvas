//! Component-template HTTP handlers (Component Editor design §3.5).
//!
//! Routers map `FromRow` models to `*Read`/`*Summary` DTOs via the service
//! layer; they never query the DB directly and never serialize a `FromRow`
//! struct. `{cid}` = component UUID; `{vnum}` = `version_number` i32.

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use serde::Deserialize;
use uuid::Uuid;

use crate::{
    error::AppResult,
    schemas::{
        component_template::{
            ComponentTemplateCreate, ComponentTemplateRead, ComponentTemplateSummary,
            ComponentTemplateUpdate, ComponentTemplateVersionRead, ResolvedComponentRead,
            VersionCreate, VersionUpdate,
        },
        pagination::{Page, PageParams},
    },
    services::component_template_service::{self, VersionSelector},
    state::AppState,
};

/// Build the component-template sub-router.
pub fn router() -> Router<AppState> {
    Router::new()
        .route("/component-templates", post(create).get(list))
        .route("/component-templates/by-slug/{slug}", get(get_by_slug))
        .route(
            "/component-templates/{cid}",
            get(get_one).patch(update).delete(delete),
        )
        .route(
            "/component-templates/{cid}/versions",
            post(create_version).get(list_versions),
        )
        .route(
            "/component-templates/{cid}/versions/{vnum}",
            get(get_version)
                .patch(update_version)
                .delete(delete_version),
        )
        .route(
            "/component-templates/{cid}/versions/{vnum}/make-default",
            post(make_default),
        )
        .route("/component-templates/{cid}/resolve", get(resolve))
}

/// Optional `?q` name filter for the list endpoint.
#[derive(Debug, Default, Deserialize)]
pub struct ComponentListQuery {
    /// Case-insensitive `name ILIKE '%q%'` filter.
    #[serde(default)]
    pub q: Option<String>,
}

/// `?version=default|N` query param for the resolve endpoint.
#[derive(Debug, Default, Deserialize)]
pub struct ResolveQuery {
    /// `default` (or absent) → current default; a positive integer → that version.
    #[serde(default)]
    pub version: Option<String>,
}

/// `POST /component-templates` — create a component + its v1.
#[utoipa::path(
    post,
    path = "/api/v1/component-templates",
    request_body = ComponentTemplateCreate,
    responses(
        (status = 201, description = "Created", body = ComponentTemplateRead),
        (status = 409, description = "Slug conflict", body = crate::error::ErrorEnvelope),
        (status = 422, description = "Validation error", body = crate::error::ErrorEnvelope)
    ),
    tag = "component_templates"
)]
pub async fn create(
    State(state): State<AppState>,
    Json(input): Json<ComponentTemplateCreate>,
) -> AppResult<impl IntoResponse> {
    let component = component_template_service::create(&state.pool, input).await?;
    Ok((StatusCode::CREATED, Json(component)))
}

/// `GET /component-templates?page&page_size&q` — paginated list.
#[utoipa::path(
    get,
    path = "/api/v1/component-templates",
    params(
        PageParams,
        ("q" = Option<String>, Query, description = "Case-insensitive name filter")
    ),
    responses((status = 200, description = "Component page", body = inline(Page<ComponentTemplateSummary>))),
    tag = "component_templates"
)]
pub async fn list(
    State(state): State<AppState>,
    Query(params): Query<PageParams>,
    Query(filter): Query<ComponentListQuery>,
) -> AppResult<Json<Page<ComponentTemplateSummary>>> {
    let page = component_template_service::list(&state.pool, &params, filter.q.as_deref()).await?;
    Ok(Json(page))
}

/// `GET /component-templates/{cid}` — fetch with version summaries.
#[utoipa::path(
    get,
    path = "/api/v1/component-templates/{cid}",
    params(("cid" = Uuid, Path, description = "Component id")),
    responses(
        (status = 200, description = "Component", body = ComponentTemplateRead),
        (status = 404, description = "Not found", body = crate::error::ErrorEnvelope)
    ),
    tag = "component_templates"
)]
pub async fn get_one(
    State(state): State<AppState>,
    Path(cid): Path<Uuid>,
) -> AppResult<Json<ComponentTemplateRead>> {
    let component = component_template_service::get_with_versions(&state.pool, cid).await?;
    Ok(Json(component))
}

/// `GET /component-templates/by-slug/{slug}` — fetch with version summaries by
/// slug. The static `by-slug` segment is registered before `{cid}` so it is not
/// shadowed by the UUID param route.
#[utoipa::path(
    get,
    path = "/api/v1/component-templates/by-slug/{slug}",
    params(("slug" = String, Path, description = "Component slug")),
    responses(
        (status = 200, description = "Component", body = ComponentTemplateRead),
        (status = 404, description = "Not found", body = crate::error::ErrorEnvelope)
    ),
    tag = "component_templates"
)]
pub async fn get_by_slug(
    State(state): State<AppState>,
    Path(slug): Path<String>,
) -> AppResult<Json<ComponentTemplateRead>> {
    let component =
        component_template_service::get_with_versions_by_slug(&state.pool, &slug).await?;
    Ok(Json(component))
}

/// `PATCH /component-templates/{cid}` — update metadata + default pointer.
#[utoipa::path(
    patch,
    path = "/api/v1/component-templates/{cid}",
    params(("cid" = Uuid, Path, description = "Component id")),
    request_body = ComponentTemplateUpdate,
    responses(
        (status = 200, description = "Updated", body = ComponentTemplateRead),
        (status = 404, description = "Not found", body = crate::error::ErrorEnvelope),
        (status = 409, description = "Slug conflict", body = crate::error::ErrorEnvelope),
        (status = 422, description = "Validation error", body = crate::error::ErrorEnvelope)
    ),
    tag = "component_templates"
)]
pub async fn update(
    State(state): State<AppState>,
    Path(cid): Path<Uuid>,
    Json(input): Json<ComponentTemplateUpdate>,
) -> AppResult<Json<ComponentTemplateRead>> {
    let component = component_template_service::update(&state.pool, cid, input).await?;
    Ok(Json(component))
}

/// `DELETE /component-templates/{cid}` — delete (cascades versions).
#[utoipa::path(
    delete,
    path = "/api/v1/component-templates/{cid}",
    params(("cid" = Uuid, Path, description = "Component id")),
    responses(
        (status = 204, description = "Deleted"),
        (status = 404, description = "Not found", body = crate::error::ErrorEnvelope)
    ),
    tag = "component_templates"
)]
pub async fn delete(
    State(state): State<AppState>,
    Path(cid): Path<Uuid>,
) -> AppResult<impl IntoResponse> {
    component_template_service::delete(&state.pool, cid).await?;
    Ok(StatusCode::NO_CONTENT)
}

/// `GET /component-templates/{cid}/versions` — list full version reads.
#[utoipa::path(
    get,
    path = "/api/v1/component-templates/{cid}/versions",
    params(("cid" = Uuid, Path, description = "Component id")),
    responses(
        (status = 200, description = "Versions", body = inline(Vec<ComponentTemplateVersionRead>)),
        (status = 404, description = "Not found", body = crate::error::ErrorEnvelope)
    ),
    tag = "component_templates"
)]
pub async fn list_versions(
    State(state): State<AppState>,
    Path(cid): Path<Uuid>,
) -> AppResult<Json<Vec<ComponentTemplateVersionRead>>> {
    let versions = component_template_service::list_versions(&state.pool, cid).await?;
    Ok(Json(versions))
}

/// `POST /component-templates/{cid}/versions` — create a new version.
#[utoipa::path(
    post,
    path = "/api/v1/component-templates/{cid}/versions",
    params(("cid" = Uuid, Path, description = "Component id")),
    request_body = VersionCreate,
    responses(
        (status = 201, description = "Created", body = ComponentTemplateVersionRead),
        (status = 404, description = "Not found", body = crate::error::ErrorEnvelope),
        (status = 422, description = "Validation error", body = crate::error::ErrorEnvelope)
    ),
    tag = "component_templates"
)]
pub async fn create_version(
    State(state): State<AppState>,
    Path(cid): Path<Uuid>,
    Json(input): Json<VersionCreate>,
) -> AppResult<impl IntoResponse> {
    let version = component_template_service::create_version(&state.pool, cid, input).await?;
    Ok((StatusCode::CREATED, Json(version)))
}

/// `GET /component-templates/{cid}/versions/{vnum}` — fetch one version.
#[utoipa::path(
    get,
    path = "/api/v1/component-templates/{cid}/versions/{vnum}",
    params(
        ("cid" = Uuid, Path, description = "Component id"),
        ("vnum" = i32, Path, description = "Version number")
    ),
    responses(
        (status = 200, description = "Version", body = ComponentTemplateVersionRead),
        (status = 404, description = "Not found", body = crate::error::ErrorEnvelope)
    ),
    tag = "component_templates"
)]
pub async fn get_version(
    State(state): State<AppState>,
    Path((cid, vnum)): Path<(Uuid, i32)>,
) -> AppResult<Json<ComponentTemplateVersionRead>> {
    let version = component_template_service::get_version(&state.pool, cid, vnum).await?;
    Ok(Json(version))
}

/// `PATCH /component-templates/{cid}/versions/{vnum}` — edit a version in place.
#[utoipa::path(
    patch,
    path = "/api/v1/component-templates/{cid}/versions/{vnum}",
    params(
        ("cid" = Uuid, Path, description = "Component id"),
        ("vnum" = i32, Path, description = "Version number")
    ),
    request_body = VersionUpdate,
    responses(
        (status = 200, description = "Updated", body = ComponentTemplateVersionRead),
        (status = 404, description = "Not found", body = crate::error::ErrorEnvelope),
        (status = 422, description = "Validation error", body = crate::error::ErrorEnvelope)
    ),
    tag = "component_templates"
)]
pub async fn update_version(
    State(state): State<AppState>,
    Path((cid, vnum)): Path<(Uuid, i32)>,
    Json(input): Json<VersionUpdate>,
) -> AppResult<Json<ComponentTemplateVersionRead>> {
    let version = component_template_service::update_version(&state.pool, cid, vnum, input).await?;
    Ok(Json(version))
}

/// `DELETE /component-templates/{cid}/versions/{vnum}` — delete a version.
#[utoipa::path(
    delete,
    path = "/api/v1/component-templates/{cid}/versions/{vnum}",
    params(
        ("cid" = Uuid, Path, description = "Component id"),
        ("vnum" = i32, Path, description = "Version number")
    ),
    responses(
        (status = 204, description = "Deleted"),
        (status = 404, description = "Not found", body = crate::error::ErrorEnvelope),
        (status = 409, description = "Last version protected", body = crate::error::ErrorEnvelope)
    ),
    tag = "component_templates"
)]
pub async fn delete_version(
    State(state): State<AppState>,
    Path((cid, vnum)): Path<(Uuid, i32)>,
) -> AppResult<impl IntoResponse> {
    component_template_service::delete_version(&state.pool, cid, vnum).await?;
    Ok(StatusCode::NO_CONTENT)
}

/// `POST /component-templates/{cid}/versions/{vnum}/make-default` — pin default.
#[utoipa::path(
    post,
    path = "/api/v1/component-templates/{cid}/versions/{vnum}/make-default",
    params(
        ("cid" = Uuid, Path, description = "Component id"),
        ("vnum" = i32, Path, description = "Version number")
    ),
    responses(
        (status = 200, description = "Default updated", body = ComponentTemplateRead),
        (status = 404, description = "Not found", body = crate::error::ErrorEnvelope)
    ),
    tag = "component_templates"
)]
pub async fn make_default(
    State(state): State<AppState>,
    Path((cid, vnum)): Path<(Uuid, i32)>,
) -> AppResult<Json<ComponentTemplateRead>> {
    let component = component_template_service::make_default(&state.pool, cid, vnum).await?;
    Ok(Json(component))
}

/// `GET /component-templates/{cid}/resolve?version=default|N` — proxy-facing
/// resolve. `Version(n)` falls back to the current default when missing.
#[utoipa::path(
    get,
    path = "/api/v1/component-templates/{cid}/resolve",
    params(
        ("cid" = Uuid, Path, description = "Component id"),
        ("version" = Option<String>, Query, description = "default | N")
    ),
    responses(
        (status = 200, description = "Resolved component", body = ResolvedComponentRead),
        (status = 404, description = "Not found", body = crate::error::ErrorEnvelope),
        (status = 422, description = "Bad version selector", body = crate::error::ErrorEnvelope)
    ),
    tag = "component_templates"
)]
pub async fn resolve(
    State(state): State<AppState>,
    Path(cid): Path<Uuid>,
    Query(query): Query<ResolveQuery>,
) -> AppResult<Json<ResolvedComponentRead>> {
    let selector = VersionSelector::parse(query.version.as_deref())?;
    let resolved = component_template_service::resolve(&state.pool, cid, selector).await?;
    Ok(Json(resolved))
}
