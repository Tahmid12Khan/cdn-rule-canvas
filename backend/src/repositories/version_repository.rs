//! Version data access (BACKEND CONTRACT §4/§7/§10).
//!
//! RUNTIME SQLx only (`query_as::<_, T>`, `.bind(...)`). Returns `Version`
//! models; never serde DTOs. The service layer owns transactions; methods here
//! accept either `&PgPool` (read paths) or a `&mut PgConnection`/transaction
//! executor (write paths within a unit of work).

use chrono::{DateTime, Utc};
use sqlx::{PgExecutor, PgPool};
use uuid::Uuid;

use crate::models::{
    component::Component, enums::VersionStatus, outcome::Outcome, version::Version,
};

const VERSION_COLUMNS: &str = "id, feature_id, version_number, description, status, \
     rule_graph, applicability, created_by, last_updated_by, last_updated_at, created_at";

/// Whether a feature with the given slug exists.
pub async fn feature_exists<'e, E>(executor: E, feature_id: &str) -> Result<bool, sqlx::Error>
where
    E: PgExecutor<'e>,
{
    sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM rre.features WHERE id = $1)")
        .bind(feature_id)
        .fetch_one(executor)
        .await
}

/// Fetch a feature's `(staging_version_id, live_version_id)` pointers.
pub async fn feature_version_pointers<'e, E>(
    executor: E,
    feature_id: &str,
) -> Result<Option<(Option<Uuid>, Option<Uuid>)>, sqlx::Error>
where
    E: PgExecutor<'e>,
{
    let row: Option<(Option<Uuid>, Option<Uuid>)> = sqlx::query_as(
        "SELECT staging_version_id, live_version_id FROM rre.features WHERE id = $1",
    )
    .bind(feature_id)
    .fetch_optional(executor)
    .await?;
    Ok(row)
}

/// Fetch a version by its UUID.
pub async fn find_by_id<'e, E>(executor: E, id: Uuid) -> Result<Option<Version>, sqlx::Error>
where
    E: PgExecutor<'e>,
{
    let sql = format!("SELECT {VERSION_COLUMNS} FROM rre.versions WHERE id = $1");
    sqlx::query_as::<_, Version>(&sql)
        .bind(id)
        .fetch_optional(executor)
        .await
}

/// Fetch a version by `(feature_id, version_number)`.
pub async fn find_by_number<'e, E>(
    executor: E,
    feature_id: &str,
    version_number: i32,
) -> Result<Option<Version>, sqlx::Error>
where
    E: PgExecutor<'e>,
{
    let sql = format!(
        "SELECT {VERSION_COLUMNS} FROM rre.versions \
         WHERE feature_id = $1 AND version_number = $2"
    );
    sqlx::query_as::<_, Version>(&sql)
        .bind(feature_id)
        .bind(version_number)
        .fetch_optional(executor)
        .await
}

/// Lock the feature's versions and return the current max `version_number`
/// (or 0 when the feature has no versions). Locks selected rows `FOR UPDATE`
/// to serialize concurrent version creation per feature.
pub async fn max_version_number_for_update<'e, E>(
    executor: E,
    feature_id: &str,
) -> Result<i32, sqlx::Error>
where
    E: PgExecutor<'e>,
{
    // Postgres forbids FOR UPDATE alongside an aggregate, so lock the rows in a
    // CTE (no aggregate there) and take MAX() in the outer query.
    let max: Option<i32> = sqlx::query_scalar(
        "WITH locked AS ( \
             SELECT version_number FROM rre.versions \
             WHERE feature_id = $1 FOR UPDATE \
         ) \
         SELECT MAX(version_number) FROM locked",
    )
    .bind(feature_id)
    .fetch_one(executor)
    .await?;
    Ok(max.unwrap_or(0))
}

/// Find the current version with the given status for a feature (used to clone
/// rule_graph from LIVE, and to demote the current LIVE/STAGING on publish).
pub async fn find_by_status<'e, E>(
    executor: E,
    feature_id: &str,
    status: VersionStatus,
) -> Result<Option<Version>, sqlx::Error>
where
    E: PgExecutor<'e>,
{
    let sql = format!(
        "SELECT {VERSION_COLUMNS} FROM rre.versions \
         WHERE feature_id = $1 AND status = $2"
    );
    sqlx::query_as::<_, Version>(&sql)
        .bind(feature_id)
        .bind(status)
        .fetch_optional(executor)
        .await
}

/// Insert a new version row. `version_number` is supplied by the service.
pub async fn insert<'e, E>(
    executor: E,
    feature_id: &str,
    version_number: i32,
    description: Option<&str>,
    status: VersionStatus,
    rule_graph: &serde_json::Value,
    created_by: &str,
) -> Result<Version, sqlx::Error>
where
    E: PgExecutor<'e>,
{
    let sql = format!(
        "INSERT INTO rre.versions \
            (feature_id, version_number, description, status, rule_graph, \
             created_by, last_updated_by) \
         VALUES ($1, $2, $3, $4, $5, $6, $6) \
         RETURNING {VERSION_COLUMNS}"
    );
    sqlx::query_as::<_, Version>(&sql)
        .bind(feature_id)
        .bind(version_number)
        .bind(description)
        .bind(status)
        .bind(rule_graph)
        .bind(created_by)
        .fetch_one(executor)
        .await
}

/// Update a version's description, rule_graph and/or applicability. `None`
/// fields are left untouched (COALESCE keeps the existing value). Bumps
/// `last_updated_*`.
pub async fn update_fields<'e, E>(
    executor: E,
    id: Uuid,
    description: Option<&str>,
    rule_graph: Option<&serde_json::Value>,
    applicability: Option<&serde_json::Value>,
    last_updated_by: &str,
    now: DateTime<Utc>,
) -> Result<Version, sqlx::Error>
where
    E: PgExecutor<'e>,
{
    let sql = format!(
        "UPDATE rre.versions SET \
            description = COALESCE($2, description), \
            rule_graph = COALESCE($3, rule_graph), \
            applicability = COALESCE($4, applicability), \
            last_updated_by = $5, \
            last_updated_at = $6 \
         WHERE id = $1 \
         RETURNING {VERSION_COLUMNS}"
    );
    sqlx::query_as::<_, Version>(&sql)
        .bind(id)
        .bind(description)
        .bind(rule_graph)
        .bind(applicability)
        .bind(last_updated_by)
        .bind(now)
        .fetch_one(executor)
        .await
}

/// Set a version's lifecycle status.
pub async fn update_status<'e, E>(
    executor: E,
    id: Uuid,
    status: VersionStatus,
) -> Result<Version, sqlx::Error>
where
    E: PgExecutor<'e>,
{
    let sql =
        format!("UPDATE rre.versions SET status = $2 WHERE id = $1 RETURNING {VERSION_COLUMNS}");
    sqlx::query_as::<_, Version>(&sql)
        .bind(id)
        .bind(status)
        .fetch_one(executor)
        .await
}

/// Lock a version row `FOR UPDATE` (publish/unpublish serialization).
pub async fn find_by_id_for_update<'e, E>(
    executor: E,
    id: Uuid,
) -> Result<Option<Version>, sqlx::Error>
where
    E: PgExecutor<'e>,
{
    let sql = format!("SELECT {VERSION_COLUMNS} FROM rre.versions WHERE id = $1 FOR UPDATE");
    sqlx::query_as::<_, Version>(&sql)
        .bind(id)
        .fetch_optional(executor)
        .await
}

/// Delete a version by id. Returns the number of rows affected.
pub async fn delete<'e, E>(executor: E, id: Uuid) -> Result<u64, sqlx::Error>
where
    E: PgExecutor<'e>,
{
    let result = sqlx::query("DELETE FROM rre.versions WHERE id = $1")
        .bind(id)
        .execute(executor)
        .await?;
    Ok(result.rows_affected())
}

/// Set the feature's `staging_version_id` pointer (NULL to clear).
pub async fn set_feature_staging<'e, E>(
    executor: E,
    feature_id: &str,
    version_id: Option<Uuid>,
) -> Result<(), sqlx::Error>
where
    E: PgExecutor<'e>,
{
    sqlx::query(
        "UPDATE rre.features SET staging_version_id = $2, updated_at = now() WHERE id = $1",
    )
    .bind(feature_id)
    .bind(version_id)
    .execute(executor)
    .await?;
    Ok(())
}

/// Set the feature's `live_version_id` pointer (NULL to clear).
pub async fn set_feature_live<'e, E>(
    executor: E,
    feature_id: &str,
    version_id: Option<Uuid>,
) -> Result<(), sqlx::Error>
where
    E: PgExecutor<'e>,
{
    sqlx::query("UPDATE rre.features SET live_version_id = $2, updated_at = now() WHERE id = $1")
        .bind(feature_id)
        .bind(version_id)
        .execute(executor)
        .await?;
    Ok(())
}

/// List versions for a feature with optional status + description search,
/// paginated, newest version_number first.
pub async fn list_paged(
    pool: &PgPool,
    feature_id: &str,
    status: Option<VersionStatus>,
    search: Option<&str>,
    limit: i64,
    offset: i64,
) -> Result<Vec<Version>, sqlx::Error> {
    let sql = format!(
        "SELECT {VERSION_COLUMNS} FROM rre.versions \
         WHERE feature_id = $1 \
           AND ($2::rre.version_status IS NULL OR status = $2) \
           AND ($3::text IS NULL OR description ILIKE '%' || $3 || '%') \
         ORDER BY version_number DESC \
         LIMIT $4 OFFSET $5"
    );
    sqlx::query_as::<_, Version>(&sql)
        .bind(feature_id)
        .bind(status)
        .bind(search)
        .bind(limit)
        .bind(offset)
        .fetch_all(pool)
        .await
}

/// Count versions matching the same filters as [`list_paged`].
pub async fn count(
    pool: &PgPool,
    feature_id: &str,
    status: Option<VersionStatus>,
    search: Option<&str>,
) -> Result<i64, sqlx::Error> {
    sqlx::query_scalar(
        "SELECT COUNT(*) FROM rre.versions \
         WHERE feature_id = $1 \
           AND ($2::rre.version_status IS NULL OR status = $2) \
           AND ($3::text IS NULL OR description ILIKE '%' || $3 || '%')",
    )
    .bind(feature_id)
    .bind(status)
    .bind(search)
    .fetch_one(pool)
    .await
}

// --- Active-version read path (BACKEND CONTRACT §10/§11) ---

const OUTCOME_COLUMNS: &str = "id, version_id, title, description, is_builtin, \
     order_index, created_at, updated_at";
const COMPONENT_COLUMNS: &str = "id, outcome_id, slug, type, config, placement, \
     order_index, created_at, updated_at";

/// All outcomes for a version, ordered by `order_index ASC`.
pub async fn list_outcomes(pool: &PgPool, version_id: Uuid) -> Result<Vec<Outcome>, sqlx::Error> {
    let sql = format!(
        "SELECT {OUTCOME_COLUMNS} FROM rre.outcomes \
         WHERE version_id = $1 ORDER BY order_index ASC, created_at ASC"
    );
    sqlx::query_as::<_, Outcome>(&sql)
        .bind(version_id)
        .fetch_all(pool)
        .await
}

/// All components for the given outcome ids, ordered by `order_index ASC`.
/// Single query (no N+1) — group app-side by `outcome_id`.
pub async fn list_components_for_outcomes(
    pool: &PgPool,
    outcome_ids: &[Uuid],
) -> Result<Vec<Component>, sqlx::Error> {
    let sql = format!(
        "SELECT {COMPONENT_COLUMNS} FROM rre.components \
         WHERE outcome_id = ANY($1) ORDER BY order_index ASC, created_at ASC"
    );
    sqlx::query_as::<_, Component>(&sql)
        .bind(outcome_ids)
        .fetch_all(pool)
        .await
}
