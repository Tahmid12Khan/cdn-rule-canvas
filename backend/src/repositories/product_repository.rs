//! Product data-access (RUNTIME SQLx only). Returns [`Product`] models; never
//! serde DTOs. All table refs are schema-qualified (`rre.products`).

use sqlx::PgPool;

use crate::models::product::Product;

/// Column list shared by all `SELECT`/`RETURNING` clauses.
const COLS: &str = "label, name, description, created_at, updated_at";

/// Insert a new product. The caller maps a unique-violation (pg `23505`) to a
/// conflict; this repository surfaces the raw [`sqlx::Error`].
pub async fn insert(
    pool: &PgPool,
    label: &str,
    name: &str,
    description: Option<&str>,
) -> Result<Product, sqlx::Error> {
    let sql = format!(
        "INSERT INTO rre.products (label, name, description) VALUES ($1, $2, $3) RETURNING {COLS}"
    );
    sqlx::query_as::<_, Product>(sqlx::AssertSqlSafe(sql))
        .bind(label)
        .bind(name)
        .bind(description)
        .fetch_one(pool)
        .await
}

/// Find a product by label. `None` when absent.
pub async fn get(pool: &PgPool, label: &str) -> Result<Option<Product>, sqlx::Error> {
    let sql = format!("SELECT {COLS} FROM rre.products WHERE label = $1");
    sqlx::query_as::<_, Product>(sqlx::AssertSqlSafe(sql))
        .bind(label)
        .fetch_optional(pool)
        .await
}

/// List products ordered by `created_at DESC, label ASC`, paginated. An
/// optional `q` filters by case-insensitive `name ILIKE '%q%'`.
pub async fn list_paged(
    pool: &PgPool,
    limit: i64,
    offset: i64,
    q: Option<&str>,
) -> Result<Vec<Product>, sqlx::Error> {
    let sql = format!(
        "SELECT {COLS} FROM rre.products \
         WHERE ($3::text IS NULL OR name ILIKE '%' || $3 || '%') \
         ORDER BY created_at DESC, label ASC LIMIT $1 OFFSET $2"
    );
    sqlx::query_as::<_, Product>(sqlx::AssertSqlSafe(sql))
        .bind(limit)
        .bind(offset)
        .bind(q)
        .fetch_all(pool)
        .await
}

/// Count products matching the optional `q` filter (for the pagination envelope).
pub async fn count(pool: &PgPool, q: Option<&str>) -> Result<i64, sqlx::Error> {
    sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM rre.products WHERE ($1::text IS NULL OR name ILIKE '%' || $1 || '%')",
    )
    .bind(q)
    .fetch_one(pool)
    .await
}

/// Partially update a product, bumping `updated_at`. Each `None` field is left
/// unchanged (COALESCE keeps the existing column value). Returns the updated
/// row, or `None` if no product with `label` exists. The caller maps a
/// unique-violation (pg `23505`) to a conflict.
///
/// `clear_description` is always `false` from the service today — the DTO's
/// `description: None` means "don't touch it", matching `SiteUpdate`'s simpler
/// fields (no clear-to-null distinction needed there either).
pub async fn update(
    pool: &PgPool,
    label: &str,
    name: Option<&str>,
    description: Option<&str>,
    clear_description: bool,
) -> Result<Option<Product>, sqlx::Error> {
    let sql = format!(
        "UPDATE rre.products SET \
            name = COALESCE($2, name), \
            description = CASE WHEN $4 THEN NULL ELSE COALESCE($3, description) END, \
            updated_at = now() \
         WHERE label = $1 RETURNING {COLS}"
    );
    sqlx::query_as::<_, Product>(sqlx::AssertSqlSafe(sql))
        .bind(label)
        .bind(name)
        .bind(description)
        .bind(clear_description)
        .fetch_optional(pool)
        .await
}

/// Delete a product by label. Returns the number of rows deleted (0 when absent).
pub async fn delete(pool: &PgPool, label: &str) -> Result<u64, sqlx::Error> {
    let res = sqlx::query("DELETE FROM rre.products WHERE label = $1")
        .bind(label)
        .execute(pool)
        .await?;
    Ok(res.rows_affected())
}
