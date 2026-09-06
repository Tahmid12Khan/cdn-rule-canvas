//! Component-template business logic (Component Editor design §3.4).
//!
//! Validates request DTOs, maps the pg unique-violation (`23505`) on the slug to
//! [`AppError::SlugConflict`], and translates "row absent" into
//! [`AppError::ComponentNotFound`] / [`AppError::ComponentVersionNotFound`].
//! Multi-statement writes (create / create_version / delete_version) run inside
//! a `pool.begin()` transaction; the deferrable default-version FK lets us INSERT
//! the component then its v1 then set the pointer in one unit of work. Read paths
//! use `&PgPool`.
//!
//! `resolve(component_id, selector)` is the proxy-facing read: `Default`
//! resolves per `default_mode`; `Version(n)` resolves to that version, FALLING
//! BACK to the current default when `n` is missing (the proxy never fails closed
//! on version drift).

use sqlx::PgPool;
use uuid::Uuid;
use validator::Validate;

use crate::{
    error::{AppError, AppResult},
    models::{
        component_template::{ComponentTemplate, ComponentTemplateVersion},
        enums::DefaultMode,
    },
    repositories::{
        component_template_repository as comp_repo,
        component_template_version_repository as ver_repo,
    },
    schemas::{
        component_template::{
            build_read, resolve_default_id, ComponentTemplateCreate, ComponentTemplateRead,
            ComponentTemplateSummary, ComponentTemplateUpdate, ComponentTemplateVersionRead,
            ComponentVariable, ResolvedComponentRead, VersionCreate, VersionUpdate,
        },
        pagination::{Page, PageParams},
    },
};

/// Postgres unique-violation SQLSTATE.
const PG_UNIQUE_VIOLATION: &str = "23505";

/// Which version a resolve request targets.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VersionSelector {
    /// The component's current default (resolved per `default_mode`).
    Default,
    /// A specific `version_number` (falls back to default when missing).
    Version(i32),
}

impl VersionSelector {
    /// Parse the `?version=default|N` query param. `default` (or absent / empty)
    /// → [`VersionSelector::Default`]; a positive integer → `Version(n)`.
    /// Anything else is rejected as a validation error.
    pub fn parse(raw: Option<&str>) -> AppResult<Self> {
        match raw.map(str::trim) {
            None | Some("") | Some("default") => Ok(VersionSelector::Default),
            Some(s) => s
                .parse::<i32>()
                .ok()
                .filter(|n| *n > 0)
                .map(VersionSelector::Version)
                .ok_or_else(|| {
                    AppError::validation(vec![crate::error::ValidationDetail::new(
                        "version",
                        "version must be 'default' or a positive integer",
                        "version_selector_invalid",
                    )])
                }),
        }
    }
}

/// Create a component + its v1, then wire the default pointer (`latest`). Dup
/// slug → 409 `SLUG_CONFLICT`.
pub async fn create(
    pool: &PgPool,
    input: ComponentTemplateCreate,
) -> AppResult<ComponentTemplateRead> {
    input.validate().map_err(AppError::from)?;

    let variables_json = variables_to_json(input.variables.as_deref())?;
    let html_body = input.html_body.unwrap_or_default();

    let mut tx = pool.begin().await?;

    let component = match comp_repo::insert(
        &mut *tx,
        Uuid::new_v4(),
        &input.slug,
        &input.name,
        input.description.as_deref(),
    )
    .await
    {
        Ok(c) => c,
        Err(err) => return Err(map_conflict_error(err)),
    };

    let v1 = ver_repo::insert(
        &mut *tx,
        Uuid::new_v4(),
        component.id,
        1,
        None,
        &html_body,
        &variables_json,
    )
    .await?;

    // Default tracks latest; pin the pointer to v1 so resolution is unambiguous
    // even though `latest` would self-heal.
    let component =
        comp_repo::set_default(&mut *tx, component.id, DefaultMode::Latest, Some(v1.id)).await?;

    tx.commit().await?;

    Ok(build_read(component, vec![v1]))
}

/// List components, newest first, paginated. Optional `q` filters by name.
pub async fn list(
    pool: &PgPool,
    params: &PageParams,
    q: Option<&str>,
) -> AppResult<Page<ComponentTemplateSummary>> {
    let (limit, offset, page, page_size) = params.resolve();
    let q = q.filter(|s| !s.is_empty());

    let (rows, total) = tokio::try_join!(
        comp_repo::list_paged(pool, limit, offset, q),
        comp_repo::count(pool, q)
    )?;

    // Per-component version summary (number + count). One extra round-trip per
    // row — the library is small (no hot path); keep it simple.
    let mut items = Vec::with_capacity(rows.len());
    for component in rows {
        let versions = ver_repo::list_for_component(pool, component.id).await?;
        items.push(summary_of(&component, &versions));
    }

    Ok(Page::new(items, page, page_size, total))
}

/// Fetch a component + all its version summaries. Absent → 404.
pub async fn get_with_versions(pool: &PgPool, cid: Uuid) -> AppResult<ComponentTemplateRead> {
    let component = comp_repo::find(pool, cid)
        .await?
        .ok_or_else(|| not_found(cid))?;
    let versions = ver_repo::list_for_component(pool, cid).await?;
    Ok(build_read(component, versions))
}

/// Fetch a component (by slug) + all its version summaries. Absent → 404.
pub async fn get_with_versions_by_slug(
    pool: &PgPool,
    slug: &str,
) -> AppResult<ComponentTemplateRead> {
    let component = comp_repo::find_by_slug(pool, slug)
        .await?
        .ok_or_else(|| not_found_slug(slug))?;
    let versions = ver_repo::list_for_component(pool, component.id).await?;
    Ok(build_read(component, versions))
}

/// List a component's full version reads (body + variables + default flag).
pub async fn list_versions(
    pool: &PgPool,
    cid: Uuid,
) -> AppResult<Vec<ComponentTemplateVersionRead>> {
    let component = comp_repo::find(pool, cid)
        .await?
        .ok_or_else(|| not_found(cid))?;
    let versions = ver_repo::list_for_component(pool, cid).await?;
    let default_id = resolve_default_id(&component, &versions);
    Ok(versions
        .into_iter()
        .map(|v| {
            let is_default = Some(v.id) == default_id;
            ComponentTemplateVersionRead::from_row(v, is_default)
        })
        .collect())
}

/// Fetch one version (by `version_number`) with its default flag. Absent
/// component → 404; absent version → 404 `COMPONENT_VERSION_NOT_FOUND`.
pub async fn get_version(
    pool: &PgPool,
    cid: Uuid,
    vnum: i32,
) -> AppResult<ComponentTemplateVersionRead> {
    let component = comp_repo::find(pool, cid)
        .await?
        .ok_or_else(|| not_found(cid))?;
    let versions = ver_repo::list_for_component(pool, cid).await?;
    let default_id = resolve_default_id(&component, &versions);
    let version = versions
        .into_iter()
        .find(|v| v.version_number == vnum)
        .ok_or_else(|| version_not_found(cid, vnum))?;
    let is_default = Some(version.id) == default_id;
    Ok(ComponentTemplateVersionRead::from_row(version, is_default))
}

/// Update metadata + the movable default pointer. Switching to `pinned`
/// validates `default_version_number`; switching to `latest` clears the pin.
pub async fn update(
    pool: &PgPool,
    cid: Uuid,
    input: ComponentTemplateUpdate,
) -> AppResult<ComponentTemplateRead> {
    input.validate().map_err(AppError::from)?;

    if input.is_empty() {
        return get_with_versions(pool, cid).await;
    }

    let mut tx = pool.begin().await?;

    let mut component = comp_repo::find(&mut *tx, cid)
        .await?
        .ok_or_else(|| not_found(cid))?;

    // Metadata.
    if input.name.is_some() || input.description.is_some() {
        let description_touch = input.description.as_deref().map(Some);
        component =
            match comp_repo::update_meta(&mut *tx, cid, input.name.as_deref(), description_touch)
                .await
            {
                Ok(Some(c)) => c,
                Ok(None) => return Err(not_found(cid)),
                Err(err) => return Err(map_conflict_error(err)),
            };
    }

    // Default pointer.
    if let Some(mode) = input.default_mode {
        let versions = ver_repo::list_for_component(&mut *tx, cid).await?;
        let (mode, pointer) = match mode {
            DefaultMode::Pinned => {
                let vnum = input.default_version_number.ok_or_else(|| {
                    AppError::validation(vec![crate::error::ValidationDetail::new(
                        "default_version_number",
                        "default_version_number is required when default_mode is 'pinned'",
                        "default_version_required",
                    )])
                })?;
                let target = versions
                    .iter()
                    .find(|v| v.version_number == vnum)
                    .ok_or_else(|| version_not_found(cid, vnum))?;
                (DefaultMode::Pinned, Some(target.id))
            }
            // Switching to latest clears the pin (resolution becomes dynamic).
            DefaultMode::Latest => (DefaultMode::Latest, None),
        };
        component = comp_repo::set_default(&mut *tx, cid, mode, pointer).await?;
    } else if let Some(vnum) = input.default_version_number {
        // No mode change but a pin target supplied → pin to it.
        let versions = ver_repo::list_for_component(&mut *tx, cid).await?;
        let target = versions
            .iter()
            .find(|v| v.version_number == vnum)
            .ok_or_else(|| version_not_found(cid, vnum))?;
        component =
            comp_repo::set_default(&mut *tx, cid, DefaultMode::Pinned, Some(target.id)).await?;
    }

    let versions = ver_repo::list_for_component(&mut *tx, cid).await?;
    tx.commit().await?;
    Ok(build_read(component, versions))
}

/// Delete a component (cascades its versions). Absent → 404. Referenced by a
/// saved outcome (FK RESTRICT) → 409 `COMPONENT_IN_USE`.
pub async fn delete(pool: &PgPool, cid: Uuid) -> AppResult<()> {
    let deleted = comp_repo::delete(pool, cid)
        .await
        .map_err(map_delete_error)?;
    if deleted == 0 {
        return Err(not_found(cid));
    }
    Ok(())
}

/// Map a pg foreign-key violation (a saved outcome still references this
/// component) to a 409; else fall through to Internal.
fn map_delete_error(err: sqlx::Error) -> AppError {
    if let sqlx::Error::Database(db_err) = &err {
        if db_err.code().as_deref() == Some("23503") {
            return AppError::ComponentInUse(
                "Component is referenced by a saved outcome and cannot be deleted".to_string(),
            );
        }
    }
    err.into()
}

/// Create a new version: lock the component's versions, take `MAX + 1`, clone
/// the current default's body/variables unless supplied, then optionally
/// re-point the default. `make_default` (or `default_mode = latest`) advances the
/// pin to the new version.
pub async fn create_version(
    pool: &PgPool,
    cid: Uuid,
    input: VersionCreate,
) -> AppResult<ComponentTemplateVersionRead> {
    input.validate().map_err(AppError::from)?;

    let mut tx = pool.begin().await?;

    let component = comp_repo::find(&mut *tx, cid)
        .await?
        .ok_or_else(|| not_found(cid))?;

    let existing = ver_repo::list_for_component(&mut *tx, cid).await?;
    let next_number = ver_repo::max_version_number_for_update(&mut *tx, cid).await? + 1;

    // Clone the current default's payload unless the caller supplied one.
    let default_id = resolve_default_id(&component, &existing);
    let source = existing.iter().find(|v| Some(v.id) == default_id);

    let html_body = input
        .html_body
        .or_else(|| source.map(|s| s.html_body.clone()))
        .unwrap_or_default();
    let variables_json = match input.variables.as_deref() {
        Some(vars) => variables_to_json(Some(vars))?,
        None => source
            .map(|s| s.variables.clone())
            .unwrap_or_else(|| serde_json::json!([])),
    };

    let version = ver_repo::insert(
        &mut *tx,
        Uuid::new_v4(),
        cid,
        next_number,
        input.description.as_deref(),
        &html_body,
        &variables_json,
    )
    .await?;

    // Re-point the default: explicit make_default → pin; else `latest` advances
    // automatically (no pin change needed beyond keeping the pointer fresh).
    let component = if input.make_default {
        comp_repo::set_default(&mut *tx, cid, DefaultMode::Pinned, Some(version.id)).await?
    } else if component.default_mode == DefaultMode::Latest {
        // Keep the stored pointer current so resolve() works even on `latest`.
        comp_repo::set_default(&mut *tx, cid, DefaultMode::Latest, Some(version.id)).await?
    } else {
        component
    };

    // Recompute the default after the re-point to flag the new row correctly.
    let mut all = existing;
    all.push(version.clone());
    let default_id = resolve_default_id(&component, &all);
    let is_default = Some(version.id) == default_id;

    tx.commit().await?;
    Ok(ComponentTemplateVersionRead::from_row(version, is_default))
}

/// Edit a version in place (body/variables/description). Absent → 404.
pub async fn update_version(
    pool: &PgPool,
    cid: Uuid,
    vnum: i32,
    input: VersionUpdate,
) -> AppResult<ComponentTemplateVersionRead> {
    input.validate().map_err(AppError::from)?;

    let component = comp_repo::find(pool, cid)
        .await?
        .ok_or_else(|| not_found(cid))?;
    let target = ver_repo::find_by_number(pool, cid, vnum)
        .await?
        .ok_or_else(|| version_not_found(cid, vnum))?;

    if input.is_empty() {
        let versions = ver_repo::list_for_component(pool, cid).await?;
        let default_id = resolve_default_id(&component, &versions);
        return Ok(ComponentTemplateVersionRead::from_row(
            target.clone(),
            Some(target.id) == default_id,
        ));
    }

    let variables_json = match input.variables.as_deref() {
        Some(vars) => Some(variables_to_json(Some(vars))?),
        None => None,
    };
    let description_touch = input.description.as_deref().map(Some);

    let updated = ver_repo::update_fields(
        pool,
        target.id,
        description_touch,
        input.html_body.as_deref(),
        variables_json.as_ref(),
    )
    .await?
    .ok_or_else(|| version_not_found(cid, vnum))?;

    let versions = ver_repo::list_for_component(pool, cid).await?;
    let default_id = resolve_default_id(&component, &versions);
    let is_default = Some(updated.id) == default_id;
    Ok(ComponentTemplateVersionRead::from_row(updated, is_default))
}

/// Delete a version. Forbidden when it is the last (`LAST_VERSION_PROTECTED`).
/// When it was the current pinned default, re-point the default to the newest
/// remaining version (mode stays `pinned`; `latest` self-heals).
pub async fn delete_version(pool: &PgPool, cid: Uuid, vnum: i32) -> AppResult<()> {
    let mut tx = pool.begin().await?;

    let component = comp_repo::find(&mut *tx, cid)
        .await?
        .ok_or_else(|| not_found(cid))?;

    // Lock the versions for the duration.
    let _ = ver_repo::max_version_number_for_update(&mut *tx, cid).await?;
    let versions = ver_repo::list_for_component(&mut *tx, cid).await?;

    let target = versions
        .iter()
        .find(|v| v.version_number == vnum)
        .ok_or_else(|| version_not_found(cid, vnum))?;
    let target_id = target.id;

    if versions.len() <= 1 {
        return Err(AppError::LastVersionProtected(format!(
            "Component '{cid}' must keep at least one version"
        )));
    }

    // If the deleted version is the current pinned default, re-point to the
    // newest remaining version before deleting (avoids leaving the pin dangling).
    let default_id = resolve_default_id(&component, &versions);
    if component.default_mode == DefaultMode::Pinned && default_id == Some(target_id) {
        let newest_remaining = versions
            .iter()
            .filter(|v| v.id != target_id)
            .max_by_key(|v| v.version_number)
            .map(|v| v.id);
        comp_repo::set_default(&mut *tx, cid, DefaultMode::Pinned, newest_remaining).await?;
    }

    let deleted = ver_repo::delete(&mut *tx, target_id).await?;
    if deleted == 0 {
        return Err(version_not_found(cid, vnum));
    }

    tx.commit().await?;
    Ok(())
}

/// Make a specific version the default (`default_mode = pinned`,
/// `default_version_id = that version`). Absent → 404.
pub async fn make_default(pool: &PgPool, cid: Uuid, vnum: i32) -> AppResult<ComponentTemplateRead> {
    let mut tx = pool.begin().await?;

    let _component = comp_repo::find(&mut *tx, cid)
        .await?
        .ok_or_else(|| not_found(cid))?;
    let target = ver_repo::find_by_number(&mut *tx, cid, vnum)
        .await?
        .ok_or_else(|| version_not_found(cid, vnum))?;

    let component =
        comp_repo::set_default(&mut *tx, cid, DefaultMode::Pinned, Some(target.id)).await?;
    let versions = ver_repo::list_for_component(&mut *tx, cid).await?;

    tx.commit().await?;
    Ok(build_read(component, versions))
}

/// Resolve a component's effective version for the proxy. `Default` → per
/// `default_mode`; `Version(n)` → that version, FALLING BACK to the current
/// default when `n` is missing. Absent component → 404; unresolvable default →
/// 404 `COMPONENT_VERSION_NOT_FOUND`.
pub async fn resolve(
    pool: &PgPool,
    cid: Uuid,
    selector: VersionSelector,
) -> AppResult<ResolvedComponentRead> {
    let component = comp_repo::find(pool, cid)
        .await?
        .ok_or_else(|| not_found(cid))?;
    let versions = ver_repo::list_for_component(pool, cid).await?;

    let resolved: Option<ComponentTemplateVersion> = match selector {
        VersionSelector::Version(n) => {
            match versions.iter().find(|v| v.version_number == n).cloned() {
                Some(v) => Some(v),
                // Version drift: fall back to the current default.
                None => default_version(&component, &versions),
            }
        }
        VersionSelector::Default => default_version(&component, &versions),
    };

    let version = resolved.ok_or_else(|| {
        AppError::ComponentVersionNotFound(format!(
            "Component '{cid}' has no resolvable default version"
        ))
    })?;
    Ok(ResolvedComponentRead::from_row(version))
}

/// The component's current default version row (cloned), or `None`.
fn default_version(
    component: &ComponentTemplate,
    versions: &[ComponentTemplateVersion],
) -> Option<ComponentTemplateVersion> {
    let default_id = resolve_default_id(component, versions);
    versions.iter().find(|v| Some(v.id) == default_id).cloned()
}

/// Build a list-row summary from a component + its versions.
fn summary_of(
    component: &ComponentTemplate,
    versions: &[ComponentTemplateVersion],
) -> ComponentTemplateSummary {
    let latest_version_number = versions.iter().map(|v| v.version_number).max().unwrap_or(0);
    let default_id = resolve_default_id(component, versions);
    let default_version_number = versions
        .iter()
        .find(|v| Some(v.id) == default_id)
        .map(|v| v.version_number);
    ComponentTemplateSummary {
        id: component.id,
        slug: component.slug.clone(),
        name: component.name.clone(),
        description: component.description.clone(),
        default_version_number,
        latest_version_number,
        updated_at: component.updated_at,
    }
}

/// Serialize declared variables to the stored JSONB array (`[]` when absent).
fn variables_to_json(variables: Option<&[ComponentVariable]>) -> AppResult<serde_json::Value> {
    match variables {
        Some(vars) => {
            serde_json::to_value(vars).map_err(|e| AppError::Internal(anyhow::anyhow!(e)))
        }
        None => Ok(serde_json::json!([])),
    }
}

/// `COMPONENT_NOT_FOUND` for a component id.
fn not_found(cid: Uuid) -> AppError {
    AppError::ComponentNotFound(format!("Component '{cid}' not found"))
}

/// `COMPONENT_NOT_FOUND` for a component slug.
fn not_found_slug(slug: &str) -> AppError {
    AppError::ComponentNotFound(format!("Component '{slug}' not found"))
}

/// `COMPONENT_VERSION_NOT_FOUND` for a `(component, version_number)`.
fn version_not_found(cid: Uuid, vnum: i32) -> AppError {
    AppError::ComponentVersionNotFound(format!("Version {vnum} not found for component '{cid}'"))
}

/// Map a pg unique-violation on the slug to a 409; else fall through to Internal.
fn map_conflict_error(err: sqlx::Error) -> AppError {
    if let sqlx::Error::Database(db_err) = &err {
        if db_err.code().as_deref() == Some(PG_UNIQUE_VIOLATION) {
            return AppError::SlugConflict("A component with this slug already exists".to_string());
        }
    }
    err.into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn selector_parses_default_and_numbers() {
        assert_eq!(
            VersionSelector::parse(None).unwrap(),
            VersionSelector::Default
        );
        assert_eq!(
            VersionSelector::parse(Some("")).unwrap(),
            VersionSelector::Default
        );
        assert_eq!(
            VersionSelector::parse(Some("default")).unwrap(),
            VersionSelector::Default
        );
        assert_eq!(
            VersionSelector::parse(Some("3")).unwrap(),
            VersionSelector::Version(3)
        );
    }

    #[test]
    fn selector_rejects_zero_negative_and_garbage() {
        assert!(VersionSelector::parse(Some("0")).is_err());
        assert!(VersionSelector::parse(Some("-1")).is_err());
        assert!(VersionSelector::parse(Some("abc")).is_err());
    }

    #[test]
    fn variables_to_json_defaults_to_empty_array() {
        assert_eq!(variables_to_json(None).unwrap(), serde_json::json!([]));
    }

    #[test]
    fn variables_to_json_serializes_each_field() {
        let vars = vec![ComponentVariable {
            name: "headline".to_string(),
            title: "Headline".to_string(),
            description: Some("the big text".to_string()),
        }];
        let value = variables_to_json(Some(&vars)).unwrap();
        assert_eq!(value[0]["name"], "headline");
        assert_eq!(value[0]["title"], "Headline");
        assert_eq!(value[0]["description"], "the big text");
    }

    #[test]
    fn conflict_mapping_falls_through_for_non_db_error() {
        let mapped = map_conflict_error(sqlx::Error::RowNotFound);
        assert!(matches!(mapped, AppError::Internal(_)));
    }
}
