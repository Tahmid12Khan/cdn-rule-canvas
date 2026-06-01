//! Version business logic (BACKEND CONTRACT §7/§10).
//!
//! Owns the transaction boundary, the status lifecycle, and the active-version
//! read. Raises domain [`AppError`]s; returns serde DTOs (never `FromRow`).

use std::collections::{HashMap, HashSet};

use chrono::Utc;
use sqlx::PgPool;
use uuid::Uuid;

use crate::{
    error::{AppError, AppResult},
    models::{component::Component, enums::VersionStatus, outcome::Outcome, version::Version},
    repositories::{component_repository, outcome_repository, version_repository as repo},
    schemas::{
        active_version::{ActiveComponent, ActiveOutcome, ActiveVersionRead},
        applicability::Applicability,
        node_type::NodeManifest,
        pagination::{Page, PageParams},
        rule_graph::{Node, RuleGraph},
        version::{
            PublishEnvironment, VersionCreate, VersionListQuery, VersionRead, VersionSummary,
            VersionUpdate,
        },
    },
    services::rule_graph_service,
};

/// Title of the builtin "show content" outcome seeded into every new version.
const SHOW_CONTENT_TITLE: &str = "Show Content";
/// Default actor when no auth context is wired (MVP).
const SYSTEM_ACTOR: &str = "system";

/// Parse the stored applicability JSONB, defaulting to `{}` on a null/missing or
/// otherwise un-parseable value (forward-compatible: never fail a read on it).
fn parse_applicability(value: serde_json::Value) -> Applicability {
    serde_json::from_value(value).unwrap_or_default()
}

/// Map a [`Version`] model to its full DTO, parsing the stored rule_graph JSON.
fn to_read(v: Version) -> AppResult<VersionRead> {
    let rule_graph: RuleGraph = serde_json::from_value(v.rule_graph)
        .map_err(|e| AppError::Internal(anyhow::anyhow!("corrupt rule_graph: {e}")))?;
    let applicability = parse_applicability(v.applicability);
    Ok(VersionRead {
        id: v.id,
        feature_id: v.feature_id,
        version_number: v.version_number,
        description: v.description,
        status: v.status,
        rule_graph,
        applicability,
        created_by: v.created_by,
        last_updated_by: v.last_updated_by,
        last_updated_at: v.last_updated_at,
        created_at: v.created_at,
    })
}

/// Map a [`Version`] model to its summary DTO (no rule_graph).
fn to_summary(v: Version) -> VersionSummary {
    VersionSummary {
        id: v.id,
        feature_id: v.feature_id,
        version_number: v.version_number,
        description: v.description,
        status: v.status,
        last_updated_by: v.last_updated_by,
        last_updated_at: v.last_updated_at,
        created_at: v.created_at,
    }
}

fn version_not_found(feature_id: &str, version_number: i32) -> AppError {
    AppError::VersionNotFound(format!(
        "Version {version_number} not found for feature '{feature_id}'"
    ))
}

/// Create a new version for a feature.
///
/// TX: lock the feature's versions, compute the next version_number, pick the
/// source version to carry forward from (current LIVE, else the highest existing
/// version_number), insert the new DRAFT version, then carry forward ALL of the
/// source's outcomes + nested components (fresh UUIDs, titles/order/is_builtin
/// preserved) while building a `source_outcome_id -> new_outcome_id` map. When
/// the feature has no prior version, seed a single builtin ShowContent outcome
/// instead.
///
/// The graph to store is `body.rule_graph` when present, else the cloned source
/// graph. Either way its outcome-node references (which point at the SOURCE
/// version's outcome ids) are REMAPPED through the carry-forward map to the new
/// outcome ids, then VALIDATED with the same rule the update path uses (422 on
/// failure) before being persisted on the new version.
pub async fn create_version(
    pool: &PgPool,
    feature_id: &str,
    body: VersionCreate,
    manifest: &NodeManifest,
) -> AppResult<VersionRead> {
    if !repo::feature_exists(pool, feature_id).await? {
        return Err(AppError::FeatureNotFound(format!(
            "Feature '{feature_id}' not found"
        )));
    }

    let mut tx = pool.begin().await?;

    let next_number = repo::max_version_number_for_update(&mut *tx, feature_id).await? + 1;

    // Carry-forward source: prefer the current LIVE version; otherwise fall back
    // to the highest existing version_number for the feature (the same row whose
    // rule_graph we clone). `None` only on the feature's very first version.
    let source = match repo::find_by_status(&mut *tx, feature_id, VersionStatus::Live).await? {
        Some(live) => Some(live),
        None => match next_number - 1 {
            0 => None,
            prev_number => repo::find_by_number(&mut *tx, feature_id, prev_number).await?,
        },
    };

    // Insert the new DRAFT version with a placeholder graph; the validated,
    // remapped graph is written below before commit. We insert first so that
    // carry-forward outcomes get a valid `version_id` to attach to.
    let placeholder = serde_json::to_value(RuleGraph::default())
        .map_err(|e| AppError::Internal(anyhow::anyhow!(e)))?;
    let version = repo::insert(
        &mut *tx,
        feature_id,
        next_number,
        body.description.as_deref(),
        VersionStatus::Draft,
        &placeholder,
        SYSTEM_ACTOR,
    )
    .await?;

    // Carry forward outcomes (building the source->new id map), or seed builtin.
    let outcome_id_map = match &source {
        // A prior version exists: carry forward all of its outcomes + components
        // (including the builtin), so we must NOT also seed a fresh builtin.
        Some(src) => carry_forward_outcomes(&mut tx, src.id, version.id).await?,
        // First version of the feature: seed the single builtin outcome.
        None => {
            seed_builtin_outcome(&mut tx, version.id).await?;
            HashMap::new()
        }
    };

    // Choose the graph to store: caller-supplied when present, else the cloned
    // source graph (today's behavior), else the empty default (first version).
    let mut graph: RuleGraph = match body.rule_graph {
        Some(graph) => graph,
        None => match &source {
            Some(src) => serde_json::from_value(src.rule_graph.clone())
                .map_err(|e| AppError::Internal(anyhow::anyhow!("corrupt rule_graph: {e}")))?,
            None => RuleGraph::default(),
        },
    };

    // Remap outcome references from the source ids onto the new outcome ids, then
    // validate with the same rule the update path uses (422 on failure).
    remap_outcome_refs(&mut graph, &outcome_id_map);

    let valid_outcome_ids: HashSet<Uuid> =
        outcome_repository::list_for_version(&mut *tx, version.id)
            .await?
            .iter()
            .map(|o| o.id)
            .collect();
    rule_graph_service::validate(&graph, &valid_outcome_ids, manifest)?;

    // Applicability: caller-supplied (validated) when present, else carry forward
    // the source version's, else the default `{}`.
    let applicability = match body.applicability {
        Some(a) => {
            validate_applicability(&a)?;
            a
        }
        None => match &source {
            Some(src) => parse_applicability(src.applicability.clone()),
            None => Applicability::default(),
        },
    };
    let applicability_json =
        serde_json::to_value(&applicability).map_err(|e| AppError::Internal(anyhow::anyhow!(e)))?;

    let rule_graph_json =
        serde_json::to_value(&graph).map_err(|e| AppError::Internal(anyhow::anyhow!(e)))?;
    let version = repo::update_fields(
        &mut *tx,
        version.id,
        None,
        Some(&rule_graph_json),
        Some(&applicability_json),
        SYSTEM_ACTOR,
        version.last_updated_at,
    )
    .await?;

    tx.commit().await?;
    to_read(version)
}

/// Validate version-level applicability (light: length-capped, non-blank
/// selectors). Returns a 422 `VALIDATION_ERROR` on overflow / blank input.
fn validate_applicability(applicability: &Applicability) -> AppResult<()> {
    let details = applicability.validation_details();
    if details.is_empty() {
        Ok(())
    } else {
        Err(AppError::validation(details))
    }
}

/// Deep-copy every outcome (and its nested components) from `source_version_id`
/// into `target_version_id` with fresh UUIDs. Titles, ordering, `is_builtin`,
/// and component slug/type/config/placement/order are preserved exactly (no
/// "(copy)" suffix). Runs inside the caller's version-creation transaction.
///
/// Returns a `source_outcome_id -> new_outcome_id` map so the caller can remap
/// the new version's rule_graph outcome references.
async fn carry_forward_outcomes(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    source_version_id: Uuid,
    target_version_id: Uuid,
) -> AppResult<HashMap<Uuid, Uuid>> {
    let source_outcomes =
        outcome_repository::list_for_version(&mut **tx, source_version_id).await?;

    let mut id_map: HashMap<Uuid, Uuid> = HashMap::with_capacity(source_outcomes.len());

    for outcome in source_outcomes {
        let new_outcome = outcome_repository::insert(
            &mut **tx,
            Uuid::new_v4(),
            target_version_id,
            &outcome.title,
            outcome.description.as_deref(),
            outcome.is_builtin,
            outcome.order_index,
        )
        .await?;
        id_map.insert(outcome.id, new_outcome.id);

        let source_components =
            component_repository::list_for_outcome(&mut **tx, outcome.id).await?;
        for c in source_components {
            component_repository::insert(
                &mut **tx,
                Uuid::new_v4(),
                new_outcome.id,
                &c.slug,
                &c.r#type,
                &c.config,
                c.placement,
                c.order_index,
            )
            .await?;
        }
    }

    Ok(id_map)
}

/// Rewrite every `apply_outcome` expression action's `outcome_id` across all
/// three canvases through `map` (source id -> new id). Keys absent from `map`
/// are left untouched, as are decision nodes, edges, positions, and
/// `root_node_id`. The `outcome_id` lives in the action's flattened field map as
/// a UUID string; non-`apply_outcome` actions and unparsable ids are skipped.
fn remap_outcome_refs(graph: &mut RuleGraph, map: &HashMap<Uuid, Uuid>) {
    if map.is_empty() {
        return;
    }
    for canvas in [
        &mut graph.anonymous,
        &mut graph.registered,
        &mut graph.customer,
    ] {
        for node in &mut canvas.nodes {
            if let Node::Expression { action, .. } = node {
                if action.r#type != "apply_outcome" {
                    continue;
                }
                let Some(serde_json::Value::String(s)) = action.fields.get("outcome_id") else {
                    continue;
                };
                let Ok(old_id) = Uuid::parse_str(s) else {
                    continue;
                };
                if let Some(new_id) = map.get(&old_id) {
                    action.fields.insert(
                        "outcome_id".to_string(),
                        serde_json::Value::String(new_id.to_string()),
                    );
                }
            }
        }
    }
}

/// Insert the protected builtin "Show Content" outcome for a freshly created
/// version. Written here (rather than `outcome_repository`) because it is part
/// of the version-creation unit of work.
async fn seed_builtin_outcome(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    version_id: Uuid,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO rre.outcomes (version_id, title, is_builtin, order_index) \
         VALUES ($1, $2, true, 0)",
    )
    .bind(version_id)
    .bind(SHOW_CONTENT_TITLE)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

/// List versions for a feature, filtered + paginated.
pub async fn list(
    pool: &PgPool,
    feature_id: &str,
    query: VersionListQuery,
) -> AppResult<Page<VersionSummary>> {
    if !repo::feature_exists(pool, feature_id).await? {
        return Err(AppError::FeatureNotFound(format!(
            "Feature '{feature_id}' not found"
        )));
    }

    let VersionListQuery {
        status,
        search,
        page,
        page_size,
    } = query;
    let (limit, offset, page_no, page_size) = PageParams { page, page_size }.resolve();
    let search = search.as_deref().filter(|s| !s.is_empty());

    let rows = repo::list_paged(pool, feature_id, status, search, limit, offset).await?;
    let total = repo::count(pool, feature_id, status, search).await?;

    let items = rows.into_iter().map(to_summary).collect();
    Ok(Page::new(items, page_no, page_size, total))
}

/// Fetch a single version by `(feature_id, version_number)`.
pub async fn get(pool: &PgPool, feature_id: &str, version_number: i32) -> AppResult<VersionRead> {
    let version = repo::find_by_number(pool, feature_id, version_number)
        .await?
        .ok_or_else(|| version_not_found(feature_id, version_number))?;
    to_read(version)
}

/// Update a version's description and/or rule_graph.
///
/// A `rule_graph` may only be set on a DRAFT version (else
/// `VERSION_EDIT_LOCKED`); when present it is validated by the
/// `rule_graph_service` before persisting. Description-only edits are allowed
/// on any status.
pub async fn update(
    pool: &PgPool,
    feature_id: &str,
    version_number: i32,
    body: VersionUpdate,
    manifest: &NodeManifest,
) -> AppResult<VersionRead> {
    let version = repo::find_by_number(pool, feature_id, version_number)
        .await?
        .ok_or_else(|| version_not_found(feature_id, version_number))?;

    let rule_graph_json = match &body.rule_graph {
        Some(graph) => {
            if version.status != VersionStatus::Draft {
                return Err(AppError::VersionEditLocked(format!(
                    "Version {version_number} is {:?} and cannot be edited",
                    version.status
                )));
            }
            // Validate the graph against the version's outcomes (422 on failure).
            let outcomes = outcome_repository::list_for_version(pool, version.id).await?;
            let valid_outcome_ids: std::collections::HashSet<Uuid> =
                outcomes.iter().map(|o| o.id).collect();
            rule_graph_service::validate(graph, &valid_outcome_ids, manifest)?;
            Some(serde_json::to_value(graph).map_err(|e| AppError::Internal(anyhow::anyhow!(e)))?)
        }
        None => None,
    };

    // Applicability is editable on DRAFT only (same lock as rule_graph); validate
    // the selectors when present.
    let applicability_json = match &body.applicability {
        Some(applicability) => {
            if version.status != VersionStatus::Draft {
                return Err(AppError::VersionEditLocked(format!(
                    "Version {version_number} is {:?} and cannot be edited",
                    version.status
                )));
            }
            validate_applicability(applicability)?;
            Some(
                serde_json::to_value(applicability)
                    .map_err(|e| AppError::Internal(anyhow::anyhow!(e)))?,
            )
        }
        None => None,
    };

    let mut tx = pool.begin().await?;
    let updated = repo::update_fields(
        &mut *tx,
        version.id,
        body.description.as_deref(),
        rule_graph_json.as_ref(),
        applicability_json.as_ref(),
        SYSTEM_ACTOR,
        Utc::now(),
    )
    .await?;
    tx.commit().await?;

    to_read(updated)
}

/// Convenience entrypoint for a rule_graph-only update (validates + persists).
pub async fn update_rule_graph(
    pool: &PgPool,
    feature_id: &str,
    version_number: i32,
    rule_graph: RuleGraph,
    manifest: &NodeManifest,
) -> AppResult<VersionRead> {
    update(
        pool,
        feature_id,
        version_number,
        VersionUpdate {
            description: None,
            rule_graph: Some(rule_graph),
            applicability: None,
        },
        manifest,
    )
    .await
}

/// Publish a version to staging or live (§7 lifecycle).
pub async fn publish(
    pool: &PgPool,
    feature_id: &str,
    version_number: i32,
    environment: PublishEnvironment,
) -> AppResult<VersionRead> {
    let target = repo::find_by_number(pool, feature_id, version_number)
        .await?
        .ok_or_else(|| version_not_found(feature_id, version_number))?;

    let mut tx = pool.begin().await?;
    // Lock the target row to serialize concurrent publishes on this feature.
    let target = repo::find_by_id_for_update(&mut *tx, target.id)
        .await?
        .ok_or_else(|| version_not_found(feature_id, version_number))?;

    let result = match environment {
        PublishEnvironment::Live => publish_live(&mut tx, &target).await,
        PublishEnvironment::Staging => publish_staging(&mut tx, &target).await,
    };
    let updated = result?;

    tx.commit().await?;
    to_read(updated)
}

/// Publish `target` to LIVE: demote the current LIVE (if any, different row) to
/// PREV, promote `target` to LIVE, point `features.live_version_id` at it.
async fn publish_live(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    target: &Version,
) -> AppResult<Version> {
    // Target must be DRAFT | STAGING | PREV.
    if target.status == VersionStatus::Live {
        return Err(AppError::InvalidStatusTransition(format!(
            "Version {} is already live",
            target.version_number
        )));
    }

    if let Some(current_live) =
        repo::find_by_status(&mut **tx, &target.feature_id, VersionStatus::Live).await?
    {
        if current_live.id != target.id {
            repo::update_status(&mut **tx, current_live.id, VersionStatus::Prev).await?;
            // If the demoted live was also the staging pointer, leave staging as-is;
            // the pointer still references a valid row.
        }
    }

    let updated = repo::update_status(&mut **tx, target.id, VersionStatus::Live).await?;
    repo::set_feature_live(&mut **tx, &target.feature_id, Some(target.id)).await?;
    Ok(updated)
}

/// Publish `target` to STAGING.
///
/// The previous STAGING row (status = `staging`) is demoted to PREV. The target
/// is promoted to `staging` unless it is currently the LIVE row, in which case
/// its `live` status is preserved (status is single-valued) and only the
/// `features.staging_version_id` pointer is moved — so both slots may reference
/// the same id (the UI derives "+1").
async fn publish_staging(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    target: &Version,
) -> AppResult<Version> {
    if target.status == VersionStatus::Staging {
        return Err(AppError::InvalidStatusTransition(format!(
            "Version {} is already staging",
            target.version_number
        )));
    }

    // Demote the existing staging row (a single-valued `status='staging'` row;
    // a LIVE row serving as staging has status `live` and is not matched here).
    if let Some(current_staging) =
        repo::find_by_status(&mut **tx, &target.feature_id, VersionStatus::Staging).await?
    {
        if current_staging.id != target.id {
            repo::update_status(&mut **tx, current_staging.id, VersionStatus::Prev).await?;
        }
    }

    // Preserve LIVE status when the target is the live row; otherwise set staging.
    let updated = if target.status == VersionStatus::Live {
        target.clone()
    } else {
        repo::update_status(&mut **tx, target.id, VersionStatus::Staging).await?
    };
    repo::set_feature_staging(&mut **tx, &target.feature_id, Some(target.id)).await?;
    Ok(updated)
}

/// Unpublish a version from staging or live (§7 lifecycle).
pub async fn unpublish(
    pool: &PgPool,
    feature_id: &str,
    version_number: i32,
    environment: PublishEnvironment,
) -> AppResult<VersionRead> {
    let target = repo::find_by_number(pool, feature_id, version_number)
        .await?
        .ok_or_else(|| version_not_found(feature_id, version_number))?;

    let mut tx = pool.begin().await?;
    let target = repo::find_by_id_for_update(&mut *tx, target.id)
        .await?
        .ok_or_else(|| version_not_found(feature_id, version_number))?;

    let updated = match environment {
        PublishEnvironment::Live => {
            if target.status != VersionStatus::Live {
                return Err(AppError::InvalidStatusTransition(format!(
                    "Version {version_number} is not live; cannot unpublish"
                )));
            }
            let updated = repo::update_status(&mut *tx, target.id, VersionStatus::Prev).await?;
            repo::set_feature_live(&mut *tx, &target.feature_id, None).await?;
            updated
        }
        PublishEnvironment::Staging => {
            // The target is acceptable if it is the staging row (status=staging)
            // OR the live row that the staging pointer also references (status=live).
            let pointers = repo::feature_version_pointers(&mut *tx, &target.feature_id).await?;
            let staging_ptr = pointers.and_then(|(s, _)| s);
            match target.status {
                VersionStatus::Staging => {
                    let updated =
                        repo::update_status(&mut *tx, target.id, VersionStatus::Prev).await?;
                    repo::set_feature_staging(&mut *tx, &target.feature_id, None).await?;
                    updated
                }
                VersionStatus::Live if staging_ptr == Some(target.id) => {
                    // Keep LIVE; only clear the staging pointer.
                    repo::set_feature_staging(&mut *tx, &target.feature_id, None).await?;
                    target.clone()
                }
                _ => {
                    return Err(AppError::InvalidStatusTransition(format!(
                        "Version {version_number} is not staging; cannot unpublish"
                    )));
                }
            }
        }
    };

    tx.commit().await?;
    to_read(updated)
}

/// Delete a version. Only DRAFT or PREV versions are deletable; LIVE/STAGING
/// → `INVALID_STATUS_TRANSITION`.
pub async fn delete(pool: &PgPool, feature_id: &str, version_number: i32) -> AppResult<()> {
    let version = repo::find_by_number(pool, feature_id, version_number)
        .await?
        .ok_or_else(|| version_not_found(feature_id, version_number))?;

    if matches!(version.status, VersionStatus::Live | VersionStatus::Staging) {
        return Err(AppError::InvalidStatusTransition(format!(
            "Version {version_number} is {:?}; only draft or prev versions can be deleted",
            version.status
        )));
    }

    repo::delete(pool, version.id).await?;
    Ok(())
}

/// Read the active (LIVE or STAGING) version for a feature, flattened for the
/// proxy. No tx; batched component read avoids N+1.
pub async fn active_version(
    pool: &PgPool,
    feature_id: &str,
    environment: PublishEnvironment,
) -> AppResult<ActiveVersionRead> {
    let pointers = repo::feature_version_pointers(pool, feature_id)
        .await?
        .ok_or_else(|| AppError::FeatureNotFound(format!("Feature '{feature_id}' not found")))?;
    let (staging_id, live_id) = pointers;

    let active_id = match environment {
        PublishEnvironment::Live => live_id,
        PublishEnvironment::Staging => staging_id,
    }
    .ok_or_else(|| {
        AppError::NoLiveVersion(format!(
            "No {environment:?} version for feature '{feature_id}'"
        ))
    })?;

    let version = repo::find_by_id(pool, active_id).await?.ok_or_else(|| {
        AppError::NoLiveVersion(format!("Active version missing for feature '{feature_id}'"))
    })?;

    let rule_graph: RuleGraph = serde_json::from_value(version.rule_graph)
        .map_err(|e| AppError::Internal(anyhow::anyhow!("corrupt rule_graph: {e}")))?;
    let applicability = parse_applicability(version.applicability);

    let outcomes = repo::list_outcomes(pool, version.id).await?;
    let outcome_ids: Vec<Uuid> = outcomes.iter().map(|o| o.id).collect();
    let components = if outcome_ids.is_empty() {
        Vec::new()
    } else {
        repo::list_components_for_outcomes(pool, &outcome_ids).await?
    };

    let active_outcomes = assemble_active_outcomes(outcomes, components);

    Ok(ActiveVersionRead {
        version_number: version.version_number,
        rule_graph,
        applicability,
        outcomes: active_outcomes,
    })
}

/// Group components under their outcomes (both already ordered by the repo),
/// producing the proxy-facing payload.
fn assemble_active_outcomes(
    outcomes: Vec<Outcome>,
    components: Vec<Component>,
) -> Vec<ActiveOutcome> {
    use std::collections::HashMap;

    let mut by_outcome: HashMap<Uuid, Vec<ActiveComponent>> = HashMap::new();
    for c in components {
        by_outcome
            .entry(c.outcome_id)
            .or_default()
            .push(ActiveComponent {
                id: c.id,
                slug: c.slug,
                r#type: c.r#type,
                config: c.config,
                placement: c.placement,
                order_index: c.order_index,
            });
    }

    outcomes
        .into_iter()
        .map(|o| ActiveOutcome {
            components: by_outcome.remove(&o.id).unwrap_or_default(),
            id: o.id,
            title: o.title,
            is_builtin: o.is_builtin,
            order_index: o.order_index,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::enums::Placement;
    use chrono::Utc;

    fn outcome(id: Uuid, title: &str, builtin: bool, order: i32) -> Outcome {
        Outcome {
            id,
            version_id: Uuid::new_v4(),
            title: title.to_string(),
            description: None,
            is_builtin: builtin,
            order_index: order,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }

    fn component(id: Uuid, outcome_id: Uuid, order: i32) -> Component {
        Component {
            id,
            outcome_id,
            slug: format!("c-{order}"),
            r#type: "html_injection".to_string(),
            config: serde_json::json!({"type": "html_injection"}),
            placement: Placement::Inline,
            order_index: order,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }

    #[test]
    fn assemble_groups_components_under_their_outcomes() {
        let o1 = Uuid::new_v4();
        let o2 = Uuid::new_v4();
        let outcomes = vec![outcome(o1, "A", true, 0), outcome(o2, "B", false, 1)];
        let components = vec![
            component(Uuid::new_v4(), o1, 0),
            component(Uuid::new_v4(), o2, 0),
            component(Uuid::new_v4(), o2, 1),
        ];

        let assembled = assemble_active_outcomes(outcomes, components);

        assert_eq!(assembled.len(), 2);
        assert_eq!(assembled[0].title, "A");
        assert!(assembled[0].is_builtin);
        assert_eq!(assembled[0].components.len(), 1);
        assert_eq!(assembled[1].title, "B");
        assert_eq!(assembled[1].components.len(), 2);
        // Preserves outcome ordering.
        assert_eq!(assembled[0].order_index, 0);
        assert_eq!(assembled[1].order_index, 1);
    }

    #[test]
    fn assemble_outcome_with_no_components_yields_empty_vec() {
        let o1 = Uuid::new_v4();
        let assembled = assemble_active_outcomes(vec![outcome(o1, "lonely", false, 0)], vec![]);
        assert_eq!(assembled.len(), 1);
        assert!(assembled[0].components.is_empty());
    }

    #[test]
    fn to_read_parses_empty_rule_graph_and_applicability() {
        let v = Version {
            id: Uuid::new_v4(),
            feature_id: "dn-article".to_string(),
            version_number: 1,
            description: Some("first".to_string()),
            status: VersionStatus::Draft,
            rule_graph: serde_json::to_value(RuleGraph::default()).unwrap(),
            applicability: serde_json::json!({ "html_selector": "#paywall" }),
            created_by: "system".to_string(),
            last_updated_by: "system".to_string(),
            last_updated_at: Utc::now(),
            created_at: Utc::now(),
        };
        let read = to_read(v).expect("parse");
        assert_eq!(read.version_number, 1);
        assert!(read.rule_graph.anonymous.nodes.is_empty());
        // Applicability round-trips out of the stored JSONB.
        assert_eq!(
            read.applicability.html_selector.as_deref(),
            Some("#paywall")
        );
        assert!(read.applicability.json_selector.is_none());
    }

    #[test]
    fn to_read_defaults_applicability_on_null() {
        // A null/missing applicability JSONB parses to the default `{}`.
        let v = Version {
            id: Uuid::new_v4(),
            feature_id: "dn-article".to_string(),
            version_number: 1,
            description: None,
            status: VersionStatus::Draft,
            rule_graph: serde_json::to_value(RuleGraph::default()).unwrap(),
            applicability: serde_json::Value::Null,
            created_by: "system".to_string(),
            last_updated_by: "system".to_string(),
            last_updated_at: Utc::now(),
            created_at: Utc::now(),
        };
        let read = to_read(v).expect("parse");
        assert_eq!(read.applicability, Applicability::default());
    }

    #[test]
    fn to_read_rejects_corrupt_rule_graph() {
        let v = Version {
            id: Uuid::new_v4(),
            feature_id: "dn-article".to_string(),
            version_number: 1,
            description: None,
            status: VersionStatus::Draft,
            rule_graph: serde_json::json!({"not": "a graph"}),
            applicability: serde_json::json!({}),
            created_by: "system".to_string(),
            last_updated_by: "system".to_string(),
            last_updated_at: Utc::now(),
            created_at: Utc::now(),
        };
        let err = to_read(v).unwrap_err();
        assert!(matches!(err, AppError::Internal(_)));
    }

    #[test]
    fn to_summary_drops_rule_graph() {
        let v = Version {
            id: Uuid::new_v4(),
            feature_id: "dn-article".to_string(),
            version_number: 3,
            description: None,
            status: VersionStatus::Live,
            rule_graph: serde_json::json!({}),
            applicability: serde_json::json!({}),
            created_by: "system".to_string(),
            last_updated_by: "alice".to_string(),
            last_updated_at: Utc::now(),
            created_at: Utc::now(),
        };
        let summary = to_summary(v);
        assert_eq!(summary.version_number, 3);
        assert_eq!(summary.status, VersionStatus::Live);
        assert_eq!(summary.last_updated_by, "alice");
    }

    #[test]
    fn validate_applicability_rejects_overlong_selector() {
        let bad = Applicability {
            html_selector: Some("a".repeat(501)),
            json_selector: None,
        };
        let err = validate_applicability(&bad).unwrap_err();
        assert!(matches!(err, AppError::Validation { .. }));
        assert!(validate_applicability(&Applicability::default()).is_ok());
    }
}
