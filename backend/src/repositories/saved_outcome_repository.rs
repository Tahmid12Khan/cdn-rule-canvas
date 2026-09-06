//! Saved-outcome data-access (RUNTIME SQLx only).

use sqlx::PgPool;
use uuid::Uuid;

use crate::models::saved_outcome::SavedOutcome;

const COLS: &str =
    "id, slug, name, component_id, version_number, variables, created_at, updated_at";

#[allow(clippy::too_many_arguments)]
pub async fn insert(
    pool: &PgPool,
    slug: &str,
    name: &str,
    component_id: Uuid,
    version_number: Option<i32>,
    variables: &serde_json::Value,
) -> Result<SavedOutcome, sqlx::Error> {
    let sql = format!(
        "INSERT INTO rre.saved_outcomes (slug, name, component_id, version_number, variables) \
         VALUES ($1, $2, $3, $4, $5) RETURNING {COLS}"
    );
    sqlx::query_as::<_, SavedOutcome>(sqlx::AssertSqlSafe(sql))
        .bind(slug)
        .bind(name)
        .bind(component_id)
        .bind(version_number)
        .bind(variables)
        .fetch_one(pool)
        .await
}

pub async fn get(pool: &PgPool, id: Uuid) -> Result<Option<SavedOutcome>, sqlx::Error> {
    let sql = format!("SELECT {COLS} FROM rre.saved_outcomes WHERE id = $1");
    sqlx::query_as::<_, SavedOutcome>(sqlx::AssertSqlSafe(sql))
        .bind(id)
        .fetch_optional(pool)
        .await
}

pub async fn list_paged(
    pool: &PgPool,
    limit: i64,
    offset: i64,
    q: Option<&str>,
) -> Result<Vec<SavedOutcome>, sqlx::Error> {
    let sql = format!(
        "SELECT {COLS} FROM rre.saved_outcomes \
         WHERE ($3::text IS NULL OR name ILIKE '%' || $3 || '%') \
         ORDER BY created_at DESC, slug ASC LIMIT $1 OFFSET $2"
    );
    sqlx::query_as::<_, SavedOutcome>(sqlx::AssertSqlSafe(sql))
        .bind(limit)
        .bind(offset)
        .bind(q)
        .fetch_all(pool)
        .await
}

pub async fn count(pool: &PgPool, q: Option<&str>) -> Result<i64, sqlx::Error> {
    sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM rre.saved_outcomes WHERE ($1::text IS NULL OR name ILIKE '%' || $1 || '%')",
    )
    .bind(q)
    .fetch_one(pool)
    .await
}

/// `version_number`: `None` = leave unchanged; the CALLER resolves the
/// double-Option (`Some(None)` = clear, `Some(Some(n))` = pin, `None` = keep)
/// into the plain `Option<Option<i32>>` shown here before calling this repo fn,
/// since SQL COALESCE can't express three states over one bind — instead this
/// takes an explicit `set_version: bool` + `version_number: Option<i32>` pair.
pub async fn update(
    pool: &PgPool,
    id: Uuid,
    name: Option<&str>,
    set_version: bool,
    version_number: Option<i32>,
    variables: Option<&serde_json::Value>,
) -> Result<Option<SavedOutcome>, sqlx::Error> {
    let sql = format!(
        "UPDATE rre.saved_outcomes SET \
            name = COALESCE($2, name), \
            version_number = CASE WHEN $3 THEN $4 ELSE version_number END, \
            variables = COALESCE($5, variables), \
            updated_at = now() \
         WHERE id = $1 RETURNING {COLS}"
    );
    sqlx::query_as::<_, SavedOutcome>(sqlx::AssertSqlSafe(sql))
        .bind(id)
        .bind(name)
        .bind(set_version)
        .bind(version_number)
        .bind(variables)
        .fetch_optional(pool)
        .await
}

pub async fn delete(pool: &PgPool, id: Uuid) -> Result<u64, sqlx::Error> {
    let res = sqlx::query("DELETE FROM rre.saved_outcomes WHERE id = $1")
        .bind(id)
        .execute(pool)
        .await?;
    Ok(res.rows_affected())
}

/// All ids currently in use (for `saved_outcome_ref_exists` validation).
pub async fn list_all_ids(pool: &PgPool) -> Result<Vec<Uuid>, sqlx::Error> {
    sqlx::query_scalar::<_, Uuid>("SELECT id FROM rre.saved_outcomes")
        .fetch_all(pool)
        .await
}
