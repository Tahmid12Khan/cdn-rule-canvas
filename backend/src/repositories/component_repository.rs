//! Component data access (BACKEND CONTRACT §4/§7). RUNTIME SQLx queries only;
//! returns [`Component`] models. Methods take a generic executor so they compose
//! inside a transaction or run against `&PgPool`.

use sqlx::Postgres;
use uuid::Uuid;

use crate::models::component::Component;
use crate::models::enums::Placement;

const COLS: &str =
    "id, outcome_id, slug, type, config, placement, order_index, created_at, updated_at";

/// Insert a new component with a caller-supplied id.
pub async fn insert<'e, E>(
    exec: E,
    id: Uuid,
    outcome_id: Uuid,
    slug: &str,
    r#type: &str,
    config: &serde_json::Value,
    placement: Placement,
    order_index: i32,
) -> Result<Component, sqlx::Error>
where
    E: sqlx::Executor<'e, Database = Postgres>,
{
    let sql = format!(
        "INSERT INTO rre.components \
            (id, outcome_id, slug, type, config, placement, order_index) \
         VALUES ($1, $2, $3, $4, $5, $6, $7) \
         RETURNING {COLS}"
    );
    sqlx::query_as::<_, Component>(sqlx::AssertSqlSafe(sql))
        .bind(id)
        .bind(outcome_id)
        .bind(slug)
        .bind(r#type)
        .bind(config)
        .bind(placement)
        .bind(order_index)
        .fetch_one(exec)
        .await
}

/// Find one component by id.
pub async fn find<'e, E>(exec: E, id: Uuid) -> Result<Option<Component>, sqlx::Error>
where
    E: sqlx::Executor<'e, Database = Postgres>,
{
    let sql = format!("SELECT {COLS} FROM rre.components WHERE id = $1");
    sqlx::query_as::<_, Component>(sqlx::AssertSqlSafe(sql))
        .bind(id)
        .fetch_optional(exec)
        .await
}

/// List components for a single outcome, ordered by `order_index ASC`.
pub async fn list_for_outcome<'e, E>(
    exec: E,
    outcome_id: Uuid,
) -> Result<Vec<Component>, sqlx::Error>
where
    E: sqlx::Executor<'e, Database = Postgres>,
{
    let sql = format!(
        "SELECT {COLS} FROM rre.components \
         WHERE outcome_id = $1 \
         ORDER BY order_index ASC, created_at ASC"
    );
    sqlx::query_as::<_, Component>(sqlx::AssertSqlSafe(sql))
        .bind(outcome_id)
        .fetch_all(exec)
        .await
}

/// List components for many outcomes in one round-trip (avoids N+1).
/// Ordered by `outcome_id`, then `order_index ASC`.
pub async fn list_for_outcomes<'e, E>(
    exec: E,
    outcome_ids: &[Uuid],
) -> Result<Vec<Component>, sqlx::Error>
where
    E: sqlx::Executor<'e, Database = Postgres>,
{
    let sql = format!(
        "SELECT {COLS} FROM rre.components \
         WHERE outcome_id = ANY($1) \
         ORDER BY outcome_id, order_index ASC, created_at ASC"
    );
    sqlx::query_as::<_, Component>(sqlx::AssertSqlSafe(sql))
        .bind(outcome_ids)
        .fetch_all(exec)
        .await
}

/// Update mutable fields of a component. `None` outer = leave untouched.
pub async fn update<'e, E>(
    exec: E,
    id: Uuid,
    slug: Option<&str>,
    r#type: Option<&str>,
    config: Option<&serde_json::Value>,
    placement: Option<Placement>,
    order_index: Option<i32>,
) -> Result<Option<Component>, sqlx::Error>
where
    E: sqlx::Executor<'e, Database = Postgres>,
{
    let sql = format!(
        "UPDATE rre.components SET \
            slug        = COALESCE($2, slug), \
            type        = COALESCE($3, type), \
            config      = COALESCE($4, config), \
            placement   = COALESCE($5, placement), \
            order_index = COALESCE($6, order_index), \
            updated_at  = now() \
         WHERE id = $1 \
         RETURNING {COLS}"
    );
    sqlx::query_as::<_, Component>(sqlx::AssertSqlSafe(sql))
        .bind(id)
        .bind(slug)
        .bind(r#type)
        .bind(config)
        .bind(placement)
        .bind(order_index)
        .fetch_optional(exec)
        .await
}

/// Set only the ordering of a component (used by reorder paths).
pub async fn set_order<'e, E>(exec: E, id: Uuid, order_index: i32) -> Result<(), sqlx::Error>
where
    E: sqlx::Executor<'e, Database = Postgres>,
{
    sqlx::query("UPDATE rre.components SET order_index = $2, updated_at = now() WHERE id = $1")
        .bind(id)
        .bind(order_index)
        .execute(exec)
        .await
        .map(|_| ())
}

/// Set the ordering of many components in ONE round-trip. `items` is a slice of
/// `(component_id, order_index)`; bound as parallel arrays joined via `unnest`
/// (no string interpolation). A no-op on an empty slice.
pub async fn set_order_bulk<'e, E>(exec: E, items: &[(Uuid, i32)]) -> Result<(), sqlx::Error>
where
    E: sqlx::Executor<'e, Database = Postgres>,
{
    if items.is_empty() {
        return Ok(());
    }
    let ids: Vec<Uuid> = items.iter().map(|(id, _)| *id).collect();
    let orders: Vec<i32> = items.iter().map(|(_, ord)| *ord).collect();
    sqlx::query(
        "UPDATE rre.components SET order_index = data.ord, updated_at = now() \
         FROM unnest($1::uuid[], $2::int[]) AS data(id, ord) \
         WHERE rre.components.id = data.id",
    )
    .bind(&ids)
    .bind(&orders)
    .execute(exec)
    .await
    .map(|_| ())
}

/// Delete a component by id. Returns the number of rows removed.
pub async fn delete<'e, E>(exec: E, id: Uuid) -> Result<u64, sqlx::Error>
where
    E: sqlx::Executor<'e, Database = Postgres>,
{
    sqlx::query("DELETE FROM rre.components WHERE id = $1")
        .bind(id)
        .execute(exec)
        .await
        .map(|r| r.rows_affected())
}

/// Highest `order_index` currently used in an outcome, or `None` if empty.
pub async fn max_order_index<'e, E>(exec: E, outcome_id: Uuid) -> Result<Option<i32>, sqlx::Error>
where
    E: sqlx::Executor<'e, Database = Postgres>,
{
    sqlx::query_scalar::<_, Option<i32>>(
        "SELECT MAX(order_index) FROM rre.components WHERE outcome_id = $1",
    )
    .bind(outcome_id)
    .fetch_one(exec)
    .await
}
