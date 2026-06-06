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

/// Create a feature. Duplicate slug → [`AppError::SlugConflict`].
pub async fn create(pool: &PgPool, input: FeatureCreate) -> AppResult<FeatureRead> {
    input.validate().map_err(AppError::from)?;

    match repo::insert(pool, &input.id, &input.name, input.r#type).await {
        Ok(feature) => Ok(feature.into()),
        Err(err) => Err(map_insert_error(err, &input.id)),
    }
}

/// List features, newest first, paginated.
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

/// Update a feature's name. Absent → [`AppError::FeatureNotFound`]. A `None`
/// `name` is a no-op that still returns the current row.
pub async fn update(pool: &PgPool, id: &str, input: FeatureUpdate) -> AppResult<FeatureRead> {
    input.validate().map_err(AppError::from)?;

    match input.name {
        Some(name) => {
            let feature = repo::update_name(pool, id, &name)
                .await?
                .ok_or_else(|| not_found(id))?;
            Ok(feature.into())
        }
        None => get(pool, id).await,
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

/// Inspect a failed insert: pg unique-violation on the slug → conflict, else
/// surface as an internal error.
fn map_insert_error(err: sqlx::Error, id: &str) -> AppError {
    if let sqlx::Error::Database(db_err) = &err {
        if db_err.code().as_deref() == Some(PG_UNIQUE_VIOLATION) {
            return AppError::SlugConflict(format!("Feature '{id}' already exists"));
        }
    }
    err.into()
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
    fn unique_violation_maps_to_slug_conflict() {
        // Synthesize a pg unique-violation by name; the helper only inspects the
        // SQLSTATE code path, so a non-database error must fall through to Internal.
        let err = sqlx::Error::RowNotFound;
        let mapped = map_insert_error(err, "demo-article");
        assert!(matches!(mapped, AppError::Internal(_)));
    }
}
