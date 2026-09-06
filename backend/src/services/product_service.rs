//! Product Catalogue business logic.
//!
//! Validates request DTOs, maps the pg unique-violation (`23505`) on the label
//! PK / name unique index to [`AppError::SlugConflict`], and translates "row
//! absent" into [`AppError::ProductNotFound`]. Single-statement writes run
//! directly on the pool (no multi-statement transaction needed).

use sqlx::PgPool;
use validator::Validate;

use crate::{
    error::{AppError, AppResult},
    repositories::product_repository as repo,
    schemas::{
        pagination::{Page, PageParams},
        product::{ProductCreate, ProductRead, ProductUpdate},
    },
};

/// Postgres unique-violation SQLSTATE.
const PG_UNIQUE_VIOLATION: &str = "23505";

/// Create a product. Duplicate label / name → [`AppError::SlugConflict`].
pub async fn create(pool: &PgPool, input: ProductCreate) -> AppResult<ProductRead> {
    input.validate().map_err(AppError::from)?;
    match repo::insert(
        pool,
        &input.label,
        &input.name,
        input.description.as_deref(),
    )
    .await
    {
        Ok(product) => Ok(product.into()),
        Err(err) => Err(map_conflict_error(err)),
    }
}

/// List products, newest first, paginated. An optional `q` filters by
/// case-insensitive `name ILIKE '%q%'`.
pub async fn list(
    pool: &PgPool,
    params: &PageParams,
    q: Option<&str>,
) -> AppResult<Page<ProductRead>> {
    let (limit, offset, page, page_size) = params.resolve();
    let q = q.filter(|s| !s.is_empty());

    let (rows, total) = tokio::try_join!(
        repo::list_paged(pool, limit, offset, q),
        repo::count(pool, q)
    )?;

    let items = rows.into_iter().map(ProductRead::from).collect();
    Ok(Page::new(items, page, page_size, total))
}

/// Fetch a single product by label. Absent → [`AppError::ProductNotFound`].
pub async fn get(pool: &PgPool, label: &str) -> AppResult<ProductRead> {
    let product = repo::get(pool, label)
        .await?
        .ok_or_else(|| not_found(label))?;
    Ok(product.into())
}

/// Partially update a product. Absent → [`AppError::ProductNotFound`]. An empty
/// PATCH is a no-op that still returns the current row. Duplicate name →
/// [`AppError::SlugConflict`].
pub async fn update(pool: &PgPool, label: &str, input: ProductUpdate) -> AppResult<ProductRead> {
    input.validate().map_err(AppError::from)?;

    if input.is_empty() {
        return get(pool, label).await;
    }

    match repo::update(
        pool,
        label,
        input.name.as_deref(),
        input.description.as_deref(),
        false,
    )
    .await
    {
        Ok(Some(product)) => Ok(product.into()),
        Ok(None) => Err(not_found(label)),
        Err(err) => Err(map_conflict_error(err)),
    }
}

/// Delete a product. Absent → [`AppError::ProductNotFound`].
pub async fn delete(pool: &PgPool, label: &str) -> AppResult<()> {
    let deleted = repo::delete(pool, label).await?;
    if deleted == 0 {
        return Err(not_found(label));
    }
    Ok(())
}

/// `PRODUCT_NOT_FOUND` for a label.
fn not_found(label: &str) -> AppError {
    AppError::ProductNotFound(format!("Product '{label}' not found"))
}

/// Inspect a failed write: a pg unique-violation hits the label PK or the name
/// unique index. We surface a single 409 naming both; else the error falls
/// through to Internal.
fn map_conflict_error(err: sqlx::Error) -> AppError {
    if let sqlx::Error::Database(db_err) = &err {
        if db_err.code().as_deref() == Some(PG_UNIQUE_VIOLATION) {
            return AppError::SlugConflict(
                "A product with this label or name already exists".to_string(),
            );
        }
    }
    err.into()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_input(label: &str) -> ProductCreate {
        ProductCreate {
            label: label.to_string(),
            name: "Premium".to_string(),
            description: None,
        }
    }

    #[test]
    fn create_input_accepts_valid() {
        assert!(create_input("premium").validate().is_ok());
    }

    #[test]
    fn validation_error_maps_to_422_with_details() {
        let err = create_input("Bad-Label").validate().unwrap_err();
        match AppError::from(err) {
            AppError::Validation { details } => assert!(!details.is_empty()),
            other => panic!("expected validation error, got {other:?}"),
        }
    }

    #[test]
    fn non_unique_db_error_falls_through_to_internal() {
        let mapped = map_conflict_error(sqlx::Error::RowNotFound);
        assert!(matches!(mapped, AppError::Internal(_)));
    }
}
