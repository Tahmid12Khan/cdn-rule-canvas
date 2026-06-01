//! Feature data-access (RUNTIME SQLx only). Returns [`Feature`] models; never
//! serde DTOs. All table refs are schema-qualified (`rre.features`).

use sqlx::PgPool;

use crate::models::{enums::FeatureType, feature::Feature};

/// Column list shared by all `SELECT`/`RETURNING` clauses. `type` is quoted
/// because it is a SQL keyword; sqlx `FromRow` maps the column `type` onto the
/// model's `r#type` field.
const COLS: &str =
    r#"id, name, "type", staging_version_id, live_version_id, created_at, updated_at"#;

/// Insert a new feature. The caller maps a unique-violation (pg `23505`) to a
/// slug conflict; this repository surfaces the raw [`sqlx::Error`].
pub async fn insert(
    pool: &PgPool,
    id: &str,
    name: &str,
    r#type: FeatureType,
) -> Result<Feature, sqlx::Error> {
    let sql = format!(
        r#"INSERT INTO rre.features (id, name, "type") VALUES ($1, $2, $3) RETURNING {COLS}"#
    );
    sqlx::query_as::<_, Feature>(&sql)
        .bind(id)
        .bind(name)
        .bind(r#type)
        .fetch_one(pool)
        .await
}

/// Find a feature by slug. `None` when absent.
pub async fn find(pool: &PgPool, id: &str) -> Result<Option<Feature>, sqlx::Error> {
    let sql = format!("SELECT {COLS} FROM rre.features WHERE id = $1");
    sqlx::query_as::<_, Feature>(&sql)
        .bind(id)
        .fetch_optional(pool)
        .await
}

/// List features ordered by `created_at DESC`, paginated.
pub async fn list_paged(
    pool: &PgPool,
    limit: i64,
    offset: i64,
) -> Result<Vec<Feature>, sqlx::Error> {
    let sql = format!(
        "SELECT {COLS} FROM rre.features ORDER BY created_at DESC, id ASC LIMIT $1 OFFSET $2"
    );
    sqlx::query_as::<_, Feature>(&sql)
        .bind(limit)
        .bind(offset)
        .fetch_all(pool)
        .await
}

/// Count all features (for the pagination envelope).
pub async fn count(pool: &PgPool) -> Result<i64, sqlx::Error> {
    sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM rre.features")
        .fetch_one(pool)
        .await
}

/// Update a feature's name and bump `updated_at`. Returns the updated row, or
/// `None` if no feature with `id` exists.
pub async fn update_name(
    pool: &PgPool,
    id: &str,
    name: &str,
) -> Result<Option<Feature>, sqlx::Error> {
    let sql = format!(
        "UPDATE rre.features SET name = $2, updated_at = now() WHERE id = $1 RETURNING {COLS}"
    );
    sqlx::query_as::<_, Feature>(&sql)
        .bind(id)
        .bind(name)
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
