//! Component-template-version data access (RUNTIME SQLx only). Returns
//! [`ComponentTemplateVersion`] models; never serde DTOs. The service owns
//! transactions — methods accept a generic executor for tx composability
//! (mirrors `outcome_repository`/`version_repository`).

use sqlx::Postgres;
use uuid::Uuid;

use crate::models::component_template::ComponentTemplateVersion;

const COLS: &str = "id, component_id, version_number, description, html_body, \
     variables, created_at, updated_at";

/// Insert a new version (the caller supplies a pre-generated id +
/// `version_number`).
#[allow(clippy::too_many_arguments)]
pub async fn insert<'e, E>(
    exec: E,
    id: Uuid,
    component_id: Uuid,
    version_number: i32,
    description: Option<&str>,
    html_body: &str,
    variables: &serde_json::Value,
) -> Result<ComponentTemplateVersion, sqlx::Error>
where
    E: sqlx::Executor<'e, Database = Postgres>,
{
    let sql = format!(
        "INSERT INTO rre.component_template_versions \
            (id, component_id, version_number, description, html_body, variables) \
         VALUES ($1, $2, $3, $4, $5, $6) RETURNING {COLS}"
    );
    sqlx::query_as::<_, ComponentTemplateVersion>(sqlx::AssertSqlSafe(sql))
        .bind(id)
        .bind(component_id)
        .bind(version_number)
        .bind(description)
        .bind(html_body)
        .bind(variables)
        .fetch_one(exec)
        .await
}

/// Fetch one version by id. `None` when absent.
pub async fn find<'e, E>(exec: E, id: Uuid) -> Result<Option<ComponentTemplateVersion>, sqlx::Error>
where
    E: sqlx::Executor<'e, Database = Postgres>,
{
    let sql = format!("SELECT {COLS} FROM rre.component_template_versions WHERE id = $1");
    sqlx::query_as::<_, ComponentTemplateVersion>(sqlx::AssertSqlSafe(sql))
        .bind(id)
        .fetch_optional(exec)
        .await
}

/// Fetch a version by `(component_id, version_number)`. `None` when absent.
pub async fn find_by_number<'e, E>(
    exec: E,
    component_id: Uuid,
    version_number: i32,
) -> Result<Option<ComponentTemplateVersion>, sqlx::Error>
where
    E: sqlx::Executor<'e, Database = Postgres>,
{
    let sql = format!(
        "SELECT {COLS} FROM rre.component_template_versions \
         WHERE component_id = $1 AND version_number = $2"
    );
    sqlx::query_as::<_, ComponentTemplateVersion>(sqlx::AssertSqlSafe(sql))
        .bind(component_id)
        .bind(version_number)
        .fetch_optional(exec)
        .await
}

/// List all versions of a component, ordered by `version_number ASC`.
pub async fn list_for_component<'e, E>(
    exec: E,
    component_id: Uuid,
) -> Result<Vec<ComponentTemplateVersion>, sqlx::Error>
where
    E: sqlx::Executor<'e, Database = Postgres>,
{
    let sql = format!(
        "SELECT {COLS} FROM rre.component_template_versions \
         WHERE component_id = $1 ORDER BY version_number ASC"
    );
    sqlx::query_as::<_, ComponentTemplateVersion>(sqlx::AssertSqlSafe(sql))
        .bind(component_id)
        .fetch_all(exec)
        .await
}

/// Lock the component's versions and return the current max `version_number`
/// (0 when none). Locks rows `FOR UPDATE` to serialize concurrent version
/// creation per component (mirrors `version_repository`).
pub async fn max_version_number_for_update<'e, E>(
    exec: E,
    component_id: Uuid,
) -> Result<i32, sqlx::Error>
where
    E: sqlx::Executor<'e, Database = Postgres>,
{
    // Postgres forbids FOR UPDATE alongside an aggregate, so lock in a CTE then
    // take MAX() in the outer query.
    let max: Option<i32> = sqlx::query_scalar(
        "WITH locked AS ( \
             SELECT version_number FROM rre.component_template_versions \
             WHERE component_id = $1 FOR UPDATE \
         ) \
         SELECT MAX(version_number) FROM locked",
    )
    .bind(component_id)
    .fetch_one(exec)
    .await?;
    Ok(max.unwrap_or(0))
}

/// Count the versions of a component (used to guard the last-version delete).
pub async fn count_for_component<'e, E>(exec: E, component_id: Uuid) -> Result<i64, sqlx::Error>
where
    E: sqlx::Executor<'e, Database = Postgres>,
{
    sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM rre.component_template_versions WHERE component_id = $1",
    )
    .bind(component_id)
    .fetch_one(exec)
    .await
}

/// Update mutable fields of a version (`description`/`html_body`/`variables`),
/// bumping `updated_at`. `None` fields are left untouched. `description` is a
/// nested Option (outer None = don't touch, inner None = set NULL).
pub async fn update_fields<'e, E>(
    exec: E,
    id: Uuid,
    description: Option<Option<&str>>,
    html_body: Option<&str>,
    variables: Option<&serde_json::Value>,
) -> Result<Option<ComponentTemplateVersion>, sqlx::Error>
where
    E: sqlx::Executor<'e, Database = Postgres>,
{
    let touch_description = description.is_some();
    let description_value = description.flatten();

    let sql = format!(
        "UPDATE rre.component_template_versions SET \
            description = CASE WHEN $2 THEN $3 ELSE description END, \
            html_body   = COALESCE($4, html_body), \
            variables   = COALESCE($5, variables), \
            updated_at  = now() \
         WHERE id = $1 RETURNING {COLS}"
    );
    sqlx::query_as::<_, ComponentTemplateVersion>(sqlx::AssertSqlSafe(sql))
        .bind(id)
        .bind(touch_description)
        .bind(description_value)
        .bind(html_body)
        .bind(variables)
        .fetch_optional(exec)
        .await
}

/// Delete a version by id. Returns rows affected.
pub async fn delete<'e, E>(exec: E, id: Uuid) -> Result<u64, sqlx::Error>
where
    E: sqlx::Executor<'e, Database = Postgres>,
{
    sqlx::query("DELETE FROM rre.component_template_versions WHERE id = $1")
        .bind(id)
        .execute(exec)
        .await
        .map(|r| r.rows_affected())
}

/// The set of component ids referenced by the given ids (used by rule_graph
/// validation's `apply_component_ref_exists` — mirrors how outcome ids are
/// threaded). Returns the ids that exist in `rre.component_templates`.
pub async fn existing_component_ids<'e, E>(exec: E, ids: &[Uuid]) -> Result<Vec<Uuid>, sqlx::Error>
where
    E: sqlx::Executor<'e, Database = Postgres>,
{
    sqlx::query_scalar::<_, Uuid>("SELECT id FROM rre.component_templates WHERE id = ANY($1)")
        .bind(ids)
        .fetch_all(exec)
        .await
}
