//! Component-template data access (RUNTIME SQLx only). Returns
//! [`ComponentTemplate`] models; never serde DTOs. All table refs are
//! schema-qualified (`rre.component_templates`). The service owns transactions —
//! write methods accept a generic executor so they compose inside a
//! `pool.begin()` transaction or run against `&PgPool` directly (mirrors
//! `outcome_repository`).

use sqlx::{PgPool, Postgres};
use uuid::Uuid;

use crate::models::{component_template::ComponentTemplate, enums::DefaultMode};

const COLS: &str = "id, slug, name, description, default_mode, default_version_id, \
     created_at, updated_at";

/// Insert a new component (the caller supplies a pre-generated id). The default
/// pointer is wired separately (`set_default`) after the v1 row exists.
pub async fn insert<'e, E>(
    exec: E,
    id: Uuid,
    slug: &str,
    name: &str,
    description: Option<&str>,
) -> Result<ComponentTemplate, sqlx::Error>
where
    E: sqlx::Executor<'e, Database = Postgres>,
{
    let sql = format!(
        "INSERT INTO rre.component_templates (id, slug, name, description) \
         VALUES ($1, $2, $3, $4) RETURNING {COLS}"
    );
    sqlx::query_as::<_, ComponentTemplate>(sqlx::AssertSqlSafe(sql))
        .bind(id)
        .bind(slug)
        .bind(name)
        .bind(description)
        .fetch_one(exec)
        .await
}

/// Fetch a component by id. `None` when absent.
pub async fn find<'e, E>(exec: E, id: Uuid) -> Result<Option<ComponentTemplate>, sqlx::Error>
where
    E: sqlx::Executor<'e, Database = Postgres>,
{
    let sql = format!("SELECT {COLS} FROM rre.component_templates WHERE id = $1");
    sqlx::query_as::<_, ComponentTemplate>(sqlx::AssertSqlSafe(sql))
        .bind(id)
        .fetch_optional(exec)
        .await
}

/// Fetch a component by slug. `None` when absent.
pub async fn find_by_slug<'e, E>(
    exec: E,
    slug: &str,
) -> Result<Option<ComponentTemplate>, sqlx::Error>
where
    E: sqlx::Executor<'e, Database = Postgres>,
{
    let sql = format!("SELECT {COLS} FROM rre.component_templates WHERE slug = $1");
    sqlx::query_as::<_, ComponentTemplate>(sqlx::AssertSqlSafe(sql))
        .bind(slug)
        .fetch_optional(exec)
        .await
}

/// Update the metadata (`name`/`description`), bumping `updated_at`. `None`
/// fields are left untouched. The caller maps a unique-violation (`23505`).
pub async fn update_meta<'e, E>(
    exec: E,
    id: Uuid,
    name: Option<&str>,
    description: Option<Option<&str>>,
) -> Result<Option<ComponentTemplate>, sqlx::Error>
where
    E: sqlx::Executor<'e, Database = Postgres>,
{
    // `description` is a nested Option: outer None = "don't touch", inner None =
    // "set NULL".
    let touch_description = description.is_some();
    let description_value = description.flatten();

    let sql = format!(
        "UPDATE rre.component_templates SET \
            name        = COALESCE($2, name), \
            description = CASE WHEN $3 THEN $4 ELSE description END, \
            updated_at  = now() \
         WHERE id = $1 RETURNING {COLS}"
    );
    sqlx::query_as::<_, ComponentTemplate>(sqlx::AssertSqlSafe(sql))
        .bind(id)
        .bind(name)
        .bind(touch_description)
        .bind(description_value)
        .fetch_optional(exec)
        .await
}

/// Set the default pointer: `default_mode` + `default_version_id` (NULL clears
/// the pin). Bumps `updated_at`.
pub async fn set_default<'e, E>(
    exec: E,
    id: Uuid,
    default_mode: DefaultMode,
    default_version_id: Option<Uuid>,
) -> Result<ComponentTemplate, sqlx::Error>
where
    E: sqlx::Executor<'e, Database = Postgres>,
{
    let sql = format!(
        "UPDATE rre.component_templates SET \
            default_mode = $2, default_version_id = $3, updated_at = now() \
         WHERE id = $1 RETURNING {COLS}"
    );
    sqlx::query_as::<_, ComponentTemplate>(sqlx::AssertSqlSafe(sql))
        .bind(id)
        .bind(default_mode)
        .bind(default_version_id)
        .fetch_one(exec)
        .await
}

/// Delete a component by id (cascades its versions). Returns rows affected.
pub async fn delete<'e, E>(exec: E, id: Uuid) -> Result<u64, sqlx::Error>
where
    E: sqlx::Executor<'e, Database = Postgres>,
{
    sqlx::query("DELETE FROM rre.component_templates WHERE id = $1")
        .bind(id)
        .execute(exec)
        .await
        .map(|r| r.rows_affected())
}

/// List components ordered by `created_at DESC, slug ASC`, paginated. An optional
/// `q` filters by case-insensitive `name ILIKE '%q%'`.
pub async fn list_paged(
    pool: &PgPool,
    limit: i64,
    offset: i64,
    q: Option<&str>,
) -> Result<Vec<ComponentTemplate>, sqlx::Error> {
    let sql = format!(
        "SELECT {COLS} FROM rre.component_templates \
         WHERE ($3::text IS NULL OR name ILIKE '%' || $3 || '%') \
         ORDER BY created_at DESC, slug ASC LIMIT $1 OFFSET $2"
    );
    sqlx::query_as::<_, ComponentTemplate>(sqlx::AssertSqlSafe(sql))
        .bind(limit)
        .bind(offset)
        .bind(q)
        .fetch_all(pool)
        .await
}

/// Count components matching the optional `q` filter (pagination envelope).
pub async fn count(pool: &PgPool, q: Option<&str>) -> Result<i64, sqlx::Error> {
    sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM rre.component_templates \
         WHERE ($1::text IS NULL OR name ILIKE '%' || $1 || '%')",
    )
    .bind(q)
    .fetch_one(pool)
    .await
}
