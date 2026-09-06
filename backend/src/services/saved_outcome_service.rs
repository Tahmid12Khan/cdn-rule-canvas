//! Outcomes Library business logic. `resolve` renders the referenced
//! component version's `html_body` against this saved outcome's own
//! `variables` (flat mustache, matching the proxy's `component_render`
//! semantics) and returns the ALREADY-RENDERED HTML — the proxy applier
//! injects it as-is, with no client-side render step.

use std::collections::HashMap;

use sqlx::PgPool;
use uuid::Uuid;
use validator::Validate;

use crate::{
    error::{AppError, AppResult, ValidationDetail},
    repositories::{component_template_repository, saved_outcome_repository as repo},
    schemas::{
        pagination::{Page, PageParams},
        saved_outcome::{
            self, ResolvedSavedOutcomeRead, SavedOutcomeCreate, SavedOutcomeRead,
            SavedOutcomeUpdate,
        },
    },
    services::component_template_service::{self, VersionSelector},
};

const PG_UNIQUE_VIOLATION: &str = "23505";
const PG_FOREIGN_KEY_VIOLATION: &str = "23503";

fn variables_to_json(vars: &HashMap<String, String>) -> serde_json::Value {
    serde_json::json!(vars)
}

/// Assert `component_id` exists in `rre.component_templates`. Mirrors
/// `outcome_service::ensure_component_ref_exists` (422 `component_ref_exists`),
/// run BEFORE the insert so a bad reference is a validation error, not the
/// FK-violation 409 reserved for the delete-time "still referenced" conflict.
async fn ensure_component_exists(pool: &PgPool, component_id: Uuid) -> AppResult<()> {
    if component_template_repository::find(pool, component_id)
        .await?
        .is_none()
    {
        return Err(AppError::validation(vec![ValidationDetail::new(
            "component_id",
            format!("library component {component_id} does not exist"),
            "component_ref_exists",
        )]));
    }
    Ok(())
}

pub async fn create(pool: &PgPool, input: SavedOutcomeCreate) -> AppResult<SavedOutcomeRead> {
    input.validate().map_err(AppError::from)?;
    ensure_component_exists(pool, input.component_id).await?;
    let variables_json = variables_to_json(&input.variables);
    let row = repo::insert(
        pool,
        &input.slug,
        &input.name,
        input.component_id,
        input.version_number,
        &variables_json,
    )
    .await
    .map_err(map_write_error)?;
    to_read(pool, row).await
}

pub async fn list(
    pool: &PgPool,
    params: &PageParams,
    q: Option<&str>,
) -> AppResult<Page<SavedOutcomeRead>> {
    let (limit, offset, page, page_size) = params.resolve();
    let q = q.filter(|s| !s.is_empty());
    let (rows, total) = tokio::try_join!(
        repo::list_paged(pool, limit, offset, q),
        repo::count(pool, q)
    )?;
    let mut items = Vec::with_capacity(rows.len());
    for row in rows {
        items.push(to_read(pool, row).await?);
    }
    Ok(Page::new(items, page, page_size, total))
}

pub async fn get(pool: &PgPool, id: Uuid) -> AppResult<SavedOutcomeRead> {
    let row = repo::get(pool, id).await?.ok_or_else(|| not_found(id))?;
    to_read(pool, row).await
}

pub async fn update(
    pool: &PgPool,
    id: Uuid,
    input: SavedOutcomeUpdate,
) -> AppResult<SavedOutcomeRead> {
    input.validate().map_err(AppError::from)?;
    if input.is_empty() {
        return get(pool, id).await;
    }
    let (set_version, version_number) = match input.version_number {
        None => (false, None),
        Some(inner) => (true, inner),
    };
    let variables_json = input.variables.as_ref().map(variables_to_json);
    let row = repo::update(
        pool,
        id,
        input.name.as_deref(),
        set_version,
        version_number,
        variables_json.as_ref(),
    )
    .await
    .map_err(map_write_error)?
    .ok_or_else(|| not_found(id))?;
    to_read(pool, row).await
}

pub async fn delete(pool: &PgPool, id: Uuid) -> AppResult<()> {
    let deleted = repo::delete(pool, id).await?;
    if deleted == 0 {
        return Err(not_found(id));
    }
    Ok(())
}

/// `GET /saved-outcomes/{id}/resolve` — the proxy-facing route. Resolves the
/// referenced component version (reusing `component_template_service`'s own
/// resolve, `version=default` when `version_number` is `None`) and renders its
/// `html_body` against this saved outcome's `variables` with `mustache`
/// (same flat-interpolation semantics as `proxy/src/domain/applier/component_render.rs`).
pub async fn resolve(pool: &PgPool, id: Uuid) -> AppResult<ResolvedSavedOutcomeRead> {
    let row = repo::get(pool, id).await?.ok_or_else(|| not_found(id))?;
    let selector = match row.version_number {
        Some(n) => VersionSelector::Version(n),
        None => VersionSelector::Default,
    };
    let resolved = component_template_service::resolve(pool, row.component_id, selector).await?;

    let mut builder = mustache::MapBuilder::new();
    if let Some(vars) = row.variables.as_object() {
        for (name, value) in vars {
            let s = value.as_str().unwrap_or_default().to_string();
            builder = builder.insert_str(name.clone(), s);
        }
    }
    let data = builder.build();
    let template = mustache::compile_str(&resolved.html_body)
        .map_err(|e| AppError::Internal(anyhow::anyhow!(e)))?;
    let html_body = template
        .render_data_to_string(&data)
        .map_err(|e| AppError::Internal(anyhow::anyhow!(e)))?;

    Ok(ResolvedSavedOutcomeRead { html_body })
}

async fn to_read(
    pool: &PgPool,
    row: crate::models::saved_outcome::SavedOutcome,
) -> AppResult<SavedOutcomeRead> {
    let component = component_template_repository::find(pool, row.component_id)
        .await?
        .ok_or_else(|| {
            AppError::ComponentNotFound(format!("Component '{}' not found", row.component_id))
        })?;
    Ok(saved_outcome::to_read(row, component.name))
}

fn not_found(id: Uuid) -> AppError {
    AppError::SavedOutcomeNotFound(format!("Saved outcome '{id}' not found"))
}

fn map_write_error(err: sqlx::Error) -> AppError {
    if let sqlx::Error::Database(db_err) = &err {
        match db_err.code().as_deref() {
            Some(PG_UNIQUE_VIOLATION) => {
                return AppError::SlugConflict(
                    "A saved outcome with this slug or name already exists".to_string(),
                );
            }
            Some(PG_FOREIGN_KEY_VIOLATION) => {
                return AppError::ComponentInUse(
                    "Referenced component is missing or in use and cannot be changed".to_string(),
                );
            }
            _ => {}
        }
    }
    err.into()
}
