//! Site business logic (sites host-config design §4).
//!
//! Validates request DTOs, maps the pg unique-violation (`23505`) on the slug PK
//! / name / source index to [`AppError::SlugConflict`], and translates "row
//! absent" into [`AppError::SiteNotFound`]. Single-statement writes run directly
//! on the pool (no multi-statement transaction needed).

use sqlx::PgPool;
use validator::Validate;

use crate::{
    error::{AppError, AppResult},
    repositories::site_repository as repo,
    schemas::{
        pagination::{Page, PageParams},
        site::{SiteCreate, SiteRead, SiteUpdate},
    },
};

/// Postgres unique-violation SQLSTATE.
const PG_UNIQUE_VIOLATION: &str = "23505";

/// Create a site. Duplicate slug / name / source → [`AppError::SlugConflict`].
pub async fn create(pool: &PgPool, input: SiteCreate) -> AppResult<SiteRead> {
    input.validate().map_err(AppError::from)?;

    match repo::insert(
        pool,
        &input.slug,
        &input.name,
        &input.source_protocol,
        &input.source_host,
        input.source_port,
        &input.dest_protocol,
        &input.dest_host,
        input.dest_port,
    )
    .await
    {
        Ok(site) => Ok(site.into()),
        Err(err) => Err(map_conflict_error(err)),
    }
}

/// List sites, newest first, paginated. An optional `q` filters by
/// case-insensitive `name ILIKE '%q%'` (for the site picker).
pub async fn list(
    pool: &PgPool,
    params: &PageParams,
    q: Option<&str>,
) -> AppResult<Page<SiteRead>> {
    let (limit, offset, page, page_size) = params.resolve();
    let q = q.filter(|s| !s.is_empty());

    let (rows, total) = tokio::try_join!(
        repo::list_paged(pool, limit, offset, q),
        repo::count(pool, q)
    )?;

    let items = rows.into_iter().map(SiteRead::from).collect();
    Ok(Page::new(items, page, page_size, total))
}

/// Fetch a single site by slug. Absent → [`AppError::SiteNotFound`].
pub async fn get(pool: &PgPool, slug: &str) -> AppResult<SiteRead> {
    let site = repo::get(pool, slug)
        .await?
        .ok_or_else(|| not_found(slug))?;
    Ok(site.into())
}

/// Partially update a site. Absent → [`AppError::SiteNotFound`]. An empty PATCH
/// is a no-op that still returns the current row. Duplicate name / source →
/// [`AppError::SlugConflict`].
pub async fn update(pool: &PgPool, slug: &str, input: SiteUpdate) -> AppResult<SiteRead> {
    input.validate().map_err(AppError::from)?;

    if input.is_empty() {
        return get(pool, slug).await;
    }

    match repo::update(
        pool,
        slug,
        input.name.as_deref(),
        input.source_protocol.as_deref(),
        input.source_host.as_deref(),
        input.source_port,
        input.dest_protocol.as_deref(),
        input.dest_host.as_deref(),
        input.dest_port,
    )
    .await
    {
        Ok(Some(site)) => Ok(site.into()),
        Ok(None) => Err(not_found(slug)),
        Err(err) => Err(map_conflict_error(err)),
    }
}

/// Delete a site. Absent → [`AppError::SiteNotFound`].
pub async fn delete(pool: &PgPool, slug: &str) -> AppResult<()> {
    let deleted = repo::delete(pool, slug).await?;
    if deleted == 0 {
        return Err(not_found(slug));
    }
    Ok(())
}

/// `SITE_NOT_FOUND` for a slug.
fn not_found(slug: &str) -> AppError {
    AppError::SiteNotFound(format!("Site '{slug}' not found"))
}

/// Inspect a failed write: a pg unique-violation hits the slug PK, the name
/// unique index, or the `(source_host, source_port)` unique index. We surface a
/// single 409 naming all three; else the error falls through to Internal.
fn map_conflict_error(err: sqlx::Error) -> AppError {
    if let sqlx::Error::Database(db_err) = &err {
        if db_err.code().as_deref() == Some(PG_UNIQUE_VIOLATION) {
            return AppError::SlugConflict(
                "A site with this slug, name, or source host:port already exists".to_string(),
            );
        }
    }
    err.into()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_input(slug: &str) -> SiteCreate {
        SiteCreate {
            slug: slug.to_string(),
            name: "Demo".to_string(),
            source_protocol: "http".to_string(),
            source_host: "localhost".to_string(),
            source_port: 9000,
            dest_protocol: "http".to_string(),
            dest_host: "demo-upstream".to_string(),
            dest_port: 8081,
        }
    }

    #[test]
    fn create_input_accepts_valid() {
        assert!(create_input("demo-localhost").validate().is_ok());
    }

    #[test]
    fn create_input_rejects_uppercase_slug() {
        assert!(create_input("Demo-Localhost").validate().is_err());
    }

    #[test]
    fn create_input_rejects_too_short_slug() {
        assert!(create_input("ab").validate().is_err());
    }

    #[test]
    fn create_input_rejects_bad_protocol() {
        let mut input = create_input("demo-localhost");
        input.source_protocol = "ftp".to_string();
        assert!(input.validate().is_err());
    }

    #[test]
    fn create_input_rejects_port_out_of_range() {
        let mut input = create_input("demo-localhost");
        input.source_port = 0;
        assert!(input.validate().is_err());
        let mut input = create_input("demo-localhost");
        input.dest_port = 70_000;
        assert!(input.validate().is_err());
    }

    #[test]
    fn create_input_rejects_empty_name() {
        let mut input = create_input("demo-localhost");
        input.name = String::new();
        assert!(input.validate().is_err());
    }

    #[test]
    fn update_input_rejects_bad_protocol() {
        let input = SiteUpdate {
            dest_protocol: Some("gopher".to_string()),
            ..SiteUpdate::default()
        };
        assert!(input.validate().is_err());
    }

    #[test]
    fn update_input_accepts_partial() {
        let input = SiteUpdate {
            name: Some("Renamed".to_string()),
            ..SiteUpdate::default()
        };
        assert!(input.validate().is_ok());
        assert!(!input.is_empty());
        assert!(SiteUpdate::default().is_empty());
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
