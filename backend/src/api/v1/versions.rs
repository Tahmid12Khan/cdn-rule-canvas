//! Version HTTP handlers (BACKEND CONTRACT §7).
//!
//! Routers map domain results to HTTP; all business logic lives in
//! [`version_service`]. No DB access here.

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{get as get_route, post as post_route},
    Json, Router,
};
use validator::Validate;

use crate::{
    error::{AppError, AppResult},
    schemas::{
        pagination::Page,
        version::{
            PublishRequest, VersionCreate, VersionListQuery, VersionRead, VersionSummary,
            VersionUpdate,
        },
    },
    services::version_service,
    state::AppState,
};

/// All version routes, nested under a feature.
pub fn router() -> Router<AppState> {
    Router::new()
        .route("/features/:fid/versions", post_route(create).get(list))
        .route(
            "/features/:fid/versions/:vnum",
            get_route(get).patch(update).delete(delete),
        )
        .route("/features/:fid/versions/:vnum/publish", post_route(publish))
        .route(
            "/features/:fid/versions/:vnum/unpublish",
            post_route(unpublish),
        )
}

/// `POST /features/{fid}/versions` — create a draft version.
#[utoipa::path(
    post,
    path = "/api/v1/features/{fid}/versions",
    params(("fid" = String, Path, description = "Feature slug")),
    request_body = VersionCreate,
    responses(
        (status = 201, description = "Created", body = VersionRead),
        (status = 404, description = "Feature not found"),
        (status = 422, description = "Validation error")
    ),
    tag = "versions"
)]
pub async fn create(
    State(state): State<AppState>,
    Path(fid): Path<String>,
    Json(body): Json<VersionCreate>,
) -> AppResult<impl IntoResponse> {
    validate(&body)?;
    let version =
        version_service::create_version(&state.pool, &fid, body, &state.node_manifest).await?;
    Ok((StatusCode::CREATED, Json(version)))
}

/// `GET /features/{fid}/versions` — list versions (filtered, paginated).
#[utoipa::path(
    get,
    path = "/api/v1/features/{fid}/versions",
    params(
        ("fid" = String, Path, description = "Feature slug"),
        VersionListQuery
    ),
    responses(
        (status = 200, description = "Version page", body = PageVersionSummary),
        (status = 404, description = "Feature not found")
    ),
    tag = "versions"
)]
pub async fn list(
    State(state): State<AppState>,
    Path(fid): Path<String>,
    Query(query): Query<VersionListQuery>,
) -> AppResult<Json<Page<VersionSummary>>> {
    let page = version_service::list(&state.pool, &fid, query).await?;
    Ok(Json(page))
}

/// `GET /features/{fid}/versions/{vnum}` — fetch a single version.
#[utoipa::path(
    get,
    path = "/api/v1/features/{fid}/versions/{vnum}",
    params(
        ("fid" = String, Path, description = "Feature slug"),
        ("vnum" = i32, Path, description = "Version number")
    ),
    responses(
        (status = 200, description = "Version", body = VersionRead),
        (status = 404, description = "Version not found")
    ),
    tag = "versions"
)]
pub async fn get(
    State(state): State<AppState>,
    Path((fid, vnum)): Path<(String, i32)>,
) -> AppResult<Json<VersionRead>> {
    let version = version_service::get(&state.pool, &fid, vnum).await?;
    Ok(Json(version))
}

/// `PATCH /features/{fid}/versions/{vnum}` — update description / rule_graph.
#[utoipa::path(
    patch,
    path = "/api/v1/features/{fid}/versions/{vnum}",
    params(
        ("fid" = String, Path, description = "Feature slug"),
        ("vnum" = i32, Path, description = "Version number")
    ),
    request_body = VersionUpdate,
    responses(
        (status = 200, description = "Updated", body = VersionRead),
        (status = 404, description = "Version not found"),
        (status = 409, description = "Version edit locked"),
        (status = 422, description = "Validation error")
    ),
    tag = "versions"
)]
pub async fn update(
    State(state): State<AppState>,
    Path((fid, vnum)): Path<(String, i32)>,
    Json(body): Json<VersionUpdate>,
) -> AppResult<Json<VersionRead>> {
    validate(&body)?;
    let version =
        version_service::update(&state.pool, &fid, vnum, body, &state.node_manifest).await?;
    Ok(Json(version))
}

/// `POST /features/{fid}/versions/{vnum}/publish` — publish to staging/live.
#[utoipa::path(
    post,
    path = "/api/v1/features/{fid}/versions/{vnum}/publish",
    params(
        ("fid" = String, Path, description = "Feature slug"),
        ("vnum" = i32, Path, description = "Version number")
    ),
    request_body = PublishRequest,
    responses(
        (status = 200, description = "Published", body = VersionRead),
        (status = 404, description = "Version not found"),
        (status = 409, description = "Invalid status transition")
    ),
    tag = "versions"
)]
pub async fn publish(
    State(state): State<AppState>,
    Path((fid, vnum)): Path<(String, i32)>,
    Json(body): Json<PublishRequest>,
) -> AppResult<Json<VersionRead>> {
    let version = version_service::publish(&state.pool, &fid, vnum, body.environment).await?;
    Ok(Json(version))
}

/// `POST /features/{fid}/versions/{vnum}/unpublish` — unpublish from staging/live.
#[utoipa::path(
    post,
    path = "/api/v1/features/{fid}/versions/{vnum}/unpublish",
    params(
        ("fid" = String, Path, description = "Feature slug"),
        ("vnum" = i32, Path, description = "Version number")
    ),
    request_body = PublishRequest,
    responses(
        (status = 200, description = "Unpublished", body = VersionRead),
        (status = 404, description = "Version not found"),
        (status = 409, description = "Invalid status transition")
    ),
    tag = "versions"
)]
pub async fn unpublish(
    State(state): State<AppState>,
    Path((fid, vnum)): Path<(String, i32)>,
    Json(body): Json<PublishRequest>,
) -> AppResult<Json<VersionRead>> {
    let version = version_service::unpublish(&state.pool, &fid, vnum, body.environment).await?;
    Ok(Json(version))
}

/// `DELETE /features/{fid}/versions/{vnum}` — delete a DRAFT/PREV version.
#[utoipa::path(
    delete,
    path = "/api/v1/features/{fid}/versions/{vnum}",
    params(
        ("fid" = String, Path, description = "Feature slug"),
        ("vnum" = i32, Path, description = "Version number")
    ),
    responses(
        (status = 204, description = "Deleted"),
        (status = 404, description = "Version not found"),
        (status = 409, description = "Invalid status transition")
    ),
    tag = "versions"
)]
pub async fn delete(
    State(state): State<AppState>,
    Path((fid, vnum)): Path<(String, i32)>,
) -> AppResult<StatusCode> {
    version_service::delete(&state.pool, &fid, vnum).await?;
    Ok(StatusCode::NO_CONTENT)
}

/// Run `validator` checks and convert failures into the uniform 422 envelope.
fn validate<T: Validate>(body: &T) -> AppResult<()> {
    body.validate().map_err(|errors| {
        let details = errors
            .field_errors()
            .into_iter()
            .flat_map(|(field, errs)| {
                errs.iter().map(move |e| {
                    crate::error::ValidationDetail::new(
                        field.to_string(),
                        e.message
                            .as_ref()
                            .map(|m| m.to_string())
                            .unwrap_or_else(|| e.code.to_string()),
                        e.code.to_string(),
                    )
                })
            })
            .collect::<Vec<_>>();
        AppError::validation(details)
    })
}
