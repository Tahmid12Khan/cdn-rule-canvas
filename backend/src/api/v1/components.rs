//! Per-component HTTP handlers (BACKEND CONTRACT §7).
//!
//! Component *add* / *list* / *reorder* live on the outcome sub-router
//! (`/outcomes/{oid}/...`); per-component *edit* and *delete* are mounted here
//! under `/api/v1`:
//! - `PATCH  /components/{cid}`  update a component
//! - `DELETE /components/{cid}`  delete a component
//!
//! Handlers stay thin: validate the DTO, delegate to `outcome_service`, map the
//! domain result to the HTTP envelope.

use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    routing::patch as patch_route,
    Json, Router,
};
use uuid::Uuid;
use validator::Validate;

use crate::error::{AppError, AppResult, ValidationDetail};
use crate::schemas::component::{ComponentRead, ComponentUpdate};
use crate::services::outcome_service;
use crate::state::AppState;

/// Build the per-component sub-router (`/components/{cid}`).
pub fn router() -> Router<AppState> {
    Router::new().route("/components/:cid", patch_route(update).delete(delete))
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

/// `PATCH /components/{cid}` — update a component (version must be DRAFT).
#[utoipa::path(
    patch,
    path = "/api/v1/components/{cid}",
    params(("cid" = Uuid, Path, description = "Component id")),
    request_body = ComponentUpdate,
    responses(
        (status = 200, description = "Component updated", body = ComponentRead),
        (status = 404, description = "Component not found"),
        (status = 409, description = "Version not editable"),
        (status = 422, description = "Validation error")
    ),
    tag = "components"
)]
pub async fn update(
    State(state): State<AppState>,
    Path(cid): Path<Uuid>,
    Json(body): Json<ComponentUpdate>,
) -> AppResult<Json<ComponentRead>> {
    body.validate().map_err(map_validation)?;
    let component = outcome_service::update_component(&state.pool, cid, body).await?;
    Ok(Json(component))
}

/// `DELETE /components/{cid}` — delete a component (version must be DRAFT).
#[utoipa::path(
    delete,
    path = "/api/v1/components/{cid}",
    params(("cid" = Uuid, Path, description = "Component id")),
    responses(
        (status = 204, description = "Component deleted"),
        (status = 404, description = "Component not found"),
        (status = 409, description = "Version not editable")
    ),
    tag = "components"
)]
pub async fn delete(
    State(state): State<AppState>,
    Path(cid): Path<Uuid>,
) -> AppResult<impl IntoResponse> {
    outcome_service::delete_component(&state.pool, cid).await?;
    Ok(StatusCode::NO_CONTENT)
}
