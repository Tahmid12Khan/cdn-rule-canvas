//! Test-preset data-access (RUNTIME SQLx only). Returns [`TestPreset`] models;
//! never serde DTOs. All table refs are schema-qualified (`rre.test_presets`).

use sqlx::PgPool;

use crate::models::test_preset::TestPreset;

/// Column list shared by all `SELECT`/`RETURNING` clauses.
const COLS: &str = "slug, name, kind, payload, created_at, updated_at";

/// Insert a new test preset. The caller maps a unique-violation (pg `23505`) to
/// a conflict; this repository surfaces the raw [`sqlx::Error`].
pub async fn insert(
    pool: &PgPool,
    slug: &str,
    name: &str,
    kind: &str,
    payload: &serde_json::Value,
) -> Result<TestPreset, sqlx::Error> {
    let sql = format!(
        "INSERT INTO rre.test_presets (slug, name, kind, payload) \
         VALUES ($1, $2, $3, $4) RETURNING {COLS}"
    );
    sqlx::query_as::<_, TestPreset>(sqlx::AssertSqlSafe(sql))
        .bind(slug)
        .bind(name)
        .bind(kind)
        .bind(payload)
        .fetch_one(pool)
        .await
}

/// Find a preset by slug. `None` when absent.
pub async fn get(pool: &PgPool, slug: &str) -> Result<Option<TestPreset>, sqlx::Error> {
    let sql = format!("SELECT {COLS} FROM rre.test_presets WHERE slug = $1");
    sqlx::query_as::<_, TestPreset>(sqlx::AssertSqlSafe(sql))
        .bind(slug)
        .fetch_optional(pool)
        .await
}

/// List presets ordered by `created_at DESC, slug ASC`, paginated. An optional
/// `q` filters by case-insensitive `name ILIKE '%q%'`; an optional `kind`
/// filters to a single kind.
pub async fn list_paged(
    pool: &PgPool,
    limit: i64,
    offset: i64,
    q: Option<&str>,
    kind: Option<&str>,
) -> Result<Vec<TestPreset>, sqlx::Error> {
    let sql = format!(
        "SELECT {COLS} FROM rre.test_presets \
         WHERE ($3::text IS NULL OR name ILIKE '%' || $3 || '%') \
           AND ($4::text IS NULL OR kind = $4) \
         ORDER BY created_at DESC, slug ASC LIMIT $1 OFFSET $2"
    );
    sqlx::query_as::<_, TestPreset>(sqlx::AssertSqlSafe(sql))
        .bind(limit)
        .bind(offset)
        .bind(q)
        .bind(kind)
        .fetch_all(pool)
        .await
}

/// Count presets matching the optional `q` and `kind` filters (for the
/// pagination envelope).
pub async fn count(pool: &PgPool, q: Option<&str>, kind: Option<&str>) -> Result<i64, sqlx::Error> {
    sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM rre.test_presets \
         WHERE ($1::text IS NULL OR name ILIKE '%' || $1 || '%') \
           AND ($2::text IS NULL OR kind = $2)",
    )
    .bind(q)
    .bind(kind)
    .fetch_one(pool)
    .await
}

/// Partially update a preset, bumping `updated_at`. Each `None` field is left
/// unchanged (COALESCE keeps the existing column value); `slug` and `kind` are
/// NOT updatable. Returns the updated row, or `None` if no preset with `slug`
/// exists. The caller maps a unique-violation (pg `23505`) to a conflict.
pub async fn update(
    pool: &PgPool,
    slug: &str,
    name: Option<&str>,
    payload: Option<&serde_json::Value>,
) -> Result<Option<TestPreset>, sqlx::Error> {
    let sql = format!(
        "UPDATE rre.test_presets SET \
            name = COALESCE($2, name), \
            payload = COALESCE($3, payload), \
            updated_at = now() \
         WHERE slug = $1 RETURNING {COLS}"
    );
    sqlx::query_as::<_, TestPreset>(sqlx::AssertSqlSafe(sql))
        .bind(slug)
        .bind(name)
        .bind(payload)
        .fetch_optional(pool)
        .await
}

/// Delete a preset by slug. Returns the number of rows deleted (0 when absent).
pub async fn delete(pool: &PgPool, slug: &str) -> Result<u64, sqlx::Error> {
    let res = sqlx::query("DELETE FROM rre.test_presets WHERE slug = $1")
        .bind(slug)
        .execute(pool)
        .await?;
    Ok(res.rows_affected())
}
