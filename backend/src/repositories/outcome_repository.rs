//! Outcome data access (BACKEND CONTRACT §4/§7). RUNTIME SQLx queries only;
//! returns [`Outcome`] models. The service owns transactions — methods accept a
//! generic executor so they compose inside a `pool.begin()` transaction or run
//! against `&PgPool` directly.

use sqlx::Postgres;
use uuid::Uuid;

use crate::models::enums::VersionStatus;
use crate::models::outcome::Outcome;

const COLS: &str =
    "id, version_id, title, description, is_builtin, order_index, created_at, updated_at";

/// Insert a new outcome. The caller supplies a pre-generated id for portability.
pub async fn insert<'e, E>(
    exec: E,
    id: Uuid,
    version_id: Uuid,
    title: &str,
    description: Option<&str>,
    is_builtin: bool,
    order_index: i32,
) -> Result<Outcome, sqlx::Error>
where
    E: sqlx::Executor<'e, Database = Postgres>,
{
    let sql = format!(
        "INSERT INTO rre.outcomes (id, version_id, title, description, is_builtin, order_index) \
         VALUES ($1, $2, $3, $4, $5, $6) \
         RETURNING {COLS}"
    );
    sqlx::query_as::<_, Outcome>(&sql)
        .bind(id)
        .bind(version_id)
        .bind(title)
        .bind(description)
        .bind(is_builtin)
        .bind(order_index)
        .fetch_one(exec)
        .await
}

/// Find one outcome by id.
pub async fn find<'e, E>(exec: E, id: Uuid) -> Result<Option<Outcome>, sqlx::Error>
where
    E: sqlx::Executor<'e, Database = Postgres>,
{
    let sql = format!("SELECT {COLS} FROM rre.outcomes WHERE id = $1");
    sqlx::query_as::<_, Outcome>(&sql)
        .bind(id)
        .fetch_optional(exec)
        .await
}

/// List all outcomes for a version, ordered by `order_index ASC, created_at ASC`.
pub async fn list_for_version<'e, E>(exec: E, version_id: Uuid) -> Result<Vec<Outcome>, sqlx::Error>
where
    E: sqlx::Executor<'e, Database = Postgres>,
{
    let sql = format!(
        "SELECT {COLS} FROM rre.outcomes \
         WHERE version_id = $1 \
         ORDER BY order_index ASC, created_at ASC"
    );
    sqlx::query_as::<_, Outcome>(&sql)
        .bind(version_id)
        .fetch_all(exec)
        .await
}

/// Update mutable fields of an outcome. `None` fields are left untouched via
/// `COALESCE`. `updated_at` is bumped.
pub async fn update<'e, E>(
    exec: E,
    id: Uuid,
    title: Option<&str>,
    description: Option<Option<&str>>,
    order_index: Option<i32>,
) -> Result<Option<Outcome>, sqlx::Error>
where
    E: sqlx::Executor<'e, Database = Postgres>,
{
    // `description` is a nested Option: outer None = "don't touch", inner None =
    // "set NULL". We encode the touch flag as a separate bind.
    let touch_description = description.is_some();
    let description_value = description.flatten();

    let sql = format!(
        "UPDATE rre.outcomes SET \
            title       = COALESCE($2, title), \
            description = CASE WHEN $3 THEN $4 ELSE description END, \
            order_index = COALESCE($5, order_index), \
            updated_at  = now() \
         WHERE id = $1 \
         RETURNING {COLS}"
    );
    sqlx::query_as::<_, Outcome>(&sql)
        .bind(id)
        .bind(title)
        .bind(touch_description)
        .bind(description_value)
        .bind(order_index)
        .fetch_optional(exec)
        .await
}

/// Set only the ordering of an outcome (used by reorder paths).
pub async fn set_order<'e, E>(exec: E, id: Uuid, order_index: i32) -> Result<(), sqlx::Error>
where
    E: sqlx::Executor<'e, Database = Postgres>,
{
    sqlx::query("UPDATE rre.outcomes SET order_index = $2, updated_at = now() WHERE id = $1")
        .bind(id)
        .bind(order_index)
        .execute(exec)
        .await
        .map(|_| ())
}

/// Delete an outcome by id. Returns the number of rows removed.
pub async fn delete<'e, E>(exec: E, id: Uuid) -> Result<u64, sqlx::Error>
where
    E: sqlx::Executor<'e, Database = Postgres>,
{
    sqlx::query("DELETE FROM rre.outcomes WHERE id = $1")
        .bind(id)
        .execute(exec)
        .await
        .map(|r| r.rows_affected())
}

/// Highest `order_index` currently used in a version, or `None` if empty.
pub async fn max_order_index<'e, E>(exec: E, version_id: Uuid) -> Result<Option<i32>, sqlx::Error>
where
    E: sqlx::Executor<'e, Database = Postgres>,
{
    sqlx::query_scalar::<_, Option<i32>>(
        "SELECT MAX(order_index) FROM rre.outcomes WHERE version_id = $1",
    )
    .bind(version_id)
    .fetch_one(exec)
    .await
}

/// Fetch the status of a version by id (`None` if the version does not exist).
/// Used by the outcome service to enforce the DRAFT edit lock.
pub async fn version_status<'e, E>(
    exec: E,
    version_id: Uuid,
) -> Result<Option<VersionStatus>, sqlx::Error>
where
    E: sqlx::Executor<'e, Database = Postgres>,
{
    sqlx::query_scalar::<_, VersionStatus>("SELECT status FROM rre.versions WHERE id = $1")
        .bind(version_id)
        .fetch_optional(exec)
        .await
}

/// Resolve the owning version id + its status for a given outcome id.
/// `None` if the outcome does not exist.
pub async fn version_status_for_outcome<'e, E>(
    exec: E,
    outcome_id: Uuid,
) -> Result<Option<(Uuid, VersionStatus)>, sqlx::Error>
where
    E: sqlx::Executor<'e, Database = Postgres>,
{
    sqlx::query_as::<_, (Uuid, VersionStatus)>(
        "SELECT v.id, v.status FROM rre.versions v \
         JOIN rre.outcomes o ON o.version_id = v.id \
         WHERE o.id = $1",
    )
    .bind(outcome_id)
    .fetch_optional(exec)
    .await
}

/// Resolve the owning version id + its status for a given component id.
/// `None` if the component does not exist.
pub async fn version_status_for_component<'e, E>(
    exec: E,
    component_id: Uuid,
) -> Result<Option<(Uuid, VersionStatus)>, sqlx::Error>
where
    E: sqlx::Executor<'e, Database = Postgres>,
{
    sqlx::query_as::<_, (Uuid, VersionStatus)>(
        "SELECT v.id, v.status FROM rre.versions v \
         JOIN rre.outcomes o ON o.version_id = v.id \
         JOIN rre.components c ON c.outcome_id = o.id \
         WHERE c.id = $1",
    )
    .bind(component_id)
    .fetch_optional(exec)
    .await
}
