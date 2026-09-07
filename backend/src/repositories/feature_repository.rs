//! Feature data-access (RUNTIME SQLx only). Returns [`Feature`] models; never
//! serde DTOs. All table refs are schema-qualified (`rre.features`).

use sqlx::PgPool;

use crate::models::{enums::FeatureType, feature::Feature};

/// Column list shared by all `SELECT`/`RETURNING` clauses. `type` is quoted
/// because it is a SQL keyword; sqlx `FromRow` maps the column `type` onto the
/// model's `r#type` field.
const COLS: &str = r#"id, name, "type", execution_order, staging_version_id, live_version_id, created_at, updated_at"#;

/// Insert a new feature at an explicit `execution_order`. The caller maps a
/// unique-violation (pg `23505`) to either a slug conflict or an
/// execution-order conflict; this repository surfaces the raw [`sqlx::Error`].
pub async fn insert(
    pool: &PgPool,
    id: &str,
    name: &str,
    r#type: FeatureType,
    execution_order: i32,
) -> Result<Feature, sqlx::Error> {
    let sql = format!(
        r#"INSERT INTO rre.features (id, name, "type", execution_order) VALUES ($1, $2, $3, $4) RETURNING {COLS}"#
    );
    sqlx::query_as::<_, Feature>(sqlx::AssertSqlSafe(sql))
        .bind(id)
        .bind(name)
        .bind(r#type)
        .bind(execution_order)
        .fetch_one(pool)
        .await
}

/// The next free execution order for a type = `MAX(execution_order) + 1` over
/// rows of that type, or `1` when the type has no rows yet.
pub async fn next_execution_order(pool: &PgPool, r#type: FeatureType) -> Result<i32, sqlx::Error> {
    sqlx::query_scalar::<_, i32>(
        r#"SELECT COALESCE(MAX(execution_order), 0) + 1 FROM rre.features WHERE "type" = $1"#,
    )
    .bind(r#type)
    .fetch_one(pool)
    .await
}

/// Find a feature by slug. `None` when absent.
pub async fn find(pool: &PgPool, id: &str) -> Result<Option<Feature>, sqlx::Error> {
    let sql = format!("SELECT {COLS} FROM rre.features WHERE id = $1");
    sqlx::query_as::<_, Feature>(sqlx::AssertSqlSafe(sql))
        .bind(id)
        .fetch_optional(pool)
        .await
}

/// List features ordered by execution priority — `type ASC, execution_order
/// ASC` (lowest runs first) — paginated.
pub async fn list_paged(
    pool: &PgPool,
    limit: i64,
    offset: i64,
) -> Result<Vec<Feature>, sqlx::Error> {
    let sql = format!(
        r#"SELECT {COLS} FROM rre.features ORDER BY "type" ASC, execution_order ASC, id ASC LIMIT $1 OFFSET $2"#
    );
    sqlx::query_as::<_, Feature>(sqlx::AssertSqlSafe(sql))
        .bind(limit)
        .bind(offset)
        .fetch_all(pool)
        .await
}

/// Every feature in execution-priority order — `type ASC, execution_order ASC`
/// — UNPAGINATED. The edge-bundle exporter needs the complete, ordered set in
/// one read: the bundle's `features` array order IS the order a host runs them
/// in, so a page boundary would silently drop rules at the edge.
///
/// `rre.feature_type` is declared `('html', 'json')`, so `"type" ASC` is html
/// before json — the ordering the bundle contract states.
pub async fn list_all(pool: &PgPool) -> Result<Vec<Feature>, sqlx::Error> {
    let sql = format!(
        r#"SELECT {COLS} FROM rre.features ORDER BY "type" ASC, execution_order ASC, id ASC"#
    );
    sqlx::query_as::<_, Feature>(sqlx::AssertSqlSafe(sql))
        .fetch_all(pool)
        .await
}

/// Count all features (for the pagination envelope).
pub async fn count(pool: &PgPool) -> Result<i64, sqlx::Error> {
    sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM rre.features")
        .fetch_one(pool)
        .await
}

/// Update a feature's `name` and/or `execution_order` (each applied only when
/// `Some`, via `COALESCE` to keep the existing value) and bump `updated_at`.
/// Returns the updated row, or `None` if no feature with `id` exists. A
/// duplicate execution_order within the type surfaces as a unique-violation
/// (pg `23505`) the caller maps to a conflict.
pub async fn update_fields(
    pool: &PgPool,
    id: &str,
    name: Option<&str>,
    execution_order: Option<i32>,
) -> Result<Option<Feature>, sqlx::Error> {
    let sql = format!(
        "UPDATE rre.features \
         SET name = COALESCE($2, name), \
             execution_order = COALESCE($3, execution_order), \
             updated_at = now() \
         WHERE id = $1 RETURNING {COLS}"
    );
    sqlx::query_as::<_, Feature>(sqlx::AssertSqlSafe(sql))
        .bind(id)
        .bind(name)
        .bind(execution_order)
        .fetch_optional(pool)
        .await
}

/// Delete a feature by slug (cascades versions/outcomes/components). Returns the
/// number of rows deleted (0 when absent).
pub async fn delete(pool: &PgPool, id: &str) -> Result<u64, sqlx::Error> {
    let res = sqlx::query("DELETE FROM rre.features WHERE id = $1")
        .bind(id)
        .execute(pool)
        .await?;
    Ok(res.rows_affected())
}
