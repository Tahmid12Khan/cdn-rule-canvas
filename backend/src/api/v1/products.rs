//! Product Catalogue HTTP handlers.
//!
//! Routers map `FromRow` models to `*Read` DTOs via the service layer; they
//! never query the DB directly and never serialize a `FromRow` struct.

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    routing::post,
    Json, Router,
};
use serde::Deserialize;

use crate::{
    error::AppResult,
    schemas::{
        pagination::{Page, PageParams},
        product::{ProductCreate, ProductRead, ProductUpdate},
    },
    services::product_service,
    state::AppState,
};

/// Build the product sub-router.
pub fn router() -> Router<AppState> {
    Router::new()
        .route("/products", post(create).get(list))
        .route(
            "/products/{label}",
            axum::routing::get(get).patch(update).delete(delete),
        )
}

/// Optional `q` filter for the list endpoint (case-insensitive name search).
#[derive(Debug, Default, Deserialize)]
pub struct ProductListQuery {
    /// Case-insensitive `name ILIKE '%q%'` filter.
    #[serde(default)]
    pub q: Option<String>,
}

/// `POST /products` — create a product.
#[utoipa::path(
    post,
    path = "/api/v1/products",
    request_body = ProductCreate,
    responses(
        (status = 201, description = "Created", body = ProductRead),
        (status = 409, description = "Label/name conflict", body = crate::error::ErrorEnvelope),
        (status = 422, description = "Validation error", body = crate::error::ErrorEnvelope)
    ),
    tag = "products"
)]
pub async fn create(
    State(state): State<AppState>,
    Json(input): Json<ProductCreate>,
) -> AppResult<impl IntoResponse> {
    let product = product_service::create(&state.pool, input).await?;
    Ok((StatusCode::CREATED, Json(product)))
}

/// `GET /products?page&page_size&q` — paginated list.
#[utoipa::path(
    get,
    path = "/api/v1/products",
    params(PageParams, ("q" = Option<String>, Query, description = "Case-insensitive name filter")),
    responses((status = 200, description = "Product page", body = inline(Page<ProductRead>))),
    tag = "products"
)]
pub async fn list(
    State(state): State<AppState>,
    Query(params): Query<PageParams>,
    Query(filter): Query<ProductListQuery>,
) -> AppResult<Json<Page<ProductRead>>> {
    let page = product_service::list(&state.pool, &params, filter.q.as_deref()).await?;
    Ok(Json(page))
}

/// `GET /products/{label}` — fetch one.
#[utoipa::path(
    get,
    path = "/api/v1/products/{label}",
    params(("label" = String, Path, description = "Product label")),
    responses(
        (status = 200, description = "Product", body = ProductRead),
        (status = 404, description = "Not found", body = crate::error::ErrorEnvelope)
    ),
    tag = "products"
)]
pub async fn get(
    State(state): State<AppState>,
    Path(label): Path<String>,
) -> AppResult<Json<ProductRead>> {
    let product = product_service::get(&state.pool, &label).await?;
    Ok(Json(product))
}

/// `PATCH /products/{label}` — partial update.
#[utoipa::path(
    patch,
    path = "/api/v1/products/{label}",
    params(("label" = String, Path, description = "Product label")),
    request_body = ProductUpdate,
    responses(
        (status = 200, description = "Updated", body = ProductRead),
        (status = 404, description = "Not found", body = crate::error::ErrorEnvelope),
        (status = 409, description = "Name conflict", body = crate::error::ErrorEnvelope),
        (status = 422, description = "Validation error", body = crate::error::ErrorEnvelope)
    ),
    tag = "products"
)]
pub async fn update(
    State(state): State<AppState>,
    Path(label): Path<String>,
    Json(input): Json<ProductUpdate>,
) -> AppResult<Json<ProductRead>> {
    let product = product_service::update(&state.pool, &label, input).await?;
    Ok(Json(product))
}

/// `DELETE /products/{label}` — delete.
#[utoipa::path(
    delete,
    path = "/api/v1/products/{label}",
    params(("label" = String, Path, description = "Product label")),
    responses(
        (status = 204, description = "Deleted"),
        (status = 404, description = "Not found", body = crate::error::ErrorEnvelope)
    ),
    tag = "products"
)]
pub async fn delete(
    State(state): State<AppState>,
    Path(label): Path<String>,
) -> AppResult<impl IntoResponse> {
    product_service::delete(&state.pool, &label).await?;
    Ok(StatusCode::NO_CONTENT)
}
