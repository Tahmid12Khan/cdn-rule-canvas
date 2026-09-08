//! Outcome + component business logic (BACKEND CONTRACT §7/§10).
//!
//! Responsibilities:
//! - List / get / create / update / delete outcomes (with nested components).
//! - Seed the builtin `ShowContent` outcome on version creation.
//! - Deep-clone an outcome (incl. components, new UUIDs, `is_builtin = false`).
//! - Reorder components within an outcome.
//! - Component CRUD with typed `ComponentConfig` validation.
//!
//! Edit guards: mutating outcomes/components on a non-`DRAFT` version is rejected
//! with `VERSION_EDIT_LOCKED`; deleting the builtin outcome is rejected with
//! `BUILTIN_OUTCOME_PROTECTED`.

use sqlx::PgPool;
use uuid::Uuid;

use crate::error::{AppError, AppResult, ValidationDetail};
use crate::models::enums::VersionStatus;
use crate::repositories::{
    component_repository as components, component_template_repository as component_templates,
    outcome_repository as outcomes,
};
use crate::schemas::component::{ComponentConfig, ComponentCreate, ComponentRead, ComponentUpdate};
use crate::schemas::outcome::{OutcomeCreate, OutcomeRead, OutcomeUpdate, ReorderItem};

/// Canonical title of the protected builtin outcome.
pub const BUILTIN_SHOW_CONTENT_TITLE: &str = "Show Content";

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn outcome_not_found(id: Uuid) -> AppError {
    AppError::OutcomeNotFound(format!("Outcome {id} not found"))
}

fn component_not_found(id: Uuid) -> AppError {
    AppError::ComponentNotFound(format!("Component {id} not found"))
}

fn version_not_found(id: Uuid) -> AppError {
    AppError::VersionNotFound(format!("Version {id} not found"))
}

fn require_draft(status: VersionStatus) -> AppResult<()> {
    if status == VersionStatus::Draft {
        Ok(())
    } else {
        Err(AppError::VersionEditLocked(format!(
            "version is {status:?} (only DRAFT versions are editable)"
        )))
    }
}

/// Validate a `ComponentCreate`/`ComponentUpdate` config against its declared
/// `type` column and domain rules. Returns the JSON to persist on success.
fn validated_config_json(type_str: &str, config: &ComponentConfig) -> AppResult<serde_json::Value> {
    // The serde `type` tag inside `config` must equal the row `type` column.
    if config.type_str() != type_str {
        return Err(AppError::validation(vec![ValidationDetail::new(
            "config.type",
            format!(
                "config type '{}' does not match component type '{}'",
                config.type_str(),
                type_str
            ),
            "component_config_type_match",
        )]));
    }
    config.validate_domain().map_err(|msg| {
        AppError::validation(vec![ValidationDetail::new(
            "config",
            msg,
            "component_config_invalid",
        )])
    })?;
    serde_json::to_value(config).map_err(|e| AppError::Internal(e.into()))
}

/// The library `component_id` referenced by a component-ref config, if any.
/// `None` for non-reference config kinds (`html_injection`, `json_set`, …).
fn referenced_component_id(config: &ComponentConfig) -> Option<Uuid> {
    match config {
        ComponentConfig::ComponentRef { component_id, .. }
        | ComponentConfig::ComponentRefJson { component_id, .. } => Some(*component_id),
        _ => None,
    }
}

/// When `config` is a component-ref, assert the referenced library component
/// exists in `rre.component_templates`. Runs inside the caller's transaction.
/// Mirrors how rule_graph validation checks `apply_component_ref_exists`, but at
/// the component-config layer (`config.component_id`, rule `component_ref_exists`).
async fn ensure_component_ref_exists<'e, E>(exec: E, config: &ComponentConfig) -> AppResult<()>
where
    E: sqlx::Executor<'e, Database = sqlx::Postgres>,
{
    let Some(component_id) = referenced_component_id(config) else {
        return Ok(());
    };
    if component_templates::find(exec, component_id)
        .await?
        .is_none()
    {
        return Err(AppError::validation(vec![ValidationDetail::new(
            "config.component_id",
            format!("library component {component_id} does not exist"),
            "component_ref_exists",
        )]));
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Seed builtin (called by version_service::create_version inside its TX)
// ---------------------------------------------------------------------------

/// Insert the protected builtin `ShowContent` outcome for a freshly created
/// version. Runs inside the caller's transaction (accepts any executor).
pub async fn seed_builtin<'e, E>(exec: E, version_id: Uuid) -> AppResult<()>
where
    E: sqlx::Executor<'e, Database = sqlx::Postgres>,
{
    outcomes::insert(
        exec,
        Uuid::new_v4(),
        version_id,
        BUILTIN_SHOW_CONTENT_TITLE,
        Some("Serve the original content unchanged."),
        true,
        0,
    )
    .await?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Read paths
// ---------------------------------------------------------------------------

/// List all outcomes for a version, each with its components nested
/// (ordered). Components are batch-loaded in one query to avoid N+1.
pub async fn list(pool: &PgPool, version_id: Uuid) -> AppResult<Vec<OutcomeRead>> {
    // Ensure the version exists for a clean 404 rather than an empty list that
    // hides a typo.
    if outcomes::version_status(pool, version_id).await?.is_none() {
        return Err(version_not_found(version_id));
    }

    let rows = outcomes::list_for_version(pool, version_id).await?;
    if rows.is_empty() {
        return Ok(Vec::new());
    }

    let ids: Vec<Uuid> = rows.iter().map(|o| o.id).collect();
    let comps = components::list_for_outcomes(pool, &ids).await?;

    Ok(group_outcomes_with_components(rows, comps))
}

/// Fetch a single outcome with its components.
pub async fn get_with_components(pool: &PgPool, outcome_id: Uuid) -> AppResult<OutcomeRead> {
    let outcome = outcomes::find(pool, outcome_id)
        .await?
        .ok_or_else(|| outcome_not_found(outcome_id))?;
    let comps = components::list_for_outcome(pool, outcome_id).await?;
    let reads: Vec<ComponentRead> = comps.into_iter().map(ComponentRead::from).collect();
    Ok(OutcomeRead::from_parts(outcome, reads))
}

/// Group ordered outcome rows with their (already ordered) components.
fn group_outcomes_with_components(
    outcome_rows: Vec<crate::models::outcome::Outcome>,
    comps: Vec<crate::models::component::Component>,
) -> Vec<OutcomeRead> {
    use std::collections::HashMap;
    let mut by_outcome: HashMap<Uuid, Vec<ComponentRead>> = HashMap::new();
    for c in comps {
        by_outcome
            .entry(c.outcome_id)
            .or_default()
            .push(ComponentRead::from(c));
    }
    outcome_rows
        .into_iter()
        .map(|o| {
            let reads = by_outcome.remove(&o.id).unwrap_or_default();
            OutcomeRead::from_parts(o, reads)
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Outcome mutations
// ---------------------------------------------------------------------------

/// Create a new (non-builtin) outcome under a version. Appends to the end.
pub async fn create(
    pool: &PgPool,
    version_id: Uuid,
    input: OutcomeCreate,
) -> AppResult<OutcomeRead> {
    let mut tx = pool.begin().await?;

    // Lock the governing version row FOR UPDATE before the draft check so a
    // concurrent publish (which also locks the row) cannot slip past the guard.
    let status = outcomes::version_status_for_update(&mut *tx, version_id)
        .await?
        .ok_or_else(|| version_not_found(version_id))?;
    require_draft(status)?;

    let next = outcomes::max_order_index(&mut *tx, version_id)
        .await?
        .map(|m| m + 1)
        .unwrap_or(0);

    let outcome = outcomes::insert(
        &mut *tx,
        Uuid::new_v4(),
        version_id,
        &input.title,
        input.description.as_deref(),
        false,
        next,
    )
    .await?;

    tx.commit().await?;
    Ok(OutcomeRead::from_parts(outcome, Vec::new()))
}

/// Patch an outcome's title/description/order. Builtin outcomes may be patched
/// (e.g. reorder) but their version must still be DRAFT.
pub async fn update(
    pool: &PgPool,
    outcome_id: Uuid,
    input: OutcomeUpdate,
) -> AppResult<OutcomeRead> {
    let mut tx = pool.begin().await?;

    let (_vid, status) = outcomes::version_status_for_outcome_for_update(&mut *tx, outcome_id)
        .await?
        .ok_or_else(|| outcome_not_found(outcome_id))?;
    require_draft(status)?;

    // PATCH semantics: a `null`/absent `description` leaves the stored value
    // unchanged (COALESCE behavior). Plain `Option<String>` cannot distinguish
    // "absent" from explicit `null`, so we treat `None` as "leave unchanged".
    let touch_description = input.description.as_ref().map(|d| Some(d.as_str()));

    let updated = outcomes::update(
        &mut *tx,
        outcome_id,
        input.title.as_deref(),
        touch_description,
        input.order_index,
    )
    .await?
    .ok_or_else(|| outcome_not_found(outcome_id))?;

    let comps = components::list_for_outcome(&mut *tx, outcome_id).await?;
    tx.commit().await?;
    let reads: Vec<ComponentRead> = comps.into_iter().map(ComponentRead::from).collect();
    Ok(OutcomeRead::from_parts(updated, reads))
}

/// Delete an outcome. Rejects builtin outcomes and non-DRAFT versions.
pub async fn delete(pool: &PgPool, outcome_id: Uuid) -> AppResult<()> {
    let mut tx = pool.begin().await?;

    let outcome = outcomes::find(&mut *tx, outcome_id)
        .await?
        .ok_or_else(|| outcome_not_found(outcome_id))?;

    if outcome.is_builtin {
        return Err(AppError::BuiltinOutcomeProtected(
            "the builtin ShowContent outcome cannot be deleted".to_string(),
        ));
    }

    // Lock the governing version row before the draft check + delete.
    let status = outcomes::version_status_for_update(&mut *tx, outcome.version_id)
        .await?
        .ok_or_else(|| version_not_found(outcome.version_id))?;
    require_draft(status)?;

    let removed = outcomes::delete(&mut *tx, outcome_id).await?;
    if removed == 0 {
        return Err(outcome_not_found(outcome_id));
    }
    tx.commit().await?;
    Ok(())
}

/// Deep-clone an outcome (incl. its components) within the same version.
/// New UUIDs, `is_builtin = false`, title `"{title} (copy)"`. TX-wrapped.
pub async fn clone_outcome(pool: &PgPool, outcome_id: Uuid) -> AppResult<OutcomeRead> {
    let mut tx = pool.begin().await?;

    let source = outcomes::find(&mut *tx, outcome_id)
        .await?
        .ok_or_else(|| outcome_not_found(outcome_id))?;

    // Lock the governing version row FOR UPDATE before the draft check + clone.
    let status = outcomes::version_status_for_update(&mut *tx, source.version_id)
        .await?
        .ok_or_else(|| version_not_found(source.version_id))?;
    require_draft(status)?;

    let source_components = components::list_for_outcome(&mut *tx, outcome_id).await?;

    let next = outcomes::max_order_index(&mut *tx, source.version_id)
        .await?
        .map(|m| m + 1)
        .unwrap_or(0);

    let new_outcome = outcomes::insert(
        &mut *tx,
        Uuid::new_v4(),
        source.version_id,
        &format!("{} (copy)", source.title),
        source.description.as_deref(),
        false,
        next,
    )
    .await?;

    let mut cloned_reads: Vec<ComponentRead> = Vec::with_capacity(source_components.len());
    for c in source_components {
        let inserted = components::insert(
            &mut *tx,
            Uuid::new_v4(),
            new_outcome.id,
            &c.slug,
            &c.r#type,
            &c.config,
            c.placement,
            c.order_index,
        )
        .await?;
        cloned_reads.push(ComponentRead::from(inserted));
    }

    tx.commit().await?;

    Ok(OutcomeRead::from_parts(new_outcome, cloned_reads))
}

// ---------------------------------------------------------------------------
// Component mutations
// ---------------------------------------------------------------------------

/// Add a component to an outcome. Validates config against `type`. Appends.
pub async fn add_component(
    pool: &PgPool,
    outcome_id: Uuid,
    input: ComponentCreate,
) -> AppResult<ComponentRead> {
    let config_json = validated_config_json(&input.r#type, &input.config)?;

    let mut tx = pool.begin().await?;

    let (_vid, status) = outcomes::version_status_for_outcome_for_update(&mut *tx, outcome_id)
        .await?
        .ok_or_else(|| outcome_not_found(outcome_id))?;
    require_draft(status)?;

    // For component-ref configs, the referenced library component must exist.
    ensure_component_ref_exists(&mut *tx, &input.config).await?;

    let next = match input.order_index {
        Some(idx) => idx,
        None => components::max_order_index(&mut *tx, outcome_id)
            .await?
            .map(|m| m + 1)
            .unwrap_or(0),
    };

    let inserted = components::insert(
        &mut *tx,
        Uuid::new_v4(),
        outcome_id,
        &input.slug,
        &input.r#type,
        &config_json,
        input.placement,
        next,
    )
    .await?;

    tx.commit().await?;
    Ok(ComponentRead::from(inserted))
}

/// Patch a component. Validates config against the (possibly updated) `type`.
pub async fn update_component(
    pool: &PgPool,
    component_id: Uuid,
    input: ComponentUpdate,
) -> AppResult<ComponentRead> {
    let mut tx = pool.begin().await?;

    let existing = components::find(&mut *tx, component_id)
        .await?
        .ok_or_else(|| component_not_found(component_id))?;

    let (_vid, status) = outcomes::version_status_for_component_for_update(&mut *tx, component_id)
        .await?
        .ok_or_else(|| component_not_found(component_id))?;
    require_draft(status)?;

    // If config is being set, validate it against the effective type (new type
    // if provided, else the existing type) and assert any referenced library
    // component exists.
    let config_json = match &input.config {
        Some(cfg) => {
            let effective_type = input.r#type.as_deref().unwrap_or(&existing.r#type);
            let json = validated_config_json(effective_type, cfg)?;
            ensure_component_ref_exists(&mut *tx, cfg).await?;
            Some(json)
        }
        None => None,
    };

    let updated = components::update(
        &mut *tx,
        component_id,
        input.slug.as_deref(),
        input.r#type.as_deref(),
        config_json.as_ref(),
        input.placement,
        input.order_index,
    )
    .await?
    .ok_or_else(|| component_not_found(component_id))?;

    tx.commit().await?;
    Ok(ComponentRead::from(updated))
}

/// Delete a component. Version must be DRAFT.
pub async fn delete_component(pool: &PgPool, component_id: Uuid) -> AppResult<()> {
    let mut tx = pool.begin().await?;

    let (_vid, status) = outcomes::version_status_for_component_for_update(&mut *tx, component_id)
        .await?
        .ok_or_else(|| component_not_found(component_id))?;
    require_draft(status)?;

    let removed = components::delete(&mut *tx, component_id).await?;
    if removed == 0 {
        return Err(component_not_found(component_id));
    }
    tx.commit().await?;
    Ok(())
}

/// Reorder an outcome's components. Body lists `{id, order_index}` pairs; every
/// id must belong to the outcome. Returns the components in the new order.
pub async fn reorder(
    pool: &PgPool,
    outcome_id: Uuid,
    items: Vec<ReorderItem>,
) -> AppResult<Vec<ComponentRead>> {
    let mut tx = pool.begin().await?;

    let (_vid, status) = outcomes::version_status_for_outcome_for_update(&mut *tx, outcome_id)
        .await?
        .ok_or_else(|| outcome_not_found(outcome_id))?;
    require_draft(status)?;

    // Validate every referenced component belongs to this outcome.
    let existing = components::list_for_outcome(&mut *tx, outcome_id).await?;
    let owned: std::collections::HashSet<Uuid> = existing.iter().map(|c| c.id).collect();
    for item in &items {
        if !owned.contains(&item.id) {
            return Err(AppError::validation(vec![ValidationDetail::new(
                "[]",
                format!(
                    "component {} does not belong to outcome {}",
                    item.id, outcome_id
                ),
                "reorder_component_owned",
            )]));
        }
    }

    // Apply all order changes in one round-trip (avoids one UPDATE per item).
    let orders: Vec<(Uuid, i32)> = items.iter().map(|i| (i.id, i.order_index)).collect();
    components::set_order_bulk(&mut *tx, &orders).await?;

    let reordered = components::list_for_outcome(&mut *tx, outcome_id).await?;
    tx.commit().await?;
    Ok(reordered.into_iter().map(ComponentRead::from).collect())
}

// ---------------------------------------------------------------------------
// Unit tests (pure logic that needs no DB)
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::enums::Placement;
    use crate::schemas::component::HtmlPlacementMode;

    #[test]
    fn require_draft_allows_draft() {
        assert!(require_draft(VersionStatus::Draft).is_ok());
    }

    #[test]
    fn require_draft_rejects_non_draft() {
        for s in [
            VersionStatus::Live,
            VersionStatus::Staging,
            VersionStatus::Prev,
        ] {
            let err = require_draft(s).unwrap_err();
            assert_eq!(err.code(), "VERSION_EDIT_LOCKED");
        }
    }

    #[test]
    fn config_type_mismatch_is_validation_error() {
        let cfg = ComponentConfig::HtmlInjection {
            target_selector: ".x".to_string(),
            placement_mode: HtmlPlacementMode::Append,
            html_body: "<p>hi</p>".to_string(),
            theme: None,
        };
        let err = validated_config_json("content_truncation", &cfg).unwrap_err();
        assert_eq!(err.code(), "VALIDATION_ERROR");
    }

    #[test]
    fn config_type_match_serializes_with_discriminator() {
        let cfg = ComponentConfig::HtmlInjection {
            target_selector: ".x".to_string(),
            placement_mode: HtmlPlacementMode::Replace,
            html_body: "<p>hi</p>".to_string(),
            theme: Some("dark".to_string()),
        };
        let json = validated_config_json("html_injection", &cfg).unwrap();
        assert_eq!(json["type"], "html_injection");
        assert_eq!(json["placement_mode"], "replace");
        assert_eq!(json["target_selector"], ".x");
    }

    #[test]
    fn empty_selector_is_rejected() {
        let cfg = ComponentConfig::HtmlInjection {
            target_selector: "   ".to_string(),
            placement_mode: HtmlPlacementMode::Append,
            html_body: String::new(),
            theme: None,
        };
        let err = validated_config_json("html_injection", &cfg).unwrap_err();
        assert_eq!(err.code(), "VALIDATION_ERROR");
    }

    #[test]
    fn truncation_word_count_bounds_enforced() {
        let zero = ComponentConfig::ContentTruncation {
            target_selector: ".article".to_string(),
            word_count: 0,
            fade_out: false,
        };
        assert!(validated_config_json("content_truncation", &zero).is_err());

        let too_big = ComponentConfig::ContentTruncation {
            target_selector: ".article".to_string(),
            word_count: 10_001,
            fade_out: true,
        };
        assert!(validated_config_json("content_truncation", &too_big).is_err());

        let ok = ComponentConfig::ContentTruncation {
            target_selector: ".article".to_string(),
            word_count: 100,
            fade_out: true,
        };
        assert!(validated_config_json("content_truncation", &ok).is_ok());
    }

    #[test]
    fn html_remove_serializes_with_discriminator_and_flag() {
        let cfg = ComponentConfig::HtmlRemove {
            target_selector: "#dn-content-ssr".to_string(),
            include_selector: true,
        };
        let json = validated_config_json("html_remove", &cfg).unwrap();
        assert_eq!(json["type"], "html_remove");
        assert_eq!(json["target_selector"], "#dn-content-ssr");
        assert_eq!(json["include_selector"], true);
    }

    #[test]
    fn html_remove_include_selector_defaults_to_false() {
        let cfg: ComponentConfig = serde_json::from_value(serde_json::json!({
            "type": "html_remove",
            "target_selector": "#dn-content-ssr"
        }))
        .unwrap();
        let json = validated_config_json("html_remove", &cfg).unwrap();
        assert_eq!(json["include_selector"], false);
    }

    #[test]
    fn html_remove_empty_selector_is_rejected() {
        let cfg = ComponentConfig::HtmlRemove {
            target_selector: "  ".to_string(),
            include_selector: false,
        };
        let err = validated_config_json("html_remove", &cfg).unwrap_err();
        assert_eq!(err.code(), "VALIDATION_ERROR");
    }

    #[test]
    fn group_outcomes_attaches_components_in_order() {
        use chrono::Utc;
        let now = Utc::now();
        let oid = Uuid::new_v4();
        let outcome = crate::models::outcome::Outcome {
            id: oid,
            version_id: Uuid::new_v4(),
            title: "T".to_string(),
            description: None,
            is_builtin: false,
            order_index: 0,
            created_at: now,
            updated_at: now,
        };
        let comp = crate::models::component::Component {
            id: Uuid::new_v4(),
            outcome_id: oid,
            slug: "c1".to_string(),
            r#type: "html_injection".to_string(),
            config: serde_json::json!({"type": "html_injection"}),
            placement: Placement::Inline,
            order_index: 0,
            created_at: now,
            updated_at: now,
        };
        let grouped = group_outcomes_with_components(vec![outcome], vec![comp]);
        assert_eq!(grouped.len(), 1);
        assert_eq!(grouped[0].components.len(), 1);
        assert_eq!(grouped[0].components[0].slug, "c1");
    }

    #[test]
    fn group_outcomes_handles_outcome_with_no_components() {
        use chrono::Utc;
        let now = Utc::now();
        let outcome = crate::models::outcome::Outcome {
            id: Uuid::new_v4(),
            version_id: Uuid::new_v4(),
            title: "Empty".to_string(),
            description: None,
            is_builtin: true,
            order_index: 0,
            created_at: now,
            updated_at: now,
        };
        let grouped = group_outcomes_with_components(vec![outcome], vec![]);
        assert_eq!(grouped.len(), 1);
        assert!(grouped[0].components.is_empty());
    }
}
