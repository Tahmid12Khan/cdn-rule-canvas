//! Site data-access (RUNTIME SQLx only). Returns [`Site`] models; never serde
//! DTOs. All table refs are schema-qualified (`rre.sites`).

use sqlx::PgPool;

use crate::models::site::Site;

/// Column list shared by all `SELECT`/`RETURNING` clauses.
const COLS: &str = "slug, name, source_protocol, source_host, source_port, \
     dest_protocol, dest_host, dest_port, headers, created_at, updated_at";

/// Insert a new site. The caller maps a unique-violation (pg `23505`) to a
/// conflict; this repository surfaces the raw [`sqlx::Error`].
#[allow(clippy::too_many_arguments)]
pub async fn insert(
    pool: &PgPool,
    slug: &str,
    name: &str,
    source_protocol: &str,
    source_host: &str,
    source_port: i32,
    dest_protocol: &str,
    dest_host: &str,
    dest_port: i32,
    headers: &serde_json::Value,
) -> Result<Site, sqlx::Error> {
    let sql = format!(
        "INSERT INTO rre.sites \
            (slug, name, source_protocol, source_host, source_port, \
             dest_protocol, dest_host, dest_port, headers) \
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9) RETURNING {COLS}"
    );
    sqlx::query_as::<_, Site>(sqlx::AssertSqlSafe(sql))
        .bind(slug)
        .bind(name)
        .bind(source_protocol)
        .bind(source_host)
        .bind(source_port)
        .bind(dest_protocol)
        .bind(dest_host)
        .bind(dest_port)
        .bind(headers)
        .fetch_one(pool)
        .await
}

/// Find a site by slug. `None` when absent.
pub async fn get(pool: &PgPool, slug: &str) -> Result<Option<Site>, sqlx::Error> {
    let sql = format!("SELECT {COLS} FROM rre.sites WHERE slug = $1");
    sqlx::query_as::<_, Site>(sqlx::AssertSqlSafe(sql))
        .bind(slug)
        .fetch_optional(pool)
        .await
}

/// List sites ordered by `created_at DESC, slug ASC`, paginated. An optional
/// `q` filters by case-insensitive `name ILIKE '%q%'` (for the site picker).
pub async fn list_paged(
    pool: &PgPool,
    limit: i64,
    offset: i64,
    q: Option<&str>,
) -> Result<Vec<Site>, sqlx::Error> {
    let sql = format!(
        "SELECT {COLS} FROM rre.sites \
         WHERE ($3::text IS NULL OR name ILIKE '%' || $3 || '%') \
         ORDER BY created_at DESC, slug ASC LIMIT $1 OFFSET $2"
    );
    sqlx::query_as::<_, Site>(sqlx::AssertSqlSafe(sql))
        .bind(limit)
        .bind(offset)
        .bind(q)
        .fetch_all(pool)
        .await
}

/// Count sites matching the optional `q` filter (for the pagination envelope).
pub async fn count(pool: &PgPool, q: Option<&str>) -> Result<i64, sqlx::Error> {
    sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM rre.sites \
         WHERE ($1::text IS NULL OR name ILIKE '%' || $1 || '%')",
    )
    .bind(q)
    .fetch_one(pool)
    .await
}

/// Partially update a site, bumping `updated_at`. Each `None` field is left
/// unchanged (COALESCE keeps the existing column value). Returns the updated
/// row, or `None` if no site with `slug` exists. The caller maps a
/// unique-violation (pg `23505`) to a conflict.
#[allow(clippy::too_many_arguments)]
pub async fn update(
    pool: &PgPool,
    slug: &str,
    name: Option<&str>,
    source_protocol: Option<&str>,
    source_host: Option<&str>,
    source_port: Option<i32>,
    dest_protocol: Option<&str>,
    dest_host: Option<&str>,
    dest_port: Option<i32>,
    headers: Option<&serde_json::Value>,
) -> Result<Option<Site>, sqlx::Error> {
    let sql = format!(
        "UPDATE rre.sites SET \
            name = COALESCE($2, name), \
            source_protocol = COALESCE($3, source_protocol), \
            source_host = COALESCE($4, source_host), \
            source_port = COALESCE($5, source_port), \
            dest_protocol = COALESCE($6, dest_protocol), \
            dest_host = COALESCE($7, dest_host), \
            dest_port = COALESCE($8, dest_port), \
            headers = COALESCE($9, headers), \
            updated_at = now() \
         WHERE slug = $1 RETURNING {COLS}"
    );
    sqlx::query_as::<_, Site>(sqlx::AssertSqlSafe(sql))
        .bind(slug)
        .bind(name)
        .bind(source_protocol)
        .bind(source_host)
        .bind(source_port)
        .bind(dest_protocol)
        .bind(dest_host)
        .bind(dest_port)
        .bind(headers)
        .fetch_optional(pool)
        .await
}

/// Delete a site by slug. Returns the number of rows deleted (0 when absent).
pub async fn delete(pool: &PgPool, slug: &str) -> Result<u64, sqlx::Error> {
    let res = sqlx::query("DELETE FROM rre.sites WHERE slug = $1")
        .bind(slug)
        .execute(pool)
        .await?;
    Ok(res.rows_affected())
}
