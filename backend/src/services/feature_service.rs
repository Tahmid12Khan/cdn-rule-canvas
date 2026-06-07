//! Feature business logic (BACKEND CONTRACT §7, §10).
//!
//! Validates request DTOs, maps the pg unique-violation (`23505`) on the slug to
//! [`AppError::SlugConflict`], and translates "row absent" into
//! [`AppError::FeatureNotFound`]. Single-statement writes run directly on the
//! pool (no multi-statement transaction needed).

use sqlx::PgPool;
use validator::Validate;

use crate::{
    error::{AppError, AppResult},
    repositories::feature_repository as repo,
    schemas::{
        feature::{FeatureCreate, FeatureRead, FeatureUpdate},
        pagination::{Page, PageParams},
    },
};

/// Postgres unique-violation SQLSTATE.
const PG_UNIQUE_VIOLATION: &str = "23505";

/// Name of the per-type execution-order unique constraint (migration 0012).
const EXEC_ORDER_CONSTRAINT: &str = "features_type_execution_order_unique";

/// Create a feature. Duplicate slug → [`AppError::SlugConflict`]; duplicate
/// `execution_order` within the type → [`AppError::ExecutionOrderConflict`].
///
/// When `execution_order` is omitted the service auto-assigns the next free
/// number for the type (`MAX(execution_order WHERE type) + 1`, or 1).
pub async fn create(pool: &PgPool, input: FeatureCreate) -> AppResult<FeatureRead> {
    input.validate().map_err(AppError::from)?;

    let execution_order = match input.execution_order {
        Some(order) => order,
        None => repo::next_execution_order(pool, input.r#type).await?,
    };

    match repo::insert(pool, &input.id, &input.name, input.r#type, execution_order).await {
        Ok(feature) => Ok(feature.into()),
        Err(err) => Err(map_conflict_error(err, &input.id, execution_order)),
    }
}

/// List features in execution-priority order (`type ASC, execution_order ASC`),
/// paginated.
pub async fn list(pool: &PgPool, params: &PageParams) -> AppResult<Page<FeatureRead>> {
    let (limit, offset, page, page_size) = params.resolve();

    let (rows, total) = tokio::try_join!(repo::list_paged(pool, limit, offset), repo::count(pool))?;

    let items = rows.into_iter().map(FeatureRead::from).collect();
    Ok(Page::new(items, page, page_size, total))
}

/// Fetch a single feature by slug. Absent → [`AppError::FeatureNotFound`].
pub async fn get(pool: &PgPool, id: &str) -> AppResult<FeatureRead> {
    let feature = repo::find(pool, id).await?.ok_or_else(|| not_found(id))?;
    Ok(feature.into())
}

/// Update a feature's `name` and/or `execution_order`. Absent →
/// [`AppError::FeatureNotFound`]. Both fields `None` is a no-op that still
/// returns the current row. A duplicate `execution_order` within the type →
/// [`AppError::ExecutionOrderConflict`] (gaps are preserved — other rows are
/// never renumbered).
pub async fn update(pool: &PgPool, id: &str, input: FeatureUpdate) -> AppResult<FeatureRead> {
    input.validate().map_err(AppError::from)?;

    if input.name.is_none() && input.execution_order.is_none() {
        return get(pool, id).await;
    }

    match repo::update_fields(pool, id, input.name.as_deref(), input.execution_order).await {
        Ok(Some(feature)) => Ok(feature.into()),
        Ok(None) => Err(not_found(id)),
        Err(err) => Err(map_update_error(err, input.execution_order)),
    }
}

/// Delete a feature (cascades). Absent → [`AppError::FeatureNotFound`].
pub async fn delete(pool: &PgPool, id: &str) -> AppResult<()> {
    let deleted = repo::delete(pool, id).await?;
    if deleted == 0 {
        return Err(not_found(id));
    }
    Ok(())
}

/// `FEATURE_NOT_FOUND` for a slug.
fn not_found(id: &str) -> AppError {
    AppError::FeatureNotFound(format!("Feature '{id}' not found"))
}

/// Inspect a failed insert: a pg unique-violation is either the slug PK or the
/// per-type execution-order constraint. Disambiguate by the constraint name so
/// each maps to its own 409, else surface as an internal error.
fn map_conflict_error(err: sqlx::Error, id: &str, execution_order: i32) -> AppError {
    if let sqlx::Error::Database(db_err) = &err {
        if db_err.code().as_deref() == Some(PG_UNIQUE_VIOLATION) {
            if db_err.constraint() == Some(EXEC_ORDER_CONSTRAINT) {
                return exec_order_conflict(execution_order);
            }
            return AppError::SlugConflict(format!("Feature '{id}' already exists"));
        }
    }
    err.into()
}

/// Inspect a failed update: a pg unique-violation here can only be the per-type
/// execution-order constraint (slug is immutable on update).
fn map_update_error(err: sqlx::Error, execution_order: Option<i32>) -> AppError {
    if let sqlx::Error::Database(db_err) = &err {
        if db_err.code().as_deref() == Some(PG_UNIQUE_VIOLATION)
            && db_err.constraint() == Some(EXEC_ORDER_CONSTRAINT)
        {
            return exec_order_conflict(execution_order.unwrap_or_default());
        }
    }
    err.into()
}

/// `EXECUTION_ORDER_CONFLICT` for a colliding order within a type.
fn exec_order_conflict(execution_order: i32) -> AppError {
    AppError::ExecutionOrderConflict(format!(
        "execution_order {execution_order} is already taken for this feature type"
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::enums::FeatureType;

    fn create_input(id: &str, name: &str) -> FeatureCreate {
        FeatureCreate {
            id: id.to_string(),
            name: name.to_string(),
            r#type: FeatureType::Html,
            execution_order: None,
        }
    }

    #[test]
    fn create_input_rejects_uppercase_slug() {
        let input = create_input("Bad-Slug", "Name");
        assert!(input.validate().is_err());
    }

    #[test]
    fn create_input_rejects_too_short_slug() {
        let input = create_input("ab", "Name");
        assert!(input.validate().is_err());
    }

    #[test]
    fn create_input_rejects_empty_name() {
        let input = create_input("good-slug", "");
        assert!(input.validate().is_err());
    }

    #[test]
    fn create_input_accepts_valid() {
        let input = create_input("demo-article", "DN Article");
        assert!(input.validate().is_ok());
    }

    #[test]
    fn validation_error_maps_to_422_with_details() {
        let err = create_input("X", "").validate().unwrap_err();
        let app = AppError::from(err);
        match app {
            AppError::Validation { details } => assert!(!details.is_empty()),
            other => panic!("expected validation error, got {other:?}"),
        }
    }

    #[test]
    fn non_database_error_falls_through_to_internal() {
        // The conflict helpers only inspect the SQLSTATE/constraint path, so a
        // non-database error must fall through to Internal.
        let mapped = map_conflict_error(sqlx::Error::RowNotFound, "demo-article", 1);
        assert!(matches!(mapped, AppError::Internal(_)));

        let mapped = map_update_error(sqlx::Error::RowNotFound, Some(1));
        assert!(matches!(mapped, AppError::Internal(_)));
    }

    #[test]
    fn exec_order_conflict_carries_the_number() {
        match exec_order_conflict(7) {
            AppError::ExecutionOrderConflict(msg) => assert!(msg.contains('7')),
            other => panic!("expected execution-order conflict, got {other:?}"),
        }
    }
}
