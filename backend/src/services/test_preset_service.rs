//! Test-preset business logic (CONTRACT §3/§7).
//!
//! Validates request DTOs, maps the pg unique-violation (`23505`) on the slug PK
//! / name index to [`AppError::SlugConflict`], and translates "row absent" into
//! [`AppError::TestPresetNotFound`]. Single-statement writes run directly on the
//! pool (no multi-statement transaction needed).

use sqlx::PgPool;
use validator::Validate;

use crate::{
    error::{AppError, AppResult},
    repositories::test_preset_repository as repo,
    schemas::{
        pagination::{Page, PageParams},
        test_preset::{self, TestPresetCreate, TestPresetRead, TestPresetUpdate},
    },
};

/// Postgres unique-violation SQLSTATE.
const PG_UNIQUE_VIOLATION: &str = "23505";

/// Create a preset. Duplicate slug / name → [`AppError::SlugConflict`].
pub async fn create(pool: &PgPool, input: TestPresetCreate) -> AppResult<TestPresetRead> {
    input.validate().map_err(AppError::from)?;

    // The DTO validator covers `kind`; the service validates the opaque payload.
    test_preset::validate_payload(&input.payload)?;

    match repo::insert(pool, &input.slug, &input.name, &input.kind, &input.payload).await {
        Ok(preset) => Ok(preset.into()),
        Err(err) => Err(map_conflict_error(err)),
    }
}

/// List presets, newest first, paginated. An optional `q` filters by
/// case-insensitive `name ILIKE '%q%'`; an optional `kind` filters to a single
/// kind.
pub async fn list(
    pool: &PgPool,
    params: &PageParams,
    q: Option<&str>,
    kind: Option<&str>,
) -> AppResult<Page<TestPresetRead>> {
    let (limit, offset, page, page_size) = params.resolve();
    let q = q.filter(|s| !s.is_empty());
    let kind = kind.filter(|s| !s.is_empty());

    let (rows, total) = tokio::try_join!(
        repo::list_paged(pool, limit, offset, q, kind),
        repo::count(pool, q, kind)
    )?;

    let items = rows.into_iter().map(TestPresetRead::from).collect();
    Ok(Page::new(items, page, page_size, total))
}

/// Fetch a single preset by slug. Absent → [`AppError::TestPresetNotFound`].
pub async fn get(pool: &PgPool, slug: &str) -> AppResult<TestPresetRead> {
    let preset = repo::get(pool, slug)
        .await?
        .ok_or_else(|| not_found(slug))?;
    Ok(preset.into())
}

/// Partially update a preset. Absent → [`AppError::TestPresetNotFound`]. An empty
/// PATCH is a no-op that still returns the current row. Duplicate name →
/// [`AppError::SlugConflict`]. The slug and kind are immutable.
pub async fn update(
    pool: &PgPool,
    slug: &str,
    input: TestPresetUpdate,
) -> AppResult<TestPresetRead> {
    input.validate().map_err(AppError::from)?;

    if input.is_empty() {
        return get(pool, slug).await;
    }

    // `Some(value)` replaces the stored payload wholesale; `None` leaves it
    // unchanged (the repo COALESCEs a NULL bind). Validate when present.
    if let Some(payload) = &input.payload {
        test_preset::validate_payload(payload)?;
    }

    match repo::update(pool, slug, input.name.as_deref(), input.payload.as_ref()).await {
        Ok(Some(preset)) => Ok(preset.into()),
        Ok(None) => Err(not_found(slug)),
        Err(err) => Err(map_conflict_error(err)),
    }
}

/// Delete a preset. Absent → [`AppError::TestPresetNotFound`].
pub async fn delete(pool: &PgPool, slug: &str) -> AppResult<()> {
    let deleted = repo::delete(pool, slug).await?;
    if deleted == 0 {
        return Err(not_found(slug));
    }
    Ok(())
}

/// `TEST_PRESET_NOT_FOUND` for a slug.
fn not_found(slug: &str) -> AppError {
    AppError::TestPresetNotFound(format!("Test preset '{slug}' not found"))
}

/// Inspect a failed write: a pg unique-violation hits the slug PK or the name
/// unique index. We surface a single 409 naming both; else the error falls
/// through to Internal.
fn map_conflict_error(err: sqlx::Error) -> AppError {
    if let sqlx::Error::Database(db_err) = &err {
        if db_err.code().as_deref() == Some(PG_UNIQUE_VIOLATION) {
            return AppError::SlugConflict(
                "A test preset with this slug or name already exists".to_string(),
            );
        }
    }
    err.into()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn create_input(slug: &str) -> TestPresetCreate {
        TestPresetCreate {
            slug: slug.to_string(),
            name: "Demo".to_string(),
            kind: "rule".to_string(),
            payload: json!({ "feature_type": "html" }),
        }
    }

    #[test]
    fn create_input_accepts_valid() {
        assert!(create_input("demo-preset").validate().is_ok());
    }

    #[test]
    fn create_input_rejects_uppercase_slug() {
        assert!(create_input("Demo-Preset").validate().is_err());
    }

    #[test]
    fn create_input_rejects_too_short_slug() {
        assert!(create_input("ab").validate().is_err());
    }

    #[test]
    fn create_input_rejects_bad_kind() {
        let mut input = create_input("demo-preset");
        input.kind = "other".to_string();
        assert!(input.validate().is_err());
    }

    #[test]
    fn create_input_rejects_empty_name() {
        let mut input = create_input("demo-preset");
        input.name = String::new();
        assert!(input.validate().is_err());
    }

    #[test]
    fn update_input_accepts_partial() {
        let input = TestPresetUpdate {
            name: Some("Renamed".to_string()),
            ..TestPresetUpdate::default()
        };
        assert!(input.validate().is_ok());
        assert!(!input.is_empty());
        assert!(TestPresetUpdate::default().is_empty());
    }

    #[test]
    fn validation_error_maps_to_422_with_details() {
        let err = create_input("X").validate().unwrap_err();
        match AppError::from(err) {
            AppError::Validation { details } => assert!(!details.is_empty()),
            other => panic!("expected validation error, got {other:?}"),
        }
    }

    #[test]
    fn non_unique_db_error_falls_through_to_internal() {
        // A non-database error must not be mistaken for a conflict.
        let mapped = map_conflict_error(sqlx::Error::RowNotFound);
        assert!(matches!(mapped, AppError::Internal(_)));
    }
}
