//! Outcome + nested-component HTTP handlers (BACKEND CONTRACT §7).
//!
//! Routes mounted under `/api/v1`:
//! - `GET    /versions/{vid}/outcomes`     list outcomes for a version
//! - `POST   /versions/{vid}/outcomes`     create an outcome
//! - `GET    /outcomes/{oid}`              get an outcome (with components)
//! - `PATCH  /outcomes/{oid}`              update an outcome
//! - `DELETE /outcomes/{oid}`              delete an outcome
//! - `POST   /outcomes/{oid}/clone`        deep-clone an outcome
//! - `POST   /outcomes/{oid}/reorder`      reorder an outcome's components
//! - `POST   /outcomes/{oid}/components`   add a component to an outcome
//!
//! Handlers stay thin: validate the DTO, delegate to `outcome_service`, map the
//! domain result to the HTTP envelope. No business logic here.

use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{get as get_route, post as post_route},
    Json, Router,
};
use uuid::Uuid;
use validator::Validate;

use crate::error::{AppError, AppResult, ValidationDetail};
use crate::schemas::component::{ComponentCreate, ComponentRead};
use crate::schemas::outcome::{OutcomeCreate, OutcomeRead, OutcomeUpdate, ReorderItem};
use crate::services::outcome_service;
use crate::state::AppState;

/// Build the outcome + nested-component sub-router.
pub fn router() -> Router<AppState> {
    Router::new()
        .route(
            "/versions/:vid/outcomes",
            get_route(list_for_version).post(create),
        )
        .route(
            "/outcomes/:oid",
            get_route(get).patch(update).delete(delete),
        )
        .route("/outcomes/:oid/clone", post_route(clone))
        .route("/outcomes/:oid/reorder", post_route(reorder_components))
        .route("/outcomes/:oid/components", post_route(add_component))
}

/// Map a `validator::ValidationErrors` into the uniform 422 envelope.
fn map_validation(errs: validator::ValidationErrors) -> AppError {
    let details = errs
        .field_errors()
        .into_iter()
        .flat_map(|(field, errors)| {
            errors.iter().map(move |e| {
                let msg = e
                    .message
                    .as_ref()
                    .map(|m| m.to_string())
                    .unwrap_or_else(|| e.code.to_string());
                ValidationDetail::new(field.to_string(), msg, e.code.to_string())
            })
        })
        .collect();
    AppError::validation(details)
}

/// `GET /versions/{vid}/outcomes` — list outcomes (components nested, ordered).
#[utoipa::path(
    get,
    path = "/api/v1/versions/{vid}/outcomes",
    params(("vid" = Uuid, Path, description = "Version id")),
    responses(
        (status = 200, description = "Outcomes for the version", body = [OutcomeRead]),
        (status = 404, description = "Version not found")
    ),
    tag = "outcomes"
)]
pub async fn list_for_version(
    State(state): State<AppState>,
    Path(vid): Path<Uuid>,
) -> AppResult<Json<Vec<OutcomeRead>>> {
    let outcomes = outcome_service::list(&state.pool, vid).await?;
    Ok(Json(outcomes))
}

/// `POST /versions/{vid}/outcomes` — create an outcome.
#[utoipa::path(
    post,
    path = "/api/v1/versions/{vid}/outcomes",
    params(("vid" = Uuid, Path, description = "Version id")),
    request_body = OutcomeCreate,
    responses(
        (status = 201, description = "Outcome created", body = OutcomeRead),
        (status = 404, description = "Version not found"),
        (status = 409, description = "Version not editable"),
        (status = 422, description = "Validation error")
    ),
    tag = "outcomes"
)]
pub async fn create(
    State(state): State<AppState>,
    Path(vid): Path<Uuid>,
    Json(body): Json<OutcomeCreate>,
) -> AppResult<impl IntoResponse> {
    body.validate().map_err(map_validation)?;
    let outcome = outcome_service::create(&state.pool, vid, body).await?;
    Ok((StatusCode::CREATED, Json(outcome)))
}

/// `GET /outcomes/{oid}` — get one outcome with its components.
#[utoipa::path(
    get,
    path = "/api/v1/outcomes/{oid}",
    params(("oid" = Uuid, Path, description = "Outcome id")),
    responses(
        (status = 200, description = "Outcome", body = OutcomeRead),
        (status = 404, description = "Outcome not found")
    ),
    tag = "outcomes"
)]
pub async fn get(
    State(state): State<AppState>,
    Path(oid): Path<Uuid>,
) -> AppResult<Json<OutcomeRead>> {
    let outcome = outcome_service::get_with_components(&state.pool, oid).await?;
    Ok(Json(outcome))
}

/// `PATCH /outcomes/{oid}` — update an outcome.
#[utoipa::path(
    patch,
    path = "/api/v1/outcomes/{oid}",
    params(("oid" = Uuid, Path, description = "Outcome id")),
    request_body = OutcomeUpdate,
    responses(
        (status = 200, description = "Outcome updated", body = OutcomeRead),
        (status = 404, description = "Outcome not found"),
        (status = 409, description = "Version not editable"),
        (status = 422, description = "Validation error")
    ),
    tag = "outcomes"
)]
pub async fn update(
    State(state): State<AppState>,
    Path(oid): Path<Uuid>,
    Json(body): Json<OutcomeUpdate>,
) -> AppResult<Json<OutcomeRead>> {
    body.validate().map_err(map_validation)?;
    let outcome = outcome_service::update(&state.pool, oid, body).await?;
    Ok(Json(outcome))
}

/// `DELETE /outcomes/{oid}` — delete an outcome.
#[utoipa::path(
    delete,
    path = "/api/v1/outcomes/{oid}",
    params(("oid" = Uuid, Path, description = "Outcome id")),
    responses(
        (status = 204, description = "Outcome deleted"),
        (status = 404, description = "Outcome not found"),
        (status = 409, description = "Builtin protected or version not editable")
    ),
    tag = "outcomes"
)]
pub async fn delete(
    State(state): State<AppState>,
    Path(oid): Path<Uuid>,
) -> AppResult<impl IntoResponse> {
    outcome_service::delete(&state.pool, oid).await?;
    Ok(StatusCode::NO_CONTENT)
}

/// `POST /outcomes/{oid}/clone` — deep-clone an outcome.
#[utoipa::path(
    post,
    path = "/api/v1/outcomes/{oid}/clone",
    params(("oid" = Uuid, Path, description = "Source outcome id")),
    responses(
        (status = 201, description = "Outcome cloned", body = OutcomeRead),
        (status = 404, description = "Outcome not found"),
        (status = 409, description = "Version not editable")
    ),
    tag = "outcomes"
)]
pub async fn clone(
    State(state): State<AppState>,
    Path(oid): Path<Uuid>,
) -> AppResult<impl IntoResponse> {
    let outcome = outcome_service::clone_outcome(&state.pool, oid).await?;
    Ok((StatusCode::CREATED, Json(outcome)))
}

/// `POST /outcomes/{oid}/reorder` — reorder an outcome's components.
#[utoipa::path(
    post,
    path = "/api/v1/outcomes/{oid}/reorder",
    params(("oid" = Uuid, Path, description = "Outcome id")),
    request_body = [ReorderItem],
    responses(
        (status = 200, description = "Components reordered", body = [ComponentRead]),
        (status = 404, description = "Outcome not found"),
        (status = 409, description = "Version not editable"),
        (status = 422, description = "Validation error")
    ),
    tag = "outcomes"
)]
pub async fn reorder_components(
    State(state): State<AppState>,
    Path(oid): Path<Uuid>,
    Json(items): Json<Vec<ReorderItem>>,
) -> AppResult<Json<Vec<ComponentRead>>> {
    let components = outcome_service::reorder(&state.pool, oid, items).await?;
    Ok(Json(components))
}

/// `POST /outcomes/{oid}/components` — add a component to an outcome.
#[utoipa::path(
    post,
    path = "/api/v1/outcomes/{oid}/components",
    params(("oid" = Uuid, Path, description = "Outcome id")),
    request_body = ComponentCreate,
    responses(
        (status = 201, description = "Component created", body = ComponentRead),
        (status = 404, description = "Outcome not found"),
        (status = 409, description = "Version not editable"),
        (status = 422, description = "Validation error")
    ),
    tag = "outcomes"
)]
pub async fn add_component(
    State(state): State<AppState>,
    Path(oid): Path<Uuid>,
    Json(body): Json<ComponentCreate>,
) -> AppResult<impl IntoResponse> {
    body.validate().map_err(map_validation)?;
    let component = outcome_service::add_component(&state.pool, oid, body).await?;
    Ok((StatusCode::CREATED, Json(component)))
}
