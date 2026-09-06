# Single Rule Canvas + Product Catalogue + Outcomes Library Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Collapse the three-canvas `rule_graph` to one Rule Canvas, add `logged_in`/`has_product` decision nodes backed by a new Product Catalogue, and add a global Outcomes library (saved Library component + version + variable values) usable from any rule via two new `apply_saved_outcome`/`apply_saved_outcome_json` action nodes.

**Architecture:** Backend-first for each new concept (migration → model → schema → service → routes → tests), then the proxy hot-path pieces that consume it (processors/appliers/caches), then the frontend UI. The single-canvas collapse touches all three layers and runs first so every later task targets the final `{canvas}` shape instead of the old three-key one.

**Tech Stack:** Rust (Axum, SQLx runtime queries, sqlx::migrate!, validator, thiserror, moka, mustache, ammonia), Next.js App Router + TypeScript strict + Zustand + TanStack Query + Zod + Tailwind, Playwright/Vitest, cargo test with testcontainers (`DOCKER_HOST`).

**Spec:** `docs/superpowers/specs/2026-09-06-single-canvas-outcomes-library-design.md`

## Global Constraints

- `backend/` and `proxy/` are standalone crates (empty `[workspace]`); build with `cargo build --manifest-path backend/Cargo.toml` / `--manifest-path proxy/Cargo.toml`. `PATH="$HOME/.cargo/bin:$PATH"` is required in non-interactive shells.
- SQLx: RUNTIME queries only (`sqlx::query_as::<_, T>(sqlx::AssertSqlSafe(sql))`), never `query!`/`query_as!`. Migrations run via embedded `sqlx::migrate!` at backend startup.
- Every DTO `#[serde(deny_unknown_fields)]` on write bodies; every response DTO plain `Serialize` + `ToSchema`.
- All manifest/wire JSON keys are snake_case, never camelCase.
- `make check` is the only gate (no CI) — run before calling any task done. Per-service: `make backend-check` / `make proxy-check` / `make frontend-check`.
- Node/eslint pins: node 24 floor, eslint 9 ceiling (do not upgrade to eslint 10).
- Follow existing file conventions exactly (see each task's "Mirror" pointer to an existing file) rather than inventing new patterns.
- Never add MIT headers to RRE files (FSL-1.1-MIT licensed); zen `core/*` stays untouched.

---

## Task 1: Backend — collapse `rule_graph` to a single Rule Canvas

**Files:**
- Create: `backend/migrations/0015_single_canvas.up.sql`, `backend/migrations/0015_single_canvas.down.sql`
- Modify: `backend/src/schemas/rule_graph.rs`, `backend/src/services/rule_graph_service.rs`, `backend/src/bin/seed_demo.rs`
- Test: `backend/src/services/rule_graph_service.rs` (inline `#[cfg(test)]`), `backend/tests/migrations.rs` (create if absent)

**Interfaces:**
- Produces: `pub struct RuleGraph { pub canvas: CanvasGraph }`, `RuleGraph::canvases() -> [(&'static str, &CanvasGraph); 1]` (kept as a 1-element array so `rule_graph_service::validate`'s existing `for (canvas_name, canvas) in graph.canvases()` loop needs no change beyond the array length).
- Consumes: nothing new (this is the root task).

- [ ] **Step 1: Write the migration**

`backend/migrations/0015_single_canvas.up.sql`:
```sql
ALTER TABLE rre.versions
  ALTER COLUMN rule_graph SET DEFAULT '{"canvas":{"nodes":[],"edges":[],"root_node_id":null}}'::jsonb;

UPDATE rre.versions
  SET rule_graph = jsonb_build_object('canvas', rule_graph->'anonymous');
```

`backend/migrations/0015_single_canvas.down.sql`:
```sql
ALTER TABLE rre.versions
  ALTER COLUMN rule_graph SET DEFAULT
    '{"anonymous":{"nodes":[],"edges":[],"root_node_id":null},"registered":{"nodes":[],"edges":[],"root_node_id":null},"customer":{"nodes":[],"edges":[],"root_node_id":null}}'::jsonb;

UPDATE rre.versions
  SET rule_graph = jsonb_build_object(
    'anonymous', rule_graph->'canvas',
    'registered', '{"nodes":[],"edges":[],"root_node_id":null}'::jsonb,
    'customer', '{"nodes":[],"edges":[],"root_node_id":null}'::jsonb
  );
```

- [ ] **Step 2: Collapse the `RuleGraph` schema**

In `backend/src/schemas/rule_graph.rs`, replace:
```rust
#[derive(Deserialize, Serialize, Clone, Debug, Default, PartialEq, ToSchema)]
pub struct RuleGraph {
    pub anonymous: CanvasGraph,
    pub registered: CanvasGraph,
    pub customer: CanvasGraph,
}

impl RuleGraph {
    pub fn canvases(&self) -> [(&'static str, &CanvasGraph); 3] {
        [
            ("anonymous", &self.anonymous),
            ("registered", &self.registered),
            ("customer", &self.customer),
        ]
    }
}
```
with:
```rust
#[derive(Deserialize, Serialize, Clone, Debug, Default, PartialEq, ToSchema)]
pub struct RuleGraph {
    pub canvas: CanvasGraph,
}

impl RuleGraph {
    /// Kept as a single-element array so callers written for the old
    /// three-canvas shape (`rule_graph_service::validate`, error `loc` builders)
    /// need no shape-specific change beyond iterating one entry.
    pub fn canvases(&self) -> [(&'static str, &CanvasGraph); 1] {
        [("canvas", &self.canvas)]
    }
}
```
Update the module doc comment's "one `CanvasGraph` per user class" language to "a single Rule Canvas".

- [ ] **Step 3: Run backend tests to find every three-canvas reference**

Run: `PATH="$HOME/.cargo/bin:$PATH" cargo build --manifest-path backend/Cargo.toml 2>&1 | grep -E "error\[|-->" | head -50`
Expected: a list of compile errors at every remaining `.anonymous`/`.registered`/`.customer` field access outside `rule_graph.rs` — this enumerates every call site that needs updating in this task (there should be none beyond `rule_graph_service.rs` tests and `seed_demo.rs`, since `validate`/`version_service` already use the shape-agnostic `canvases()` iterator per the spec).

- [ ] **Step 4: Fix `rule_graph_service.rs` tests**

`rule_graph_service.rs`'s `#[cfg(test)]` module builds `RuleGraph { anonymous: ..., registered: CanvasGraph::default(), customer: CanvasGraph::default() }` literals. Replace every such literal with `RuleGraph { canvas: ... }` (drop the `registered`/`customer` fields). The test `violations_accumulate_across_canvases` (asserts violations from 2+ canvases accumulate) becomes meaningless with one canvas — rename it `violations_accumulate_within_one_canvas` and rewrite it to assert multiple violation types accumulate within the single `canvas` (e.g. a bad edge endpoint AND a missing start node in the same graph, both appearing in `details`).

- [ ] **Step 5: Fix `seed_demo.rs`'s three graph-builder functions**

`anonymous_rule_graph()`, `draft_rule_graph()`, and `json_anonymous_rule_graph()` each build a `json!({ "anonymous": {...}, "registered": {...}, "customer": {...} })` literal. In each function, replace the outer wrapper so the former `"anonymous"` object becomes the value of a single `"canvas"` key, and delete the `"registered"`/`"customer"` keys entirely:
```rust
fn anonymous_rule_graph() -> serde_json::Value {
    json!({
        "canvas": {
            "root_node_id": "start",
            "nodes": [ /* ...unchanged, copy verbatim from the old "anonymous" value... */ ],
            "edges": [ /* ...unchanged... */ ]
        }
    })
}
```
Do this for all three functions, keeping every node/edge inside `"canvas"` byte-for-byte identical to what was inside the old `"anonymous"` key.

- [ ] **Step 6: Rebuild and run backend tests**

Run: `PATH="$HOME/.cargo/bin:$PATH" cargo build --manifest-path backend/Cargo.toml`
Expected: clean build.
Run: `PATH="$HOME/.cargo/bin:$PATH" DOCKER_HOST=unix:///var/run/docker.sock cargo test --manifest-path backend/Cargo.toml`
Expected: all tests pass, including the rewritten `rule_graph_service` tests.

- [ ] **Step 7: Add a migration round-trip test**

Create `backend/tests/migrations.rs` (or add to an existing migration test file if one already exists — check `backend/tests/` first) with a test that runs the embedded migrator up to head against a fresh testcontainers Postgres, inserts a version row with a three-key `rule_graph` shaped like the OLD default, confirms the 0015 `UPDATE` (run automatically as part of `sqlx::migrate!` applying all migrations in order) leaves `rule_graph->'canvas'` present. This only needs to verify the shape after a full `migrate!` run since down-migrations aren't invoked at runtime; skip a literal down/up round-trip if the project's existing migration tests don't do that (check `backend/tests/` for an existing pattern to mirror first).

- [ ] **Step 8: Commit**

```bash
git add backend/migrations/0015_single_canvas.up.sql backend/migrations/0015_single_canvas.down.sql \
  backend/src/schemas/rule_graph.rs backend/src/services/rule_graph_service.rs backend/src/bin/seed_demo.rs \
  backend/tests/migrations.rs
git commit -m "feat(backend): collapse rule_graph to a single Rule Canvas"
```

---

## Task 2: Backend — Product Catalogue (`rre.products` CRUD)

**Files:**
- Create: `backend/migrations/0016_products.up.sql`, `backend/migrations/0016_products.down.sql`, `backend/src/models/product.rs`, `backend/src/schemas/product.rs`, `backend/src/repositories/product_repository.rs`, `backend/src/services/product_service.rs`, `backend/src/api/v1/products.rs`
- Modify: `backend/src/models/mod.rs`, `backend/src/schemas/mod.rs`, `backend/src/repositories/mod.rs`, `backend/src/services/mod.rs`, `backend/src/api/v1/mod.rs`, `backend/src/error.rs`
- Test: inline `#[cfg(test)]` in `schemas/product.rs` + `services/product_service.rs`; `backend/tests/products.rs`

**Interfaces:**
- Consumes: `crate::schemas::feature::SLUG_RE` pattern (mirror it with a new `PRODUCT_LABEL_RE`), `crate::error::{AppError, AppResult, ValidationDetail}`, `crate::schemas::pagination::{Page, PageParams}`.
- Produces: `product_service::{create, list, get, update, delete}`, `AppError::ProductNotFound(String)` → 404 `PRODUCT_NOT_FOUND`, reuses `AppError::SlugConflict` → 409 `SLUG_CONFLICT`. Routes at `/api/v1/products`.

- [ ] **Step 1: Migration**

`backend/migrations/0016_products.up.sql`:
```sql
CREATE TABLE rre.products (
  label VARCHAR(64) PRIMARY KEY,
  name VARCHAR(200) NOT NULL UNIQUE,
  description VARCHAR(500),
  created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX products_created_at_idx ON rre.products (created_at DESC, label ASC);
ALTER TABLE rre.products ADD CONSTRAINT products_label_snake_case
  CHECK (label ~ '^[a-z0-9]+(_[a-z0-9]+)*$');
```
`backend/migrations/0016_products.down.sql`:
```sql
DROP TABLE rre.products;
```

- [ ] **Step 2: Model**

`backend/src/models/product.rs`:
```rust
//! Product domain model (`sqlx::FromRow`). Never serialized on the API
//! boundary — mapped to [`crate::schemas::product::ProductRead`] in the service.

use chrono::{DateTime, Utc};

/// A product row from `rre.products`: a flat entitlement label a visitor may
/// hold, tested by the `has_product` decision node.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct Product {
    /// Snake_case label primary key, immutable after create.
    pub label: String,
    /// Human-readable name (unique).
    pub name: String,
    /// Optional description.
    pub description: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}
```
Add `pub mod product;` to `backend/src/models/mod.rs`.

- [ ] **Step 3: Schema**

`backend/src/schemas/product.rs` — mirror `backend/src/schemas/site.rs`'s shape exactly, minus the header-validation machinery:
```rust
//! Product DTOs (Product Catalogue design).
//!
//! `ProductCreate`/`ProductUpdate` are request bodies (validated); `ProductRead`
//! is the response shape. The label regex enforces snake_case
//! (`^[a-z0-9]+(_[a-z0-9]+)*$`) and is immutable after create.

use chrono::{DateTime, Utc};
use once_cell::sync::Lazy;
use regex::Regex;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use validator::Validate;

use crate::models::product::Product;

/// Snake_case label grammar, matching the DB CHECK constraint.
pub static PRODUCT_LABEL_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"^[a-z0-9]+(_[a-z0-9]+)*$").expect("valid product label regex"));

#[derive(Debug, Deserialize, Validate, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct ProductCreate {
    #[validate(length(min = 1, max = 64), regex(path = *PRODUCT_LABEL_RE))]
    pub label: String,
    #[validate(length(min = 1, max = 200))]
    pub name: String,
    #[validate(length(max = 500))]
    pub description: Option<String>,
}

#[derive(Debug, Default, Deserialize, Validate, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct ProductUpdate {
    #[validate(length(min = 1, max = 200))]
    pub name: Option<String>,
    #[validate(length(max = 500))]
    pub description: Option<String>,
}

impl ProductUpdate {
    pub fn is_empty(&self) -> bool {
        self.name.is_none() && self.description.is_none()
    }
}

#[derive(Debug, Serialize, ToSchema)]
pub struct ProductRead {
    pub label: String,
    pub name: String,
    pub description: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl From<Product> for ProductRead {
    fn from(p: Product) -> Self {
        Self {
            label: p.label,
            name: p.name,
            description: p.description,
            created_at: p.created_at,
            updated_at: p.updated_at,
        }
    }
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
    fn create_input_rejects_uppercase_label() {
        assert!(create_input("Premium").validate().is_err());
    }

    #[test]
    fn create_input_rejects_kebab_case_label() {
        assert!(create_input("premium-tier").validate().is_err());
    }

    #[test]
    fn create_input_rejects_empty_name() {
        let mut input = create_input("premium");
        input.name = String::new();
        assert!(input.validate().is_err());
    }

    #[test]
    fn update_input_accepts_partial() {
        let input = ProductUpdate {
            name: Some("Renamed".to_string()),
            ..ProductUpdate::default()
        };
        assert!(input.validate().is_ok());
        assert!(!input.is_empty());
        assert!(ProductUpdate::default().is_empty());
    }
}
```
Add `pub mod product;` to `backend/src/schemas/mod.rs`.

- [ ] **Step 4: Repository**

`backend/src/repositories/product_repository.rs` — mirror `site_repository.rs` exactly, dropping the `headers` column and the multi-field update in favor of `name`/`description`:
```rust
//! Product data-access (RUNTIME SQLx only). Returns [`Product`] models.

use sqlx::PgPool;

use crate::models::product::Product;

const COLS: &str = "label, name, description, created_at, updated_at";

pub async fn insert(
    pool: &PgPool,
    label: &str,
    name: &str,
    description: Option<&str>,
) -> Result<Product, sqlx::Error> {
    let sql = format!(
        "INSERT INTO rre.products (label, name, description) VALUES ($1, $2, $3) RETURNING {COLS}"
    );
    sqlx::query_as::<_, Product>(sqlx::AssertSqlSafe(sql))
        .bind(label)
        .bind(name)
        .bind(description)
        .fetch_one(pool)
        .await
}

pub async fn get(pool: &PgPool, label: &str) -> Result<Option<Product>, sqlx::Error> {
    let sql = format!("SELECT {COLS} FROM rre.products WHERE label = $1");
    sqlx::query_as::<_, Product>(sqlx::AssertSqlSafe(sql))
        .bind(label)
        .fetch_optional(pool)
        .await
}

pub async fn list_paged(
    pool: &PgPool,
    limit: i64,
    offset: i64,
    q: Option<&str>,
) -> Result<Vec<Product>, sqlx::Error> {
    let sql = format!(
        "SELECT {COLS} FROM rre.products \
         WHERE ($3::text IS NULL OR name ILIKE '%' || $3 || '%') \
         ORDER BY created_at DESC, label ASC LIMIT $1 OFFSET $2"
    );
    sqlx::query_as::<_, Product>(sqlx::AssertSqlSafe(sql))
        .bind(limit)
        .bind(offset)
        .bind(q)
        .fetch_all(pool)
        .await
}

pub async fn count(pool: &PgPool, q: Option<&str>) -> Result<i64, sqlx::Error> {
    sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM rre.products WHERE ($1::text IS NULL OR name ILIKE '%' || $1 || '%')",
    )
    .bind(q)
    .fetch_one(pool)
    .await
}

pub async fn update(
    pool: &PgPool,
    label: &str,
    name: Option<&str>,
    description: Option<&str>,
    clear_description: bool,
) -> Result<Option<Product>, sqlx::Error> {
    let sql = format!(
        "UPDATE rre.products SET \
            name = COALESCE($2, name), \
            description = CASE WHEN $4 THEN NULL ELSE COALESCE($3, description) END, \
            updated_at = now() \
         WHERE label = $1 RETURNING {COLS}"
    );
    sqlx::query_as::<_, Product>(sqlx::AssertSqlSafe(sql))
        .bind(label)
        .bind(name)
        .bind(description)
        .bind(clear_description)
        .fetch_optional(pool)
        .await
}

pub async fn delete(pool: &PgPool, label: &str) -> Result<u64, sqlx::Error> {
    let res = sqlx::query("DELETE FROM rre.products WHERE label = $1")
        .bind(label)
        .execute(pool)
        .await?;
    Ok(res.rows_affected())
}
```
Note: `description: Option<String>` in `ProductUpdate` can't distinguish "leave unchanged" from "clear to null" with a plain `Option`. Since the spec doesn't require clearing an existing description to null (only setting a new one), keep this simple: `clear_description` is always `false` from the service for now (pass `false` at the call site) — `description: None` in the update DTO means "don't touch it". This matches `SiteUpdate`'s simpler fields (no clear-to-null distinction needed there either).
Add `pub mod product_repository;` to `backend/src/repositories/mod.rs`.

- [ ] **Step 5: Service**

`backend/src/services/product_service.rs` — mirror `site_service.rs`:
```rust
//! Product Catalogue business logic.

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

const PG_UNIQUE_VIOLATION: &str = "23505";

pub async fn create(pool: &PgPool, input: ProductCreate) -> AppResult<ProductRead> {
    input.validate().map_err(AppError::from)?;
    match repo::insert(pool, &input.label, &input.name, input.description.as_deref()).await {
        Ok(product) => Ok(product.into()),
        Err(err) => Err(map_conflict_error(err)),
    }
}

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

pub async fn get(pool: &PgPool, label: &str) -> AppResult<ProductRead> {
    let product = repo::get(pool, label).await?.ok_or_else(|| not_found(label))?;
    Ok(product.into())
}

pub async fn update(pool: &PgPool, label: &str, input: ProductUpdate) -> AppResult<ProductRead> {
    input.validate().map_err(AppError::from)?;
    if input.is_empty() {
        return get(pool, label).await;
    }
    match repo::update(pool, label, input.name.as_deref(), input.description.as_deref(), false).await
    {
        Ok(Some(product)) => Ok(product.into()),
        Ok(None) => Err(not_found(label)),
        Err(err) => Err(map_conflict_error(err)),
    }
}

pub async fn delete(pool: &PgPool, label: &str) -> AppResult<()> {
    let deleted = repo::delete(pool, label).await?;
    if deleted == 0 {
        return Err(not_found(label));
    }
    Ok(())
}

fn not_found(label: &str) -> AppError {
    AppError::ProductNotFound(format!("Product '{label}' not found"))
}

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
    fn non_unique_db_error_falls_through_to_internal() {
        let mapped = map_conflict_error(sqlx::Error::RowNotFound);
        assert!(matches!(mapped, AppError::Internal(_)));
    }
}
```
Add `pub mod product_service;` to `backend/src/services/mod.rs`.

- [ ] **Step 6: Add `ProductNotFound` to `AppError`**

In `backend/src/error.rs`, add a variant next to `SiteNotFound`:
```rust
/// Product label missing. → 404
#[error("{0}")]
ProductNotFound(String),
```
Add its arm to `code()`:
```rust
AppError::ProductNotFound(_) => "PRODUCT_NOT_FOUND",
```
And to the `status_code()` / `IntoResponse` match (mirror exactly how `SiteNotFound` maps to 404 — find that arm and add `AppError::ProductNotFound(_) => StatusCode::NOT_FOUND,` alongside it).

- [ ] **Step 7: Routes**

`backend/src/api/v1/products.rs` — mirror `backend/src/api/v1/sites.rs` exactly, dropping the header-specific parts:
```rust
//! Product Catalogue HTTP handlers.

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

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/products", post(create).get(list))
        .route(
            "/products/{label}",
            axum::routing::get(get).patch(update).delete(delete),
        )
}

#[derive(Debug, Default, Deserialize)]
pub struct ProductListQuery {
    #[serde(default)]
    pub q: Option<String>,
}

#[utoipa::path(
    post, path = "/api/v1/products", request_body = ProductCreate,
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

#[utoipa::path(
    get, path = "/api/v1/products",
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

#[utoipa::path(
    get, path = "/api/v1/products/{label}",
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

#[utoipa::path(
    patch, path = "/api/v1/products/{label}",
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

#[utoipa::path(
    delete, path = "/api/v1/products/{label}",
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
```
Mount it in `backend/src/api/v1/mod.rs` the same way `sites::router()` is merged into the v1 router (find that `.merge(sites::router())` line and add `.merge(products::router())` beside it; add `pub mod products;` to the module list).

- [ ] **Step 8: Integration test**

Create `backend/tests/products.rs` mirroring the existing `backend/tests/sites.rs`-equivalent test file's testcontainers harness (find and open the sites test file first to copy its harness setup verbatim, then adapt). Cover: create + get round-trip, dup label → 409, dup name → 409, uppercase/kebab label → 422, list with `q` filter, update partial, delete then 404.

- [ ] **Step 9: Run backend checks**

Run: `cd backend && PATH="$HOME/.cargo/bin:$PATH" cargo fmt && cargo clippy --all-targets -- -D warnings && DOCKER_HOST=unix:///var/run/docker.sock cargo test`
Expected: clean.

- [ ] **Step 10: Commit**

```bash
git add backend/migrations/0016_products.up.sql backend/migrations/0016_products.down.sql \
  backend/src/models/product.rs backend/src/models/mod.rs \
  backend/src/schemas/product.rs backend/src/schemas/mod.rs \
  backend/src/repositories/product_repository.rs backend/src/repositories/mod.rs \
  backend/src/services/product_service.rs backend/src/services/mod.rs \
  backend/src/api/v1/products.rs backend/src/api/v1/mod.rs backend/src/error.rs \
  backend/tests/products.rs
git commit -m "feat(backend): add Product Catalogue CRUD (rre.products)"
```

---

## Task 3: Backend — `logged_in`/`has_product` decision nodes + validation

**Files:**
- Modify: `backend/config/node_types.json`, `backend/src/schemas/node_type.rs`, `backend/src/services/rule_graph_service.rs`, wherever `validate` is invoked with `valid_component_ids` (find the call site in `version_service.rs` and thread a new `valid_product_labels` argument alongside it)
- Test: inline in `rule_graph_service.rs`

**Interfaces:**
- Consumes: Task 1's collapsed `RuleGraph`/`CanvasGraph`, Task 2's `rre.products` table (via a repo call in `version_service.rs` fetching all labels into a `HashSet<String>`).
- Produces: `Control::ProductSelect` variant, `validate(graph, valid_outcome_ids, valid_component_ids, valid_product_labels: &HashSet<String>, manifest)` (new 4th positional param inserted before `manifest`), rule_id `has_product_ref_exists`.

- [ ] **Step 1: Add the two node types to the manifest**

In `backend/config/node_types.json`, find the `categories` array entry for `"id": "user"` and remove `"coming_soon": true` from it (or delete the key entirely — `#[serde(default)]` treats absence as `false`). Then add two entries to `node_types` (anywhere in the array; palette order follows array order, so place them near other `content`/`session`-adjacent decision nodes):
```json
{
  "kind": "logged_in",
  "label": "Logged In",
  "category": "user",
  "summary": "Branch on whether the visitor is logged in",
  "applies_to": "all",
  "node_kind": "decision",
  "fields": [],
  "output": { "branches": [{ "id": "yes" }, { "id": "no" }] }
},
{
  "kind": "has_product",
  "label": "Has Product",
  "category": "user",
  "summary": "Branch on whether the visitor holds a chosen product",
  "applies_to": "all",
  "node_kind": "decision",
  "fields": [
    {
      "name": "product",
      "label": "Product",
      "control": "product_select",
      "required": true,
      "placeholder": "Search products…"
    }
  ],
  "output": { "branches": [{ "id": "yes" }, { "id": "no" }] }
}
```

- [ ] **Step 2: Add the `ProductSelect` control variant**

In `backend/src/schemas/node_type.rs`, find `pub enum Control` and add a variant next to `SiteSelect`:
```rust
/// Dynamic searchable single-select of configured products. Options are NOT in
/// the manifest; the client queries `GET /api/v1/products?q=` and stores the
/// selected product's label.
ProductSelect,
```
Check the enum's `#[serde(rename_all = ...)]` attribute (mirror `SiteSelect` → `"site_select"` exactly so `ProductSelect` serializes as `"product_select"`).

- [ ] **Step 3: Write the failing validation test**

In `rule_graph_service.rs`'s test module, add:
```rust
#[test]
fn has_product_ref_exists_rejects_unknown_label() {
    let graph = single_node_decision_graph("has_product", json!({ "product": "ghost" }));
    let manifest = test_manifest(); // however the existing tests build a NodeManifest fixture
    let valid_products: HashSet<String> = HashSet::new();
    let result = validate(
        &graph,
        &HashSet::new(),
        &HashSet::new(),
        &valid_products,
        &manifest,
    );
    let details = match result {
        Err(AppError::Validation { details }) => details,
        other => panic!("expected validation error, got {other:?}"),
    };
    assert!(details.iter().any(|d| d.rule_id == "has_product_ref_exists"));
}

#[test]
fn has_product_ref_exists_accepts_known_label() {
    let graph = single_node_decision_graph("has_product", json!({ "product": "premium" }));
    let manifest = test_manifest();
    let mut valid_products = HashSet::new();
    valid_products.insert("premium".to_string());
    let result = validate(&graph, &HashSet::new(), &HashSet::new(), &valid_products, &manifest);
    assert!(result.is_ok());
}
```
(Reuse whatever helper the existing tests use to build a one-decision-node `RuleGraph` fixture — grep the test module for a helper like `single_node_decision_graph` or similar; if none exists, write one inline that constructs a `CanvasGraph` with a `start -> decision -> end` shape.)

- [ ] **Step 4: Run the test to see it fail to compile**

Run: `PATH="$HOME/.cargo/bin:$PATH" cargo test --manifest-path backend/Cargo.toml has_product_ref_exists 2>&1 | tail -30`
Expected: compile error — `validate` takes 4 args, not 5 (the new `valid_product_labels` param doesn't exist yet).

- [ ] **Step 5: Add the `valid_product_labels` parameter and the validation rule**

In `rule_graph_service.rs`, update `validate`'s signature:
```rust
pub fn validate(
    graph: &RuleGraph,
    valid_outcome_ids: &HashSet<Uuid>,
    valid_component_ids: &HashSet<Uuid>,
    valid_product_labels: &HashSet<String>,
    manifest: &NodeManifest,
) -> AppResult<()> {
```
Thread it into `validate_canvas` the same way `valid_component_ids` is threaded (add the parameter to `validate_canvas`'s signature and its call site inside `validate`). Inside `validate_canvas`'s node loop, in the `Node::Decision { processor, .. }` arm, add a call after `validate_processor`:
```rust
Node::Decision { processor, .. } => {
    validate_processor(canvas, idx, processor, spec_by_kind, details);
    validate_has_product_ref(canvas, idx, processor, valid_product_labels, details);
}
```
Add the new function near `validate_apply_component_ref`:
```rust
/// `has_product_ref_exists`: a Decision node whose `processor.type ==
/// "has_product"` has `processor.product` present in `valid_product_labels`.
/// Other processor kinds are ignored. Mirrors `validate_apply_component_ref`.
fn validate_has_product_ref(
    canvas: &'static str,
    idx: usize,
    processor: &ProcessorConfig,
    valid_product_labels: &HashSet<String>,
    details: &mut Vec<ValidationDetail>,
) {
    if processor.r#type != "has_product" {
        return;
    }
    let product = processor.fields.get("product").and_then(|v| v.as_str());
    match product {
        Some(label) if valid_product_labels.contains(label) => {}
        _ => {
            details.push(ValidationDetail::new(
                format!("rule_graph.{canvas}.nodes[{idx}].processor.product"),
                "referenced product does not exist".to_string(),
                "has_product_ref_exists",
            ));
        }
    }
}
```

- [ ] **Step 6: Run the tests to verify they pass**

Run: `PATH="$HOME/.cargo/bin:$PATH" cargo test --manifest-path backend/Cargo.toml has_product_ref_exists`
Expected: both new tests pass.

- [ ] **Step 7: Thread `valid_product_labels` through the real call site**

Find where `rule_graph_service::validate` is invoked from `version_service.rs` (it already fetches `valid_component_ids` there — grep for that call). Add a sibling fetch of all product labels (`product_repository::list_paged` with a large page size, or add a small `list_all_labels(pool) -> HashSet<String>` helper to `product_repository.rs` that does `SELECT label FROM rre.products`) and pass the resulting `HashSet<String>` as the new argument.

- [ ] **Step 8: Rebuild and run full backend test suite**

Run: `PATH="$HOME/.cargo/bin:$PATH" cargo build --manifest-path backend/Cargo.toml && DOCKER_HOST=unix:///var/run/docker.sock cargo test --manifest-path backend/Cargo.toml`
Expected: clean build, all tests pass (this also catches any other `validate(...)` call site the grep in Step 7 might have missed).

- [ ] **Step 9: Commit**

```bash
git add backend/config/node_types.json backend/src/schemas/node_type.rs backend/src/services/rule_graph_service.rs \
  backend/src/repositories/product_repository.rs backend/src/services/version_service.rs
git commit -m "feat(backend): add logged_in/has_product decision nodes + validation"
```

---

## Task 4: Backend — Outcomes Library (`rre.saved_outcomes`) + `apply_saved_outcome(_json)` nodes

**Files:**
- Create: `backend/migrations/0017_saved_outcomes.up.sql`, `.down.sql`, `backend/src/models/saved_outcome.rs`, `backend/src/schemas/saved_outcome.rs`, `backend/src/repositories/saved_outcome_repository.rs`, `backend/src/services/saved_outcome_service.rs`, `backend/src/api/v1/saved_outcomes.rs`
- Modify: `backend/config/node_types.json`, `backend/src/schemas/mod.rs`, `backend/src/models/mod.rs`, `backend/src/repositories/mod.rs`, `backend/src/services/mod.rs`, `backend/src/api/v1/mod.rs`, `backend/src/error.rs`, `backend/src/services/rule_graph_service.rs`, `backend/Cargo.toml`
- Test: `backend/tests/saved_outcomes.rs`, inline in `rule_graph_service.rs`

**Interfaces:**
- Consumes: `component_template_service`'s existing resolve logic (the function that backs `GET /component-templates/{cid}/resolve` — read `backend/src/services/component_template_service.rs` to find its exact name, e.g. `resolve`, and its return shape `ResolvedComponentRead { version_number, html_body, variables }`), `mustache` crate (add to `backend/Cargo.toml` as a new dependency: `mustache = "0.9"`, matching the proxy's pinned version — check `proxy/Cargo.toml` for the exact version string and match it).
- Produces: `saved_outcome_service::{create, list, get, update, delete, resolve}`, `SavedOutcomeRead`, `ResolvedSavedOutcomeRead { html_body: String }`, rule_id `saved_outcome_ref_exists`, manifest kinds `apply_saved_outcome`/`apply_saved_outcome_json`.

- [ ] **Step 1: Migration**

`backend/migrations/0017_saved_outcomes.up.sql`:
```sql
CREATE TABLE rre.saved_outcomes (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  slug VARCHAR(120) NOT NULL UNIQUE,
  name VARCHAR(200) NOT NULL UNIQUE,
  component_id UUID NOT NULL REFERENCES rre.component_templates(id) ON DELETE RESTRICT,
  version_number INTEGER,
  variables JSONB NOT NULL DEFAULT '{}'::jsonb,
  created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX saved_outcomes_created_at_idx ON rre.saved_outcomes (created_at DESC, slug ASC);
```
`.down.sql`:
```sql
DROP TABLE rre.saved_outcomes;
```

- [ ] **Step 2: Model**

`backend/src/models/saved_outcome.rs`:
```rust
//! Saved-outcome domain model (`sqlx::FromRow`). Never serialized on the API
//! boundary directly — mapped to [`crate::schemas::saved_outcome::SavedOutcomeRead`].

use chrono::{DateTime, Utc};
use uuid::Uuid;

/// A `rre.saved_outcomes` row: a named, reusable reference to one Library
/// component version plus fixed variable values, usable from any rule via the
/// `apply_saved_outcome`/`apply_saved_outcome_json` action nodes.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct SavedOutcome {
    pub id: Uuid,
    pub slug: String,
    pub name: String,
    pub component_id: Uuid,
    /// `None` = "Latest" (tracks the component's own default pointer).
    pub version_number: Option<i32>,
    pub variables: serde_json::Value,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}
```
Add `pub mod saved_outcome;` to `backend/src/models/mod.rs`.

- [ ] **Step 3: Schema**

`backend/src/schemas/saved_outcome.rs`:
```rust
//! Saved-outcome DTOs (Outcomes Library design).

use std::collections::HashMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;
use validator::Validate;

use crate::{models::saved_outcome::SavedOutcome, schemas::feature::SLUG_RE};

#[derive(Debug, Deserialize, Validate, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct SavedOutcomeCreate {
    #[validate(length(min = 3, max = 120), regex(path = *SLUG_RE))]
    pub slug: String,
    #[validate(length(min = 1, max = 200))]
    pub name: String,
    pub component_id: Uuid,
    /// `None`/absent = "Latest".
    #[serde(default)]
    pub version_number: Option<i32>,
    #[serde(default)]
    pub variables: HashMap<String, String>,
}

/// `PATCH` body. `version_number` uses the double-Option convention: field
/// OMITTED -> unchanged; present as JSON `null` -> clear to "Latest"; present
/// with a value -> pin that version.
#[derive(Debug, Default, Deserialize, Validate, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct SavedOutcomeUpdate {
    #[validate(length(min = 1, max = 200))]
    pub name: Option<String>,
    #[serde(default, with = "::serde_with::rust::double_option")]
    pub version_number: Option<Option<i32>>,
    pub variables: Option<HashMap<String, String>>,
}

impl SavedOutcomeUpdate {
    pub fn is_empty(&self) -> bool {
        self.name.is_none() && self.version_number.is_none() && self.variables.is_none()
    }
}

#[derive(Debug, Serialize, ToSchema)]
pub struct SavedOutcomeRead {
    pub id: Uuid,
    pub slug: String,
    pub name: String,
    pub component_id: Uuid,
    /// Denormalized for list display; joined at read time.
    pub component_name: String,
    pub version_number: Option<i32>,
    pub variables: HashMap<String, String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Proxy-facing resolved-and-rendered shape (already-rendered `html_body`; the
/// applier injects it as-is, no client-side mustache pass).
#[derive(Debug, Serialize, ToSchema)]
pub struct ResolvedSavedOutcomeRead {
    pub html_body: String,
}

/// Build a `SavedOutcomeRead` from a row plus its joined component name.
pub fn to_read(row: SavedOutcome, component_name: String) -> SavedOutcomeRead {
    let variables: HashMap<String, String> = row
        .variables
        .as_object()
        .map(|m| {
            m.iter()
                .filter_map(|(k, v)| v.as_str().map(|s| (k.clone(), s.to_string())))
                .collect()
        })
        .unwrap_or_default();
    SavedOutcomeRead {
        id: row.id,
        slug: row.slug,
        name: row.name,
        component_id: row.component_id,
        component_name,
        version_number: row.version_number,
        variables,
        created_at: row.created_at,
        updated_at: row.updated_at,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn create_input_accepts_valid() {
        let input = SavedOutcomeCreate {
            slug: "promo-banner-default".to_string(),
            name: "Promo Banner (Default)".to_string(),
            component_id: Uuid::new_v4(),
            version_number: None,
            variables: HashMap::new(),
        };
        assert!(input.validate().is_ok());
    }

    #[test]
    fn create_input_rejects_short_slug() {
        let input = SavedOutcomeCreate {
            slug: "ab".to_string(),
            name: "X".to_string(),
            component_id: Uuid::new_v4(),
            version_number: None,
            variables: HashMap::new(),
        };
        assert!(input.validate().is_err());
    }
}
```
Add `pub mod saved_outcome;` to `backend/src/schemas/mod.rs`. Add `serde_with = "3"` to `backend/Cargo.toml` if not already a dependency (check first: `grep serde_with backend/Cargo.toml`); if the project has an established double-Option convention already in use elsewhere in the schemas crate (grep for `double_option` or `Option<Option<`), use that existing convention instead of adding a new dependency.

- [ ] **Step 4: Repository**

`backend/src/repositories/saved_outcome_repository.rs`:
```rust
//! Saved-outcome data-access (RUNTIME SQLx only).

use sqlx::PgPool;
use uuid::Uuid;

use crate::models::saved_outcome::SavedOutcome;

const COLS: &str = "id, slug, name, component_id, version_number, variables, created_at, updated_at";

#[allow(clippy::too_many_arguments)]
pub async fn insert(
    pool: &PgPool,
    slug: &str,
    name: &str,
    component_id: Uuid,
    version_number: Option<i32>,
    variables: &serde_json::Value,
) -> Result<SavedOutcome, sqlx::Error> {
    let sql = format!(
        "INSERT INTO rre.saved_outcomes (slug, name, component_id, version_number, variables) \
         VALUES ($1, $2, $3, $4, $5) RETURNING {COLS}"
    );
    sqlx::query_as::<_, SavedOutcome>(sqlx::AssertSqlSafe(sql))
        .bind(slug)
        .bind(name)
        .bind(component_id)
        .bind(version_number)
        .bind(variables)
        .fetch_one(pool)
        .await
}

pub async fn get(pool: &PgPool, id: Uuid) -> Result<Option<SavedOutcome>, sqlx::Error> {
    let sql = format!("SELECT {COLS} FROM rre.saved_outcomes WHERE id = $1");
    sqlx::query_as::<_, SavedOutcome>(sqlx::AssertSqlSafe(sql))
        .bind(id)
        .fetch_optional(pool)
        .await
}

pub async fn list_paged(
    pool: &PgPool,
    limit: i64,
    offset: i64,
    q: Option<&str>,
) -> Result<Vec<SavedOutcome>, sqlx::Error> {
    let sql = format!(
        "SELECT {COLS} FROM rre.saved_outcomes \
         WHERE ($3::text IS NULL OR name ILIKE '%' || $3 || '%') \
         ORDER BY created_at DESC, slug ASC LIMIT $1 OFFSET $2"
    );
    sqlx::query_as::<_, SavedOutcome>(sqlx::AssertSqlSafe(sql))
        .bind(limit)
        .bind(offset)
        .bind(q)
        .fetch_all(pool)
        .await
}

pub async fn count(pool: &PgPool, q: Option<&str>) -> Result<i64, sqlx::Error> {
    sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM rre.saved_outcomes WHERE ($1::text IS NULL OR name ILIKE '%' || $1 || '%')",
    )
    .bind(q)
    .fetch_one(pool)
    .await
}

/// `version_number`: `None` = leave unchanged; the CALLER resolves the
/// double-Option (`Some(None)` = clear, `Some(Some(n))` = pin, `None` = keep)
/// into the plain `Option<Option<i32>>` shown here before calling this repo fn,
/// since SQL COALESCE can't express three states over one bind — instead this
/// takes an explicit `set_version: bool` + `version_number: Option<i32>` pair.
pub async fn update(
    pool: &PgPool,
    id: Uuid,
    name: Option<&str>,
    set_version: bool,
    version_number: Option<i32>,
    variables: Option<&serde_json::Value>,
) -> Result<Option<SavedOutcome>, sqlx::Error> {
    let sql = format!(
        "UPDATE rre.saved_outcomes SET \
            name = COALESCE($2, name), \
            version_number = CASE WHEN $3 THEN $4 ELSE version_number END, \
            variables = COALESCE($5, variables), \
            updated_at = now() \
         WHERE id = $1 RETURNING {COLS}"
    );
    sqlx::query_as::<_, SavedOutcome>(sqlx::AssertSqlSafe(sql))
        .bind(id)
        .bind(name)
        .bind(set_version)
        .bind(version_number)
        .bind(variables)
        .fetch_optional(pool)
        .await
}

pub async fn delete(pool: &PgPool, id: Uuid) -> Result<u64, sqlx::Error> {
    let res = sqlx::query("DELETE FROM rre.saved_outcomes WHERE id = $1")
        .bind(id)
        .execute(pool)
        .await?;
    Ok(res.rows_affected())
}

/// All ids currently in use (for `saved_outcome_ref_exists` validation).
pub async fn list_all_ids(pool: &PgPool) -> Result<Vec<Uuid>, sqlx::Error> {
    sqlx::query_scalar::<_, Uuid>("SELECT id FROM rre.saved_outcomes")
        .fetch_all(pool)
        .await
}
```
Add `pub mod saved_outcome_repository;` to `backend/src/repositories/mod.rs`. Add a matching `pub async fn list_all_labels(pool: &PgPool) -> Result<Vec<String>, sqlx::Error>` to `product_repository.rs` too (from Task 2/3) if it wasn't already added there — `SELECT label FROM rre.products`.

- [ ] **Step 5: Service (including the FK-restrict → 409 mapping and the render-on-resolve path)**

First, open `backend/src/services/component_template_service.rs` and note the exact name/signature of the function backing `GET /component-templates/{cid}/resolve` (per CONTRACTS §5 it's `component_template_service::resolve`, returning something shaped like `ResolvedComponentRead { version_number, html_body, variables }` — confirm the exact type name before writing this service).

`backend/src/services/saved_outcome_service.rs`:
```rust
//! Outcomes Library business logic. `resolve` renders the referenced
//! component version's `html_body` against this saved outcome's own
//! `variables` (flat mustache, matching the proxy's `component_render`
//! semantics) and returns the ALREADY-RENDERED HTML — the proxy applier
//! injects it as-is, with no client-side render step.

use std::collections::HashMap;

use sqlx::PgPool;
use uuid::Uuid;
use validator::Validate;

use crate::{
    error::{AppError, AppResult},
    repositories::{component_template_repository, saved_outcome_repository as repo},
    schemas::{
        component_template::VersionSelector, // adjust import path to whatever the
                                              // component_template_service actually
                                              // exports for "default"/"pinned"
        pagination::{Page, PageParams},
        saved_outcome::{
            self, ResolvedSavedOutcomeRead, SavedOutcomeCreate, SavedOutcomeRead,
            SavedOutcomeUpdate,
        },
    },
    services::component_template_service,
};

const PG_UNIQUE_VIOLATION: &str = "23505";
const PG_FOREIGN_KEY_VIOLATION: &str = "23503";

fn variables_to_json(vars: &HashMap<String, String>) -> serde_json::Value {
    serde_json::json!(vars)
}

pub async fn create(pool: &PgPool, input: SavedOutcomeCreate) -> AppResult<SavedOutcomeRead> {
    input.validate().map_err(AppError::from)?;
    let variables_json = variables_to_json(&input.variables);
    let row = repo::insert(
        pool,
        &input.slug,
        &input.name,
        input.component_id,
        input.version_number,
        &variables_json,
    )
    .await
    .map_err(map_write_error)?;
    to_read(pool, row).await
}

pub async fn list(
    pool: &PgPool,
    params: &PageParams,
    q: Option<&str>,
) -> AppResult<Page<SavedOutcomeRead>> {
    let (limit, offset, page, page_size) = params.resolve();
    let q = q.filter(|s| !s.is_empty());
    let (rows, total) = tokio::try_join!(
        repo::list_paged(pool, limit, offset, q),
        repo::count(pool, q)
    )?;
    let mut items = Vec::with_capacity(rows.len());
    for row in rows {
        items.push(to_read(pool, row).await?);
    }
    Ok(Page::new(items, page, page_size, total))
}

pub async fn get(pool: &PgPool, id: Uuid) -> AppResult<SavedOutcomeRead> {
    let row = repo::get(pool, id).await?.ok_or_else(|| not_found(id))?;
    to_read(pool, row).await
}

pub async fn update(
    pool: &PgPool,
    id: Uuid,
    input: SavedOutcomeUpdate,
) -> AppResult<SavedOutcomeRead> {
    input.validate().map_err(AppError::from)?;
    if input.is_empty() {
        return get(pool, id).await;
    }
    let (set_version, version_number) = match input.version_number {
        None => (false, None),
        Some(inner) => (true, inner),
    };
    let variables_json = input.variables.as_ref().map(variables_to_json);
    let row = repo::update(
        pool,
        id,
        input.name.as_deref(),
        set_version,
        version_number,
        variables_json.as_ref(),
    )
    .await
    .map_err(map_write_error)?
    .ok_or_else(|| not_found(id))?;
    to_read(pool, row).await
}

pub async fn delete(pool: &PgPool, id: Uuid) -> AppResult<()> {
    let deleted = repo::delete(pool, id).await?;
    if deleted == 0 {
        return Err(not_found(id));
    }
    Ok(())
}

/// `GET /saved-outcomes/{id}/resolve` — the proxy-facing route. Resolves the
/// referenced component version (reusing `component_template_service`'s own
/// resolve, `version=default` when `version_number` is `None`) and renders its
/// `html_body` against this saved outcome's `variables` with `mustache`
/// (same flat-interpolation semantics as `proxy/src/domain/applier/component_render.rs`).
pub async fn resolve(pool: &PgPool, id: Uuid) -> AppResult<ResolvedSavedOutcomeRead> {
    let row = repo::get(pool, id).await?.ok_or_else(|| not_found(id))?;
    let selector = match row.version_number {
        Some(n) => VersionSelector::Version(n),
        None => VersionSelector::Default,
    };
    let resolved = component_template_service::resolve(pool, row.component_id, selector).await?;

    let mut builder = mustache::MapBuilder::new();
    if let Some(vars) = row.variables.as_object() {
        for (name, value) in vars {
            let s = value.as_str().unwrap_or_default().to_string();
            builder = builder.insert_str(name.clone(), s);
        }
    }
    let data = builder.build();
    let template = mustache::compile_str(&resolved.html_body)
        .map_err(|_| AppError::Internal(anyhow::anyhow!("saved outcome template failed to compile")))?;
    let html_body = template
        .render_data_to_string(&data)
        .map_err(|_| AppError::Internal(anyhow::anyhow!("saved outcome template failed to render")))?;

    Ok(ResolvedSavedOutcomeRead { html_body })
}

async fn to_read(pool: &PgPool, row: crate::models::saved_outcome::SavedOutcome) -> AppResult<SavedOutcomeRead> {
    let component = component_template_repository::get(pool, row.component_id)
        .await?
        .ok_or_else(|| AppError::ComponentNotFound(format!("Component '{}' not found", row.component_id)))?;
    Ok(saved_outcome::to_read(row, component.name))
}

fn not_found(id: Uuid) -> AppError {
    AppError::SavedOutcomeNotFound(format!("Saved outcome '{id}' not found"))
}

fn map_write_error(err: sqlx::Error) -> AppError {
    if let sqlx::Error::Database(db_err) = &err {
        match db_err.code().as_deref() {
            Some(PG_UNIQUE_VIOLATION) => {
                return AppError::SlugConflict(
                    "A saved outcome with this slug or name already exists".to_string(),
                );
            }
            Some(PG_FOREIGN_KEY_VIOLATION) => {
                return AppError::ComponentInUse(
                    "Referenced component is missing or in use and cannot be changed".to_string(),
                );
            }
            _ => {}
        }
    }
    err.into()
}
```
Note: adjust the exact import paths for `component_template_repository::get`, `VersionSelector`, and `component_template_service::resolve` to match what actually exists in `component_template_service.rs`/`component_template_repository.rs` (read those two files first — this step assumes their shape from CONTRACTS §5 but the exact function/type names must be confirmed against the real file). Add `mustache = "0.9"` (match proxy's pinned version) to `backend/Cargo.toml`.
Add `pub mod saved_outcome_service;` to `backend/src/services/mod.rs`.

- [ ] **Step 6: Add `SavedOutcomeNotFound` and `ComponentInUse` to `AppError`**

In `backend/src/error.rs`, add two variants and their `code()`/status arms:
```rust
/// Saved-outcome id missing. → 404
#[error("{0}")]
SavedOutcomeNotFound(String),
/// A component is referenced by a saved outcome (FK RESTRICT). → 409
#[error("{0}")]
ComponentInUse(String),
```
```rust
AppError::SavedOutcomeNotFound(_) => "SAVED_OUTCOME_NOT_FOUND",
AppError::ComponentInUse(_) => "COMPONENT_IN_USE",
```
Map both to their status codes in the same match the other variants use (404 and 409 respectively — mirror `OutcomeNotFound`/`LastVersionProtected`'s status arms).

- [ ] **Step 7: Routes**

`backend/src/api/v1/saved_outcomes.rs` — same shape as `products.rs` but keyed by `Uuid` path param and with the extra `resolve` route:
```rust
//! Outcomes Library HTTP handlers.

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    routing::post,
    Json, Router,
};
use serde::Deserialize;
use uuid::Uuid;

use crate::{
    error::AppResult,
    schemas::{
        pagination::{Page, PageParams},
        saved_outcome::{
            ResolvedSavedOutcomeRead, SavedOutcomeCreate, SavedOutcomeRead, SavedOutcomeUpdate,
        },
    },
    services::saved_outcome_service,
    state::AppState,
};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/saved-outcomes", post(create).get(list))
        .route(
            "/saved-outcomes/{id}",
            axum::routing::get(get).patch(update).delete(delete),
        )
        .route("/saved-outcomes/{id}/resolve", axum::routing::get(resolve))
}

#[derive(Debug, Default, Deserialize)]
pub struct SavedOutcomeListQuery {
    #[serde(default)]
    pub q: Option<String>,
}

pub async fn create(
    State(state): State<AppState>,
    Json(input): Json<SavedOutcomeCreate>,
) -> AppResult<impl IntoResponse> {
    let outcome = saved_outcome_service::create(&state.pool, input).await?;
    Ok((StatusCode::CREATED, Json(outcome)))
}

pub async fn list(
    State(state): State<AppState>,
    Query(params): Query<PageParams>,
    Query(filter): Query<SavedOutcomeListQuery>,
) -> AppResult<Json<Page<SavedOutcomeRead>>> {
    let page = saved_outcome_service::list(&state.pool, &params, filter.q.as_deref()).await?;
    Ok(Json(page))
}

pub async fn get(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> AppResult<Json<SavedOutcomeRead>> {
    let outcome = saved_outcome_service::get(&state.pool, id).await?;
    Ok(Json(outcome))
}

pub async fn update(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Json(input): Json<SavedOutcomeUpdate>,
) -> AppResult<Json<SavedOutcomeRead>> {
    let outcome = saved_outcome_service::update(&state.pool, id, input).await?;
    Ok(Json(outcome))
}

pub async fn delete(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> AppResult<impl IntoResponse> {
    saved_outcome_service::delete(&state.pool, id).await?;
    Ok(StatusCode::NO_CONTENT)
}

pub async fn resolve(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> AppResult<Json<ResolvedSavedOutcomeRead>> {
    let resolved = saved_outcome_service::resolve(&state.pool, id).await?;
    Ok(Json(resolved))
}
```
(utoipa `#[utoipa::path]` attributes omitted above for brevity — add them mirroring `products.rs`'s style before committing, including the 404 `COMPONENT_NOT_FOUND`/`SAVED_OUTCOME_NOT_FOUND` responses on `resolve`.)
Mount in `backend/src/api/v1/mod.rs` alongside `products::router()`.

- [ ] **Step 8: Add `saved_outcome_ref_exists` validation + the two manifest node types**

In `backend/config/node_types.json`, add:
```json
{
  "kind": "apply_saved_outcome",
  "label": "Apply Saved Outcome",
  "category": "content",
  "applies_to": "html",
  "node_kind": "expression",
  "summary": "Inject a saved Outcomes Library entry into the page",
  "fields": [
    { "name": "saved_outcome_id", "label": "Outcome", "control": "saved_outcome_select", "required": true },
    { "name": "target_selector", "label": "Target selector", "control": "text", "required": true },
    { "name": "placement_mode", "label": "Placement", "control": "select", "required": true,
      "options": [
        { "value": "replace", "label": "Replace" },
        { "value": "append", "label": "Append" },
        { "value": "prepend", "label": "Prepend" },
        { "value": "before", "label": "Before" },
        { "value": "after", "label": "After" }
      ] }
  ],
  "output": { "branches": [{ "id": "out", "label": "Next" }] }
},
{
  "kind": "apply_saved_outcome_json",
  "label": "Apply Saved Outcome (JSON)",
  "category": "json",
  "applies_to": "json",
  "node_kind": "expression",
  "summary": "Set a saved Outcomes Library entry's rendered HTML at a JSON path",
  "fields": [
    { "name": "saved_outcome_id", "label": "Outcome", "control": "saved_outcome_select", "required": true },
    { "name": "target_path", "label": "Target path", "control": "text", "required": true }
  ],
  "output": { "branches": [{ "id": "out", "label": "Next" }] }
}
```
Add `SavedOutcomeSelect` to `Control` in `node_type.rs` (serializes as `"saved_outcome_select"`, same pattern as Step 2 of the previous task).

In `rule_graph_service.rs`, add `valid_saved_outcome_ids: &HashSet<Uuid>` as a new parameter to `validate`/`validate_canvas` (same threading pattern as `valid_component_ids`). In the `Node::Expression` match arm, add:
```rust
validate_saved_outcome_ref(canvas, idx, action, valid_saved_outcome_ids, details);
```
And the function:
```rust
/// `saved_outcome_ref_exists`: an expression node whose `action.type` is
/// `apply_saved_outcome`/`apply_saved_outcome_json` has `action.saved_outcome_id`
/// present in `valid_saved_outcome_ids`. Mirrors `validate_apply_component_ref`.
fn validate_saved_outcome_ref(
    canvas: &'static str,
    idx: usize,
    action: &ProcessorConfig,
    valid_saved_outcome_ids: &HashSet<Uuid>,
    details: &mut Vec<ValidationDetail>,
) {
    if action.r#type != "apply_saved_outcome" && action.r#type != "apply_saved_outcome_json" {
        return;
    }
    let id = action
        .fields
        .get("saved_outcome_id")
        .and_then(|v| v.as_str())
        .and_then(|s| Uuid::parse_str(s).ok());
    match id {
        Some(id) if valid_saved_outcome_ids.contains(&id) => {}
        _ => {
            details.push(ValidationDetail::new(
                format!("rule_graph.{canvas}.nodes[{idx}].action.saved_outcome_id"),
                "referenced saved outcome does not exist".to_string(),
                "saved_outcome_ref_exists",
            ));
        }
    }
}
```
Thread the new `valid_saved_outcome_ids` set into the `version_service.rs` call site (Step 7 of Task 3) using `saved_outcome_repository::list_all_ids` converted to a `HashSet<Uuid>`.

- [ ] **Step 9: Tests**

Write inline tests in `rule_graph_service.rs` mirroring Task 3 Step 3/6 (`saved_outcome_ref_exists_rejects_unknown_id` / `_accepts_known_id`), and `backend/tests/saved_outcomes.rs` (mirror `backend/tests/products.rs` from Task 2, covering: create + get, dup slug/name → 409, delete-while-referenced-by-a-saved-outcome on the component side → 409 `COMPONENT_IN_USE` (create a component, create a saved outcome pointing at it, attempt `DELETE /component-templates/{id}` and assert 409), resolve with `version_number: None` → calls through to the component's default, resolve with a pinned version, resolve renders mustache against `variables` correctly (assert the returned `html_body` has the variable substituted), 404 on unknown id).

- [ ] **Step 10: Run full backend suite**

Run: `cd backend && PATH="$HOME/.cargo/bin:$PATH" cargo fmt && cargo clippy --all-targets -- -D warnings && DOCKER_HOST=unix:///var/run/docker.sock cargo test`
Expected: clean.

- [ ] **Step 11: Commit**

```bash
git add backend/migrations/0017_saved_outcomes.up.sql backend/migrations/0017_saved_outcomes.down.sql \
  backend/src/models/saved_outcome.rs backend/src/models/mod.rs \
  backend/src/schemas/saved_outcome.rs backend/src/schemas/mod.rs \
  backend/src/repositories/saved_outcome_repository.rs backend/src/repositories/mod.rs \
  backend/src/services/saved_outcome_service.rs backend/src/services/mod.rs \
  backend/src/api/v1/saved_outcomes.rs backend/src/api/v1/mod.rs backend/src/error.rs \
  backend/src/services/rule_graph_service.rs backend/src/services/version_service.rs \
  backend/config/node_types.json backend/src/schemas/node_type.rs backend/Cargo.toml \
  backend/tests/saved_outcomes.rs
git commit -m "feat(backend): add Outcomes Library (rre.saved_outcomes) + apply_saved_outcome nodes"
```

---

## Task 5: Proxy — collapse `RuleGraph` to a single Rule Canvas

**Files:**
- Modify: `proxy/src/domain/graph.rs`, `proxy/src/domain/mod.rs`, `proxy/src/domain/evaluator.rs`, `proxy/src/infra/compiled_cache.rs`, `proxy/src/infra/backend_client.rs`, `proxy/src/forwarder.rs`, `proxy/src/full_journey.rs`
- Delete: `proxy/src/domain/classifier.rs`
- Test: inline `#[cfg(test)]` across the above

**Interfaces:**
- Consumes: nothing new — pure removal.
- Produces: `RuleGraph { pub canvas: CanvasGraph }`, `RuleGraph::canvas() -> &CanvasGraph` (no parameter), `CompiledCache::get_or_compile(feature_id, version_number, build)` (drops the `canvas: Canvas` parameter), `GraphEvaluator::evaluate(canvas, ctx, feature_id, version_number)` (drops `canvas_class`).

- [ ] **Step 1: Collapse `graph.rs`**

Replace:
```rust
#[derive(Deserialize, Serialize, Clone, Debug, Default)]
pub struct RuleGraph {
    pub anonymous: CanvasGraph,
    pub registered: CanvasGraph,
    pub customer: CanvasGraph,
}
```
with:
```rust
#[derive(Deserialize, Serialize, Clone, Debug, Default)]
pub struct RuleGraph {
    pub canvas: CanvasGraph,
}
```
Delete the `Canvas` enum and the `impl RuleGraph { pub fn canvas(&self, c: Canvas) -> &CanvasGraph { ... } }` block entirely, replacing any remaining need for a "get the canvas" accessor with direct field access `rule_graph.canvas` at call sites (Step 5 below).

- [ ] **Step 2: Delete `classifier.rs` and its re-export**

Run: `rm proxy/src/domain/classifier.rs`
In `proxy/src/domain/mod.rs`, remove `pub mod classifier;` and `pub use graph::Canvas;`.

- [ ] **Step 3: Update `compiled_cache.rs`**

Replace:
```rust
use crate::domain::graph::Canvas;

pub struct CompiledCache {
    inner: Cache<(String, i32, Canvas), Arc<DecisionContent>>,
}
```
with:
```rust
pub struct CompiledCache {
    inner: Cache<(String, i32), Arc<DecisionContent>>,
}
```
Update `get_or_compile`'s signature to drop the `canvas: Canvas` parameter and the key tuple to `(feature_id.to_string(), version_number)`. Update the module doc comment ("Keyed by `(feature_id, version_number, Canvas)`" → "Keyed by `(feature_id, version_number)`").

- [ ] **Step 4: Update `evaluator.rs`**

In both `GraphEvaluator::evaluate` and `evaluate_with_trace`, remove the `canvas_class: Canvas` parameter and its use in `self.compiled.get_or_compile(feature_id, version_number, canvas_class, || ...)` → `self.compiled.get_or_compile(feature_id, version_number, || ...)`. Remove the `use crate::domain::graph::{Canvas, CanvasGraph, Node};` import's `Canvas` (keep `CanvasGraph, Node`).

- [ ] **Step 5: Update `backend_client.rs`**

Replace:
```rust
use crate::domain::graph::{Canvas, CanvasGraph, RuleGraph};
...
impl ActiveVersionRead {
    pub fn canvas(&self, c: Canvas) -> &CanvasGraph {
        self.rule_graph.canvas(c)
    }
```
with:
```rust
use crate::domain::graph::{CanvasGraph, RuleGraph};
...
impl ActiveVersionRead {
    pub fn canvas(&self) -> &CanvasGraph {
        &self.rule_graph.canvas
    }
```
(keep the rest of `ActiveVersionRead::find_outcome` etc. unchanged).

- [ ] **Step 6: Update `forwarder.rs`**

Remove `use crate::domain::classifier;` and the `Canvas` import from `use crate::domain::graph::{Canvas, CanvasGraph, Node};` (keep `CanvasGraph, Node`). Remove the classify call:
```rust
let canvas_class = classifier::classify(req.headers());
let canvas_label = canvas_name(canvas_class);
```
Delete `canvas_label` entirely (it only fed the per-request log line's `canvas` field — drop that field from the `tracing::info!("request")` calls too; grep for `canvas = %canvas_label` / `canvas_label` across the file). Delete the `canvas_name(c: Canvas) -> &'static str` helper function (the one mapping `Canvas::Anonymous => "anonymous"` etc.) entirely. Every function that took `canvas_class: Canvas` as a parameter (the `evaluate` helper, the two feature-loop functions, `resolve_action_components`'s siblings) drops that parameter; every `av.canvas(canvas_class)` call becomes `av.canvas()`; every `evaluator.evaluate(canvas_graph, ctx, feature_id, av.version_number, canvas_class)` call becomes `evaluator.evaluate(canvas_graph, ctx, feature_id, av.version_number)`.

- [ ] **Step 7: Update `full_journey.rs`**

Remove `classifier::classify(&fetched.request_headers)` and the `canvas_class` variable; change `resolved.rule_graph.canvas(canvas_class).clone()` to `resolved.rule_graph.canvas.clone()`.

- [ ] **Step 8: Compile and let the compiler enumerate anything missed**

Run: `PATH="$HOME/.cargo/bin:$PATH" cargo build --manifest-path proxy/Cargo.toml 2>&1 | grep -E "error|-->"`
Expected: initially some errors pointing at any remaining `Canvas`/`canvas_class`/`classifier` reference in `eval.rs` (which builds `CanvasGraph` directly from a test request body and never called `classify`, so should need no change) or elsewhere. Fix each reported site the same way (drop the parameter/field), then re-run until clean.

- [ ] **Step 9: Verify no `Canvas` references remain**

Run: `grep -rn "Canvas::" proxy/src; grep -rn "classifier" proxy/src; grep -rn "canvas_class" proxy/src`
Expected: no output from any of the three.

- [ ] **Step 10: Run the proxy test suite**

Run: `PATH="$HOME/.cargo/bin:$PATH" cargo test --manifest-path proxy/Cargo.toml`
Expected: all tests pass (fix any test fixture that still constructs a three-key `RuleGraph` literal or calls a function with the old `Canvas` parameter — grep test modules in `graph.rs`, `evaluator.rs`, `compiled_cache.rs`, `forwarder.rs` for `Canvas::` / `anonymous:`/`registered:`/`customer:` struct literals and collapse each to the new `{ canvas: ... }` shape).

- [ ] **Step 11: Commit**

```bash
git add proxy/src/domain/graph.rs proxy/src/domain/mod.rs proxy/src/domain/evaluator.rs \
  proxy/src/infra/compiled_cache.rs proxy/src/infra/backend_client.rs proxy/src/forwarder.rs \
  proxy/src/full_journey.rs
git rm proxy/src/domain/classifier.rs
git commit -m "feat(proxy): collapse RuleGraph to a single Rule Canvas"
```

---

## Task 6: Proxy — identity resolution + `logged_in`/`has_product` processors

**Files:**
- Create: `proxy/src/domain/identity.rs`, `proxy/src/domain/processors/logged_in.rs`, `proxy/src/domain/processors/has_product.rs`
- Modify: `proxy/src/config.rs`, `proxy/config/default.json`, `proxy/src/domain/context.rs`, `proxy/src/domain/mod.rs`, `proxy/src/domain/processors/mod.rs`, `proxy/src/forwarder.rs`, `proxy/src/eval.rs`, `proxy/src/full_journey.rs`
- Test: inline `#[cfg(test)]` in `identity.rs`, `logged_in.rs`, `has_product.rs`

**Interfaces:**
- Produces: `pub struct Identity { pub logged_in: bool, pub products: HashSet<String> }`, `identity::resolve(headers: &HeaderMap, cookies: &HashMap<String, String>, settings: &IdentitySettings) -> Identity`, `EvaluationContext.identity: Identity`, `LoggedInProcessor`, `HasProductProcessor`.
- Consumes: Task 1/5's collapsed proxy graph, existing `Settings` load pattern in `config.rs`.

- [ ] **Step 1: Add identity settings**

In `proxy/src/config.rs`, add four fields to `Settings`:
```rust
pub identity_user_cookie: String,
pub identity_products_cookie: String,
pub identity_user_header: String,
pub identity_products_header: String,
```
In `proxy/config/default.json`, add matching defaults:
```json
"identity_user_cookie": "rre_user",
"identity_products_cookie": "rre_products",
"identity_user_header": "x-rre-user",
"identity_products_header": "x-rre-products"
```

- [ ] **Step 2: Write the failing identity tests**

`proxy/src/domain/identity.rs`:
```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn settings() -> IdentitySettings {
        IdentitySettings {
            user_cookie: "rre_user".into(),
            products_cookie: "rre_products".into(),
            user_header: "x-rre-user".into(),
            products_header: "x-rre-products".into(),
        }
    }

    #[test]
    fn cookie_present_marks_logged_in_and_parses_products() {
        let headers = HeaderMap::new();
        let mut cookies = HashMap::new();
        cookies.insert("rre_user".to_string(), "u1".to_string());
        cookies.insert("rre_products".to_string(), "premium, Sports ,".to_string());
        let identity = resolve(&headers, &cookies, &settings());
        assert!(identity.logged_in);
        assert!(identity.products.contains("premium"));
        assert!(identity.products.contains("sports"));
        assert_eq!(identity.products.len(), 2);
    }

    #[test]
    fn header_fallback_when_cookie_absent() {
        let mut headers = HeaderMap::new();
        headers.insert("x-rre-user", "u1".parse().unwrap());
        headers.insert("x-rre-products", "gold".parse().unwrap());
        let identity = resolve(&headers, &HashMap::new(), &settings());
        assert!(identity.logged_in);
        assert!(identity.products.contains("gold"));
    }

    #[test]
    fn absent_everywhere_is_anonymous() {
        let identity = resolve(&HeaderMap::new(), &HashMap::new(), &settings());
        assert!(!identity.logged_in);
        assert!(identity.products.is_empty());
    }

    #[test]
    fn cookie_takes_priority_over_header() {
        let mut headers = HeaderMap::new();
        headers.insert("x-rre-products", "header-product".parse().unwrap());
        let mut cookies = HashMap::new();
        cookies.insert("rre_user".to_string(), "u1".to_string());
        cookies.insert("rre_products".to_string(), "cookie-product".to_string());
        let identity = resolve(&headers, &cookies, &settings());
        assert!(identity.products.contains("cookie-product"));
        assert!(!identity.products.contains("header-product"));
    }
}
```

- [ ] **Step 2b: Run to see it fail to compile**

Run: `PATH="$HOME/.cargo/bin:$PATH" cargo test --manifest-path proxy/Cargo.toml identity:: 2>&1 | tail -20`
Expected: compile errors — `resolve`, `IdentitySettings`, `Identity` don't exist yet.

- [ ] **Step 3: Implement `identity.rs`**

```rust
//! Visitor identity: whether the request is logged in, and which product
//! labels the visitor holds. Resolved once per request from a cookie first,
//! falling back to a header (Product Catalogue / Outcomes Library design).
//! Never logs raw cookie/header values.

use std::collections::{HashMap, HashSet};

use http::HeaderMap;

#[derive(Clone, Debug, Default)]
pub struct Identity {
    pub logged_in: bool,
    pub products: HashSet<String>,
}

#[derive(Clone, Debug)]
pub struct IdentitySettings {
    pub user_cookie: String,
    pub products_cookie: String,
    pub user_header: String,
    pub products_header: String,
}

/// Resolve identity: cookie first, header fallback. `logged_in` is true when
/// EITHER source has a non-empty user value; `products` is parsed from
/// WHICHEVER source produced a non-empty value, cookie checked first (never
/// merged across sources).
pub fn resolve(
    headers: &HeaderMap,
    cookies: &HashMap<String, String>,
    settings: &IdentitySettings,
) -> Identity {
    let user_from_cookie = cookies.get(&settings.user_cookie).filter(|v| !v.is_empty());
    let user_from_header = header_value(headers, &settings.user_header);
    let logged_in = user_from_cookie.is_some() || user_from_header.is_some();

    let products_source = cookies
        .get(&settings.products_cookie)
        .filter(|v| !v.is_empty())
        .cloned()
        .or_else(|| header_value(headers, &settings.products_header));

    let products = products_source
        .map(|raw| parse_products(&raw))
        .unwrap_or_default();

    Identity { logged_in, products }
}

fn header_value(headers: &HeaderMap, name: &str) -> Option<String> {
    headers
        .get(name)
        .and_then(|v| v.to_str().ok())
        .filter(|v| !v.is_empty())
        .map(str::to_string)
}

fn parse_products(raw: &str) -> HashSet<String> {
    raw.split(',')
        .map(|s| s.trim().to_ascii_lowercase())
        .filter(|s| !s.is_empty())
        .collect()
}
```
(place the `#[cfg(test)]` module from Step 2 at the bottom of this same file).

- [ ] **Step 4: Run tests to verify they pass**

Run: `PATH="$HOME/.cargo/bin:$PATH" cargo test --manifest-path proxy/Cargo.toml identity::`
Expected: all four pass.

- [ ] **Step 5: Add `identity` to `EvaluationContext`/`EvaluationContextParts`**

In `proxy/src/domain/context.rs`, add `pub identity: crate::domain::identity::Identity,` to both `EvaluationContext` and `EvaluationContextParts`. Update `EvaluationContextParts::from_request` to accept an additional `identity: Identity` parameter and store it; update `into_context` to move it across. Update `to_input_value` (the function projecting the context into the JDM input `Variable`) to add `"identity": { "logged_in": ..., "products": [...] }` to the projected JSON object (grep the file for how `site` is currently added to the input value and mirror that pattern) — this is what the `logged_in`/`has_product` processors will read via `ctx.identity` directly (processors read `&EvaluationContext`, not the projected JSON, so this JSON projection is only needed if OTHER parts of the graph, like a `json_expression` node, need to reference identity via JSONPath; if `to_input_value` only exists to build the zen input for expression/switch nodes and processors get `ctx` directly, confirm whether skipping the JSON projection is safe by checking how `ctx.site` is both used by `SiteMatchProcessor` (reads `ctx.site` directly) AND whether it's ALSO projected into `to_input_value`'s JSON — mirror `site`'s exact treatment, whatever that turns out to be).

- [ ] **Step 6: Wire `identity::resolve` into the forwarder**

In `forwarder.rs`, right after removing the `classifier::classify` call (Task 5 Step 6), add:
```rust
let identity_settings = crate::domain::identity::IdentitySettings {
    user_cookie: state.settings.identity_user_cookie.clone(),
    products_cookie: state.settings.identity_products_cookie.clone(),
    user_header: state.settings.identity_user_header.clone(),
    products_header: state.settings.identity_products_header.clone(),
};
let identity = crate::domain::identity::resolve(req.headers(), &cookies, &identity_settings);
```
(place it after `cookies` is computed, per the existing snapshot-before-consuming-request pattern already in the file). Pass `identity` into `EvaluationContextParts::from_request(...)` at its call site(s).

- [ ] **Step 7: Write the `LoggedInProcessor`**

`proxy/src/domain/processors/logged_in.rs`:
```rust
//! `LoggedInProcessor` (kind = "logged_in"). Branches on `ctx.identity.logged_in`.
//! No config fields.

use serde_json::Value;

use crate::domain::context::EvaluationContext;

use super::{Branch, CanvasProcessor, ProcessorError, ProcessorOutcome};

#[derive(Debug)]
pub struct LoggedInProcessor;

impl CanvasProcessor for LoggedInProcessor {
    fn kind(&self) -> &'static str {
        "logged_in"
    }

    fn evaluate(
        &self,
        _config: &Value,
        ctx: &EvaluationContext,
    ) -> Result<ProcessorOutcome, ProcessorError> {
        let branch = if ctx.identity.logged_in {
            Branch::Yes
        } else {
            Branch::No
        };
        Ok(ProcessorOutcome { branch })
    }
}

#[cfg(test)]
mod tests {
    use std::collections::{HashMap, HashSet};

    use http::HeaderMap;
    use serde_json::json;

    use crate::domain::context::{DeviceType, EvaluationContext};
    use crate::domain::identity::Identity;

    use super::*;

    fn ctx(logged_in: bool) -> EvaluationContext {
        EvaluationContext {
            request_headers: HeaderMap::new(),
            request_path: "/".to_string(),
            request_cookies: HashMap::new(),
            device: DeviceType::Desktop,
            meta_tags: HashMap::new(),
            response_json: None,
            site: None,
            identity: Identity { logged_in, products: HashSet::new() },
        }
    }

    #[test]
    fn branches_yes_when_logged_in() {
        let outcome = LoggedInProcessor.evaluate(&json!({}), &ctx(true)).unwrap();
        assert_eq!(outcome.branch, Branch::Yes);
    }

    #[test]
    fn branches_no_when_anonymous() {
        let outcome = LoggedInProcessor.evaluate(&json!({}), &ctx(false)).unwrap();
        assert_eq!(outcome.branch, Branch::No);
    }
}
```
(if `Branch` doesn't derive `PartialEq` yet, add it — check `proxy/src/domain/processors/mod.rs`'s `Branch` enum derive list first; it likely already does per `site_match.rs`'s test style using `assert_eq!`).

- [ ] **Step 8: Write the `HasProductProcessor`**

`proxy/src/domain/processors/has_product.rs`:
```rust
//! `HasProductProcessor` (kind = "has_product"). Branches `Yes` iff the
//! visitor's identity carries the configured `product` label. A stale/unknown
//! label (deleted product) fails open to `No` — mirrors `site_match`'s
//! fail-open behavior on a stale reference.

use serde_json::Value;

use crate::domain::context::EvaluationContext;

use super::{Branch, CanvasProcessor, ProcessorError, ProcessorOutcome};

#[derive(Debug)]
pub struct HasProductProcessor;

impl CanvasProcessor for HasProductProcessor {
    fn kind(&self) -> &'static str {
        "has_product"
    }

    fn evaluate(
        &self,
        config: &Value,
        ctx: &EvaluationContext,
    ) -> Result<ProcessorOutcome, ProcessorError> {
        let product = config
            .get("product")
            .and_then(Value::as_str)
            .ok_or_else(|| ProcessorError::Config("missing product".to_string()))?;

        let branch = if ctx.identity.products.contains(product) {
            Branch::Yes
        } else {
            Branch::No
        };

        Ok(ProcessorOutcome { branch })
    }
}

#[cfg(test)]
mod tests {
    use std::collections::{HashMap, HashSet};

    use http::HeaderMap;
    use serde_json::json;

    use crate::domain::context::{DeviceType, EvaluationContext};
    use crate::domain::identity::Identity;

    use super::*;

    fn ctx(products: &[&str]) -> EvaluationContext {
        EvaluationContext {
            request_headers: HeaderMap::new(),
            request_path: "/".to_string(),
            request_cookies: HashMap::new(),
            device: DeviceType::Desktop,
            meta_tags: HashMap::new(),
            response_json: None,
            site: None,
            identity: Identity {
                logged_in: true,
                products: products.iter().map(|s| s.to_string()).collect(),
            },
        }
    }

    #[test]
    fn branches_yes_when_product_held() {
        let outcome = HasProductProcessor
            .evaluate(&json!({ "product": "premium" }), &ctx(&["premium"]))
            .unwrap();
        assert_eq!(outcome.branch, Branch::Yes);
    }

    #[test]
    fn branches_no_when_product_absent_or_stale() {
        let outcome = HasProductProcessor
            .evaluate(&json!({ "product": "gold" }), &ctx(&["premium"]))
            .unwrap();
        assert_eq!(outcome.branch, Branch::No);
    }

    #[test]
    fn errors_when_config_missing_product() {
        assert!(HasProductProcessor.evaluate(&json!({}), &ctx(&[])).is_err());
    }
}
```

- [ ] **Step 9: Register both processors**

In `proxy/src/domain/processors/mod.rs`, add:
```rust
pub mod has_product;
pub mod logged_in;
```
and in `default_registry()`:
```rust
registry.register(Arc::new(logged_in::LoggedInProcessor));
registry.register(Arc::new(has_product::HasProductProcessor));
```

- [ ] **Step 10: Extend the eval-test-panel endpoints**

In `proxy/src/eval.rs`, find `EvalContext` (the request body struct for `/__rre/eval` and `/__rre/eval-url`) and add:
```rust
#[serde(default)]
pub logged_in: bool,
#[serde(default)]
pub products: Vec<String>,
```
Find wherever `EvaluationContext`/`EvaluationContextParts` is constructed from an `EvalContext` in this file and set `identity: Identity { logged_in: body.logged_in, products: body.products.iter().map(|s| s.to_lowercase()).collect() }` directly (bypassing `identity::resolve`'s cookie/header parsing, since the test panel supplies the state directly).

- [ ] **Step 11: Extend `full_journey.rs`'s identity resolution**

In `full_journey.rs`, the request no longer classifies via cookie (Task 5 removed that call) but needs identity for the new decision nodes. Since `/__rre/eval-full-journey`'s request body doesn't currently carry `logged_in`/`products` per CONTRACTS, add the same `identity::resolve(&fetched.request_headers, &fetched.request_cookies, &identity_settings)` call the forwarder uses (full_journey fetches a REAL URL, so cookie/header resolution from the live fetch response is appropriate, mirroring how the forwarder does it) rather than accepting them as request-body overrides — this keeps `full_journey` testing real identity from whatever cookies/headers the test URL's live request naturally carries. Confirm this matches how `device_type`/`site` are already resolved in this file (grep for how `DeviceType` is computed here) and follow the same pattern.

- [ ] **Step 12: Build and run full proxy suite**

Run: `PATH="$HOME/.cargo/bin:$PATH" cargo build --manifest-path proxy/Cargo.toml && cargo test --manifest-path proxy/Cargo.toml`
Expected: clean build, all tests pass.

- [ ] **Step 13: Commit**

```bash
git add proxy/src/domain/identity.rs proxy/src/domain/processors/logged_in.rs \
  proxy/src/domain/processors/has_product.rs proxy/src/domain/processors/mod.rs \
  proxy/src/config.rs proxy/config/default.json proxy/src/domain/context.rs proxy/src/domain/mod.rs \
  proxy/src/forwarder.rs proxy/src/eval.rs proxy/src/full_journey.rs
git commit -m "feat(proxy): add visitor identity resolution + logged_in/has_product processors"
```

---

## Task 7: Proxy — Outcomes Library cache + `apply_saved_outcome`/`apply_saved_outcome_json` appliers

**Files:**
- Create: `proxy/src/infra/saved_outcome_cache.rs`
- Modify: `proxy/src/state.rs`, `proxy/src/main.rs`, `proxy/src/domain/applier/json_apply.rs`, `proxy/src/forwarder.rs`, `proxy/src/eval.rs`, `proxy/src/full_journey.rs`
- Test: inline `#[cfg(test)]` in `saved_outcome_cache.rs`, `json_apply.rs`

**Interfaces:**
- Consumes: Task 4's `GET /api/v1/saved-outcomes/{id}/resolve` endpoint, `ApplyError`, `html_injection::inject_html`, `html_sanitizer::sanitize`, `json_apply::add_attribute`.
- Produces: `SavedOutcomeCache::resolve_saved_outcome(id) -> Option<Arc<ResolvedSavedOutcome>>`, `ResolvedSavedOutcomeMap = HashMap<Uuid, Arc<ResolvedSavedOutcome>>`, `json_apply::saved_outcome_ref(action) -> Option<Uuid>`, new match arms in `apply_action_html`/`apply_action_json`.

- [ ] **Step 1: Write the cache, mirroring `component_cache.rs` almost verbatim**

`proxy/src/infra/saved_outcome_cache.rs` — copy `component_cache.rs`'s structure exactly (SWR, `Stamped`, `FetchResult`, `spawn_refresh`, `fetch_inner`), with these changes: key type is `Uuid` (not `(Uuid, VersionSelector)`), value type is `Arc<ResolvedSavedOutcome>`, and the fetch URL is `GET {base}/api/v1/saved-outcomes/{id}/resolve`:
```rust
//! Saved-outcome resolve cache (Outcomes Library design). Mirrors
//! `component_cache.rs`'s stale-while-revalidate shape exactly, keyed by the
//! saved outcome's own id (no version selector — the backend `resolve` route
//! already picked the version and rendered the HTML).

use std::sync::Arc;
use std::time::{Duration, Instant};

use moka::future::Cache;
use serde::Deserialize;
use uuid::Uuid;

#[derive(Deserialize, Clone, Debug)]
pub struct ResolvedSavedOutcome {
    pub html_body: String,
}

#[derive(Clone)]
struct Stamped {
    value: Arc<ResolvedSavedOutcome>,
    fetched_at: Instant,
}

enum FetchResult {
    Ok(Arc<ResolvedSavedOutcome>),
    NotFound,
    Error,
}

pub struct SavedOutcomeCache {
    http: reqwest::Client,
    base: String,
    cache: Cache<Uuid, Stamped>,
    ttl: Duration,
    refresh_inflight: Cache<Uuid, ()>,
}

impl SavedOutcomeCache {
    pub fn new(http: reqwest::Client, base: String, ttl_secs: u64) -> Self {
        let ttl = Duration::from_secs(ttl_secs);
        const SAFETY_FLOOR: Duration = Duration::from_secs(300);
        let safety_cap = ttl.saturating_mul(20).max(SAFETY_FLOOR);
        let cache = Cache::builder().time_to_live(safety_cap).max_capacity(256).build();
        let inflight_ttl = ttl.max(Duration::from_secs(10));
        let refresh_inflight = Cache::builder().time_to_live(inflight_ttl).max_capacity(256).build();
        Self { http, base, cache, ttl, refresh_inflight }
    }

    pub async fn resolve_saved_outcome(&self, id: Uuid) -> Option<Arc<ResolvedSavedOutcome>> {
        if let Some(stamped) = self.cache.get(&id).await {
            if stamped.fetched_at.elapsed() < self.ttl {
                return Some(stamped.value);
            }
            self.spawn_refresh(id);
            return Some(stamped.value);
        }
        match fetch_inner(&self.http, &self.base, id).await {
            FetchResult::Ok(rso) => {
                self.cache.insert(id, Stamped { value: rso.clone(), fetched_at: Instant::now() }).await;
                Some(rso)
            }
            FetchResult::NotFound | FetchResult::Error => None,
        }
    }

    fn spawn_refresh(&self, id: Uuid) {
        let inflight = self.refresh_inflight.clone();
        let cache = self.cache.clone();
        let http = self.http.clone();
        let base = self.base.clone();
        tokio::spawn(async move {
            if inflight.get(&id).await.is_some() {
                return;
            }
            inflight.insert(id, ()).await;
            match fetch_inner(&http, &base, id).await {
                FetchResult::Ok(rso) => {
                    cache.insert(id, Stamped { value: rso, fetched_at: Instant::now() }).await;
                }
                FetchResult::NotFound => {
                    cache.invalidate(&id).await;
                }
                FetchResult::Error => {}
            }
            inflight.invalidate(&id).await;
        });
    }
}

async fn fetch_inner(http: &reqwest::Client, base: &str, id: Uuid) -> FetchResult {
    let url = format!("{}/api/v1/saved-outcomes/{}/resolve", base.trim_end_matches('/'), id);
    let resp = match http.get(&url).send().await {
        Ok(r) => r,
        Err(e) => {
            tracing::warn!(error = %e, saved_outcome_id = %id, "resolve_saved_outcome=miss (transport error)");
            return FetchResult::Error;
        }
    };
    let status = resp.status();
    if !status.is_success() {
        tracing::warn!(status = %status, saved_outcome_id = %id, "resolve_saved_outcome=miss (non-2xx)");
        return if status == reqwest::StatusCode::NOT_FOUND {
            FetchResult::NotFound
        } else {
            FetchResult::Error
        };
    }
    match resp.json::<ResolvedSavedOutcome>().await {
        Ok(rso) => FetchResult::Ok(Arc::new(rso)),
        Err(e) => {
            tracing::warn!(error = %e, saved_outcome_id = %id, "resolve_saved_outcome=miss (decode error)");
            FetchResult::Error
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[tokio::test]
    async fn resolve_within_ttl_hits_backend_once() {
        let server = MockServer::start().await;
        let id = Uuid::new_v4();
        Mock::given(method("GET"))
            .and(path(format!("/api/v1/saved-outcomes/{id}/resolve")))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({ "html_body": "<p>hi</p>" })))
            .expect(1)
            .mount(&server)
            .await;
        let cache = SavedOutcomeCache::new(reqwest::Client::new(), server.uri(), 3600);
        for _ in 0..5 {
            let rso = cache.resolve_saved_outcome(id).await.unwrap();
            assert_eq!(rso.html_body, "<p>hi</p>");
        }
    }

    #[tokio::test]
    async fn unresolvable_returns_none() {
        let server = MockServer::start().await;
        let id = Uuid::new_v4();
        Mock::given(method("GET"))
            .and(path(format!("/api/v1/saved-outcomes/{id}/resolve")))
            .respond_with(ResponseTemplate::new(404))
            .mount(&server)
            .await;
        let cache = SavedOutcomeCache::new(reqwest::Client::new(), server.uri(), 3600);
        assert!(cache.resolve_saved_outcome(id).await.is_none());
    }
}
```
Add `pub mod saved_outcome_cache;` to `proxy/src/infra/mod.rs` (or wherever `pub mod component_cache;` is declared).

- [ ] **Step 2: Run the new cache tests**

Run: `PATH="$HOME/.cargo/bin:$PATH" cargo test --manifest-path proxy/Cargo.toml saved_outcome_cache::`
Expected: both tests pass.

- [ ] **Step 3: Add `saved_outcome_cache` to `AppState`**

In `proxy/src/state.rs`, add:
```rust
pub saved_outcome_cache: Arc<crate::infra::saved_outcome_cache::SavedOutcomeCache>,
```
In `proxy/src/main.rs`, construct it right after `component_cache` and add it to the `AppState { ... }` literal:
```rust
let saved_outcome_cache = rre_proxy::infra::saved_outcome_cache::SavedOutcomeCache::new(
    http.clone(),
    settings.backend_base_url.clone(),
    settings.active_version_ttl_secs,
);
```

- [ ] **Step 4: Add `saved_outcome_ref` extraction + the two apply functions to `json_apply.rs`**

Add near `component_ref`:
```rust
/// If `action` is an `apply_saved_outcome` / `apply_saved_outcome_json` action,
/// return its `saved_outcome_id` for PRE-RESOLUTION on the async side. `None`
/// for any other action type or a malformed/absent id.
pub fn saved_outcome_ref(action: &Value) -> Option<Uuid> {
    let kind = action.get("type").and_then(Value::as_str)?;
    if kind != "apply_saved_outcome" && kind != "apply_saved_outcome_json" {
        return None;
    }
    let id_str = action.get("saved_outcome_id").and_then(Value::as_str)?;
    Uuid::parse_str(id_str).ok()
}

/// Pre-resolved saved-outcome map for one feature's apply pass, keyed by
/// `saved_outcome_id`. Parallel to `ResolvedComponentMap` but with no version
/// selector (the backend `resolve` route already picked the version).
pub type ResolvedSavedOutcomeMap =
    HashMap<Uuid, Arc<crate::infra::saved_outcome_cache::ResolvedSavedOutcome>>;

/// Stable idempotency marker for an injected saved outcome, keyed by its id
/// alone (the backend resolve already picked/rendered the version).
fn saved_outcome_marker(id: Uuid) -> String {
    format!("rso-{id}")
}
```
Add a parameter `saved_outcomes: &ResolvedSavedOutcomeMap` to BOTH `apply_action_html` and `apply_action_json`'s signatures (right after `components: &ResolvedComponentMap`). Add match arms to each:

In `apply_action_html`, add `"apply_saved_outcome" => apply_saved_outcome_html(body, action, saved_outcomes, sanitizer),` and extend the `"apply_component_json"` (json-only, warn+skip) arm's sibling list to also warn-skip `"apply_saved_outcome_json"` when it appears on an HTML body:
```rust
"apply_saved_outcome_json" => {
    tracing::warn!("apply_saved_outcome_json action on HTML body, skipped");
    (body, false)
}
```
with the function:
```rust
/// `apply_saved_outcome` (HTML): look up the PRE-RESOLVED, ALREADY-RENDERED
/// saved outcome, sanitize its `html_body`, then inject at `target_selector`
/// per `placement_mode` (reuses the `html_injection` core exactly like
/// `apply_component`). No mustache render here — the backend already rendered
/// it against the saved outcome's own `variables`.
fn apply_saved_outcome_html(
    body: String,
    action: &Value,
    saved_outcomes: &ResolvedSavedOutcomeMap,
    sanitizer: &ammonia::Builder<'static>,
) -> (String, bool) {
    let Some(id) = saved_outcome_ref(action) else {
        tracing::warn!("apply_saved_outcome action: bad/absent saved_outcome_id, skipped");
        return (body, false);
    };
    let Some(resolved) = saved_outcomes.get(&id) else {
        tracing::warn!("apply_saved_outcome action: saved outcome not resolved, skipped");
        return (body, false);
    };
    let sanitized = html_sanitizer::sanitize(sanitizer, &resolved.html_body);
    let target_selector = action.get("target_selector").and_then(Value::as_str).unwrap_or("");
    let placement_mode = action.get("placement_mode").and_then(Value::as_str).unwrap_or("append");
    let marker = saved_outcome_marker(id);
    match html_injection::inject_html(&body, target_selector, placement_mode, &sanitized, &marker) {
        Ok(next) => {
            let changed = next != body;
            (next, changed)
        }
        Err(e) => {
            tracing::warn!(error = %e, "apply_saved_outcome inject failed, serving original");
            (body, false)
        }
    }
}
```

In `apply_action_json`, add `"apply_saved_outcome_json" => apply_saved_outcome_json(body, action, saved_outcomes, sanitizer),` and a warn-skip arm for `"apply_saved_outcome"` on a JSON body, with the function:
```rust
/// `apply_saved_outcome_json`: sanitize the PRE-RESOLVED, ALREADY-RENDERED
/// saved outcome's `html_body` and set it as a STRING at `target_path` via the
/// `json_set` core (`add_attribute`). No mustache render here.
fn apply_saved_outcome_json(
    body: &mut Value,
    action: &Value,
    saved_outcomes: &ResolvedSavedOutcomeMap,
    sanitizer: &ammonia::Builder<'static>,
) -> bool {
    let Some(id) = saved_outcome_ref(action) else {
        tracing::warn!("apply_saved_outcome_json action: bad/absent saved_outcome_id, skipped");
        return false;
    };
    let Some(resolved) = saved_outcomes.get(&id) else {
        tracing::warn!("apply_saved_outcome_json action: saved outcome not resolved, skipped");
        return false;
    };
    let Some(target) = action.get("target_path").and_then(Value::as_str) else {
        tracing::warn!("apply_saved_outcome_json action missing `target_path`, skipped");
        return false;
    };
    let sanitized = html_sanitizer::sanitize(sanitizer, &resolved.html_body);
    match add_attribute(body, target, Value::String(sanitized)) {
        Ok(changed) => changed,
        Err(e) => {
            tracing::warn!(error = %e, "apply_saved_outcome_json set-at-path failed, skipped");
            false
        }
    }
}
```

- [ ] **Step 5: Thread the new parameter through every `apply_action_html`/`apply_action_json` call site**

There are exactly four call sites (confirmed by prior grep): `forwarder.rs:304` and `:468`, `eval.rs:583` and `:591`. At each, build a `ResolvedSavedOutcomeMap` the same way `resolve_action_components` builds a `ResolvedComponentMap` (Step 6 below) and pass it as the new argument.

- [ ] **Step 6: Extend the pre-resolve pass**

In `forwarder.rs`, add a sibling function next to `resolve_action_components`:
```rust
/// Pre-resolve every saved-outcome reference touched by `actions`, ON THE
/// ASYNC SIDE, mirroring `resolve_action_components`. Only direct action refs
/// (`apply_saved_outcome`/`apply_saved_outcome_json`) exist for saved
/// outcomes — no outcome-embedded variant.
pub(crate) async fn resolve_action_saved_outcomes(
    state: &AppState,
    actions: &[MatchedAction],
) -> json_apply::ResolvedSavedOutcomeMap {
    let mut map = json_apply::ResolvedSavedOutcomeMap::new();
    let mut ids: Vec<uuid::Uuid> = Vec::new();
    for ma in actions {
        if let Some(id) = json_apply::saved_outcome_ref(&ma.action) {
            if !ids.contains(&id) {
                ids.push(id);
            }
        }
    }
    for id in ids {
        if let Some(resolved) = state.saved_outcome_cache.resolve_saved_outcome(id).await {
            map.insert(id, resolved);
        }
    }
    map
}
```
Call it alongside every existing `resolve_action_components(...)` call site (in `forwarder.rs` at both feature-loop functions, and via `full_journey.rs`/`eval.rs` wherever they call `forwarder::resolve_action_components`) and pass the result into the corresponding `apply_action_html`/`apply_action_json` call from Step 5. In `full_journey.rs` and `eval.rs`, add a matching `forwarder::resolve_action_saved_outcomes(state, &trace.actions).await` call next to their existing `forwarder::resolve_action_components(...)` call.

- [ ] **Step 7: Build and run full proxy suite**

Run: `PATH="$HOME/.cargo/bin:$PATH" cargo build --manifest-path proxy/Cargo.toml && cargo test --manifest-path proxy/Cargo.toml`
Expected: clean build (fixes any missed call site the compiler flags for the new function parameters), all tests pass.

- [ ] **Step 8: Add applier integration tests**

In `json_apply.rs`'s test module, add tests mirroring the existing `apply_component`/`apply_component_json` tests: `apply_saved_outcome_html` injects sanitized html at the target selector when resolved; `apply_saved_outcome_json` sets the sanitized html as a string at the target path; both fail open (return the original body unchanged) when the id isn't in the resolved map.

- [ ] **Step 9: Commit**

```bash
git add proxy/src/infra/saved_outcome_cache.rs proxy/src/infra/mod.rs proxy/src/state.rs proxy/src/main.rs \
  proxy/src/domain/applier/json_apply.rs proxy/src/forwarder.rs proxy/src/eval.rs proxy/src/full_journey.rs
git commit -m "feat(proxy): add Outcomes Library cache + apply_saved_outcome(_json) appliers"
```

---

## Task 8: Frontend — collapse the Rule Canvas UI to a single canvas

**Files:**
- Modify: `frontend/src/lib/api/ruleGraph.ts`, `frontend/src/lib/canvas/types.ts`, `frontend/src/lib/canvas/serialize.ts`, `frontend/src/lib/canvas/deserialize.ts`, `frontend/src/lib/canvas/diff.ts`, `frontend/src/lib/canvas/graphValidation.ts`, `frontend/src/lib/canvas/validationMapping.ts`, `frontend/src/state/ruleBuilderStore.ts`, `frontend/src/components/version/RuleBuilderClient.tsx`, `frontend/src/components/canvas/compare/CompareDialog.tsx`, `frontend/src/components/canvas/compare/ChangeList.tsx`
- Delete: `frontend/src/components/canvas/CanvasSlider.tsx` (and its test file, if present)
- Test: update every existing Vitest file that references `CanvasKey`, `CanvasSlider`, or the three canvas-name literals

**Interfaces:**
- Consumes: Task 1's collapsed backend `RuleGraph { canvas }` wire shape.
- Produces: `RuleGraph` zod schema `{ canvas: CanvasGraph }`, `ruleBuilderStore`'s `canvas: CanvasWorkingState` (replacing `canvases: Record<CanvasKey, ...>` + `selected`).

- [ ] **Step 1: Collapse the zod schema**

In `frontend/src/lib/api/ruleGraph.ts`, replace:
```ts
export const RuleGraph = z.object({
  anonymous: CanvasGraph,
  registered: CanvasGraph,
  customer: CanvasGraph,
});
```
with:
```ts
export const RuleGraph = z.object({
  canvas: CanvasGraph,
});
```

- [ ] **Step 2: Remove `CanvasKey` from `types.ts`**

In `frontend/src/lib/canvas/types.ts`, delete the `CanvasKey` type export entirely (grep first to confirm its exact definition and every other export in the file that depends on it before deleting).

- [ ] **Step 3: Collapse `serialize.ts`/`deserialize.ts`**

In `serialize.ts`, replace `serializeRuleGraph`:
```ts
export function serializeRuleGraph(canvas: CanvasWorkingState): RuleGraph {
  return { canvas: serializeCanvas(canvas.nodes, canvas.edges, canvas.rootNodeId) };
}
```
(drop the `Record<CanvasKey, CanvasWorkingState>` parameter type and the three named calls). `serializeCanvas` itself is unchanged.

In `deserialize.ts`, replace `deserializeRuleGraph`:
```ts
export function deserializeRuleGraph(
  rg: RuleGraph,
  outcomeTitleById: (id: string) => string,
  componentNameById: (id: string) => string | undefined = () => undefined,
): CanvasWorkingState {
  return deserializeCanvas(rg.canvas, outcomeTitleById, componentNameById);
}
```
`deserializeCanvas` itself is unchanged.

- [ ] **Step 4: Collapse `ruleBuilderStore.ts`**

Replace `canvases: Record<CanvasKey, CanvasWorkingState>` + `selected: CanvasKey` with a single `canvas: CanvasWorkingState`. Remove `CANVAS_KEYS`, `emptyCanvases()` (keep `emptyCanvas()`), `setSelected`, and the `selected` field from `RuleBuilderState`. Every reducer that took a `k: CanvasKey` first parameter (`addNode`, `removeNode`, `updateNodePosition`, `setNodePositions`, `addEdge`, `removeEdge`, `onNodesChange`, `onEdgesChange`, `updateNodeProcessor`, `updateNodeAction`) drops that parameter; every internal `state.canvases[k]` becomes `state.canvas`, and every `canvases: { ...state.canvases, [k]: {...} }` becomes `canvas: {...}` directly (no wrapping record). `seedFromRuleGraph`'s signature drops the multi-canvas fan-out:
```ts
seedFromRuleGraph: (rg, status, outcomeTitleById, componentNameById) => {
  const deserialized = deserializeRuleGraph(rg, outcomeTitleById, componentNameById);
  const canvas = withStartEnd(deserialized);
  set({
    canvas,
    journeyPath: computeJourneyPath(canvas),
    versionStatus: status,
    isEditing: false,
    dirty: false,
    nodeErrors: {},
    configNodeId: null,
    testHighlight: null,
    baselineHash: hashRuleGraph(serializeRuleGraph(canvas)),
    lastSavedAt: null,
  });
},
```
`applySeedLayout` drops its `Record<CanvasKey, Map<...>>` parameter down to a single `Map<string, {x,y}>` and applies it directly to `state.canvas`. `isDirty`/`selectCurrentRuleGraph`/`selectJourneyPath` read `state.canvas` instead of `state.canvases[state.selected]`. Remove the `export { CANVAS_KEYS };` line at the bottom.

Apply this mechanically to every remaining reducer in the file (the pattern is identical each time: drop the `k` param, replace `state.canvases[k]` with `state.canvas`, replace `canvases: { ...state.canvases, [k]: next }` with `canvas: next`).

- [ ] **Step 5: Delete `CanvasSlider.tsx` and update `RuleBuilderClient.tsx`**

Run: `rm frontend/src/components/canvas/CanvasSlider.tsx`
Run: `find frontend/src -iname "*CanvasSlider*"` to find and delete its test file too, if one exists.
In `RuleBuilderClient.tsx`, remove the `<CanvasSlider selected={selected} onSelect={setSelected}/>` render and the `selected`/`setSelected` store selectors; change `<RuleBuilderCanvas canvasKey={selected} editable={isEditing}/>` to drop the `canvasKey` prop (and remove that prop from `RuleBuilderCanvas`'s own props type + every internal `state.canvases[canvasKey]` read inside it, applying the same collapse pattern as Step 4).

- [ ] **Step 6: Collapse `diff.ts`/`graphValidation.ts`/`validationMapping.ts`/compare components**

In each of `diff.ts`, `graphValidation.ts`, `validationMapping.ts`: find every `CanvasKey`-indexed loop or three-way comparison (the explorer research identified `diff.ts:179-181`, `graphValidation.ts:14-16`, `validationMapping.ts:14-19,159`) and collapse each to operate on the single canvas directly — where the code previously looped `for (const key of CANVAS_KEYS)` and built a `Record<CanvasKey, X>` result, it now runs once against `rg.canvas` and returns a single `X` (not a record).

In `CompareDialog.tsx`/`ChangeList.tsx` (used by the version-diff feature comparing two versions' rule_graphs), remove the three-way per-canvas comparison UI (tabs/sections keyed by canvas) down to a single comparison view of `versionA.canvas` vs `versionB.canvas`.

- [ ] **Step 7: Typecheck to enumerate remaining call sites**

Run: `cd frontend && npm run typecheck 2>&1 | head -100`
Expected: a list of every remaining reference to `CanvasKey`, `.anonymous`/`.registered`/`.customer`, or a now-invalid `canvases`/`selected` prop/field. Fix each reported site using the same collapse pattern, then re-run until clean.

- [ ] **Step 8: Update existing tests**

Run: `grep -rl "CanvasKey\|CanvasSlider\|anonymous\|registered.*customer" frontend/src --include="*.test.ts*"`
For each returned file, update the test fixtures/assertions to the single-canvas shape (a `RuleGraph` fixture becomes `{ canvas: {...} }` instead of `{ anonymous: {...}, registered: {...}, customer: {...} }`; drop any assertion about the CanvasSlider or canvas-switching behavior).

- [ ] **Step 9: Run the frontend test suite**

Run: `cd frontend && npm run test`
Expected: all tests pass.

- [ ] **Step 10: Commit**

```bash
git add frontend/src/lib/api/ruleGraph.ts frontend/src/lib/canvas/types.ts frontend/src/lib/canvas/serialize.ts \
  frontend/src/lib/canvas/deserialize.ts frontend/src/lib/canvas/diff.ts frontend/src/lib/canvas/graphValidation.ts \
  frontend/src/lib/canvas/validationMapping.ts frontend/src/state/ruleBuilderStore.ts \
  frontend/src/components/version/RuleBuilderClient.tsx frontend/src/components/canvas/compare/ \
  frontend/src/components/canvas/RuleBuilderCanvas.tsx
git rm frontend/src/components/canvas/CanvasSlider.tsx
git commit -m "feat(frontend): collapse Rule Canvas UI to a single canvas"
```

---

## Task 9: Frontend — Product Catalogue UI

**Files:**
- Create: `frontend/src/lib/api/products.ts`, `frontend/src/components/canvas/config/ProductSelectControl.tsx`, `frontend/src/components/product-catalogue/ProductCatalogueClient.tsx`, `frontend/src/components/product-catalogue/ProductCreateModal.tsx`, `frontend/src/app/products/catalogue/page.tsx`
- Modify: `frontend/src/lib/api/nodeTypes.ts`, `frontend/src/components/canvas/config/GenericNodeForm.tsx`, `frontend/src/components/nav/TopNav.tsx`, `frontend/src/lib/uiCopy.ts`
- Test: `frontend/src/lib/api/products.test.ts` (if the project keeps API-client tests; else skip straight to component tests), `ProductCatalogueClient.test.tsx`, `ProductSelectControl.test.tsx`

**Interfaces:**
- Consumes: Task 2's `/api/v1/products` backend routes, `apiGet`/`apiSend`/`Page` from `@/lib/api/client`, `useDebouncedValue`.
- Produces: `ProductRead`, `ProductCreate`, `ProductUpdate` zod types + `listProducts`/`getProduct`/`createProduct`/`updateProduct`/`deleteProduct`/`searchProducts`, `<ProductSelectControl>`.

- [ ] **Step 1: API client + zod schema**

`frontend/src/lib/api/products.ts` — mirror `frontend/src/lib/api/sites.ts` structure, dropping every header-specific piece:
```ts
import { z } from "zod";

import { apiGet, apiSend, Page } from "@/lib/api/client";

// Product Catalogue API client. Mirrors the backend ProductCreate / ProductUpdate
// / ProductRead DTOs. All keys are snake_case on the wire.

export const PRODUCT_LABEL_RE = /^[a-z0-9]+(_[a-z0-9]+)*$/;

export const ProductRead = z.object({
  label: z.string(),
  name: z.string(),
  description: z.string().nullable(),
  created_at: z.string(),
  updated_at: z.string(),
});
export type ProductRead = z.infer<typeof ProductRead>;

export const ProductCreate = z.object({
  label: z
    .string()
    .min(1, "Label is required")
    .max(64, "Label must be at most 64 characters")
    .regex(PRODUCT_LABEL_RE, "Use lowercase snake_case (e.g. premium_tier)"),
  name: z.string().min(1, "Name is required").max(200),
  description: z.string().max(500).optional(),
});
export type ProductCreate = z.infer<typeof ProductCreate>;

export const ProductUpdate = z.object({
  name: z.string().min(1).max(200).optional(),
  description: z.string().max(500).optional(),
});
export type ProductUpdate = z.infer<typeof ProductUpdate>;

export interface ListProductsParams {
  page: number;
  page_size: number;
  q?: string;
}

export const listProducts = (p: ListProductsParams = { page: 1, page_size: 20 }) => {
  const params = new URLSearchParams({ page: String(p.page), page_size: String(p.page_size) });
  if (p.q && p.q.trim() !== "") params.set("q", p.q.trim());
  return apiGet(`/api/v1/products?${params.toString()}`, Page(ProductRead));
};

export const getProduct = (label: string) => apiGet(`/api/v1/products/${label}`, ProductRead);

export const createProduct = (b: ProductCreate) =>
  apiSend("POST", `/api/v1/products`, ProductRead, b);

export const updateProduct = (label: string, b: ProductUpdate) =>
  apiSend("PATCH", `/api/v1/products/${label}`, ProductRead, b);

export const deleteProduct = (label: string) =>
  apiSend("DELETE", `/api/v1/products/${label}`, z.void());

const SEARCH_PAGE_SIZE = 10;
export const searchProducts = (q: string) => listProducts({ page: 1, page_size: SEARCH_PAGE_SIZE, q });
```

- [ ] **Step 2: `ProductSelectControl.tsx`**

Copy `SiteSelectControl.tsx` verbatim and adapt: rename to `ProductSelectControl`, swap `getSite`/`searchSites`/`SiteRead` for `getProduct`/`searchProducts`/`ProductRead`, swap the stored value from "slug" to "label", change the query keys from `["sites", ...]` to `["products", ...]`, change the placeholder default to `"Search products by name…"`, and change the selected-option display from `${site.name} (${site.slug})` to `${product.name} (${product.label})`. Keep every other structural piece (debounce, combobox a11y attributes, outside-click close) identical.

- [ ] **Step 3: Add `product_select` to the node manifest zod enum + `GenericNodeForm`**

In `frontend/src/lib/api/nodeTypes.ts`, add `"product_select"` to `NodeFieldControl`'s enum list.

In `GenericNodeForm.tsx`, add an import `import { ProductSelectControl } from "@/components/canvas/config/ProductSelectControl";` and a new branch in the control switch, right after the `site_select` branch:
```tsx
) : field.control === "product_select" ? (
  <ProductSelectControl
    id={inputId}
    value={asInputValue(draft[field.name])}
    onChange={(label) => setField(field, label)}
    disabled={disabled}
    placeholder={field.placeholder}
  />
```

- [ ] **Step 4: Catalogue list page**

`frontend/src/components/product-catalogue/ProductCatalogueClient.tsx` — mirror the structure of the Sites list client component (find it, e.g. `frontend/src/components/site/SitesListClient.tsx` or similar — grep `frontend/src/app/products/sites` for its client component) for the list+search+create-modal composition: a page heading, a search input wired to `searchProducts`/`listProducts` via TanStack Query, a table/card list of products (label, name, description, edit/delete actions), and a "New product" button opening `ProductCreateModal.tsx` (a small form: label + name + description, zod-validated via `ProductCreate`, calling `createProduct` on submit and invalidating the products query).

`frontend/src/app/products/catalogue/page.tsx` — a server shell mirroring the Sites page's shell (SSR nothing special needed; just render `<ProductCatalogueClient />`).

- [ ] **Step 5: Nav + copy**

In `frontend/src/lib/uiCopy.ts`, add `catalogue: "Catalogue",` to the `UI` const.
In `TopNav.tsx`, add a link after the Library link:
```tsx
<Link
  href="/products/catalogue"
  className="text-sm font-medium text-nav-muted hover:text-fg"
>
  {UI.catalogue}
</Link>
```

- [ ] **Step 6: Write component tests**

`ProductSelectControl.test.tsx` — mirror whatever test file exists for `SiteSelectControl` (search debounce, selection, clear). `ProductCatalogueClient.test.tsx` — list renders, search filters, create modal submits and shows the new row.

- [ ] **Step 7: Typecheck, lint, test**

Run: `cd frontend && npm run typecheck && npm run lint && npm run test`
Expected: clean.

- [ ] **Step 8: Manual smoke**

Run: `./scripts/dev.sh` (or the full `make up` stack), navigate to `/products/catalogue`, create a product, confirm it appears in the `product_select` dropdown when configuring a `has_product` node on a Rule Canvas.

- [ ] **Step 9: Commit**

```bash
git add frontend/src/lib/api/products.ts frontend/src/components/canvas/config/ProductSelectControl.tsx \
  frontend/src/components/product-catalogue/ frontend/src/app/products/catalogue/ \
  frontend/src/lib/api/nodeTypes.ts frontend/src/components/canvas/config/GenericNodeForm.tsx \
  frontend/src/components/nav/TopNav.tsx frontend/src/lib/uiCopy.ts
git commit -m "feat(frontend): add Product Catalogue UI + product_select control"
```

---

## Task 10: Frontend — Outcomes Library UI + rule-node integration

**Files:**
- Create: `frontend/src/lib/api/savedOutcomes.ts`, `frontend/src/components/canvas/config/SavedOutcomeSelectControl.tsx`, `frontend/src/components/outcomes-library/SavedOutcomeLibraryClient.tsx`, `frontend/src/components/outcomes-library/SavedOutcomeEditorPage.tsx`, `frontend/src/app/products/outcomes/page.tsx`, `frontend/src/app/products/outcomes/[slug]/page.tsx`
- Modify: `frontend/src/lib/api/nodeTypes.ts`, `frontend/src/components/canvas/config/GenericNodeForm.tsx`, `frontend/src/components/nav/TopNav.tsx`, `frontend/src/lib/uiCopy.ts`, the Test panel component that carries device/UA/path inputs (find via `grep -rl "device_type\|user_agent" frontend/src/components` — likely something like `TestPanel.tsx`/`TestARuleTab.tsx`)
- Test: `SavedOutcomeSelectControl.test.tsx`, `SavedOutcomeLibraryClient.test.tsx`, `SavedOutcomeEditorPage.test.tsx`

**Interfaces:**
- Consumes: Task 4's `/api/v1/saved-outcomes` backend routes, Task 9's nav/copy conventions, existing `<ComponentPreview>`, `componentKeys`/`listComponentTemplates`/`getComponentTemplate`/`resolveComponent` from `componentTemplates.ts`.
- Produces: `SavedOutcomeRead`/`Create`/`Update` zod types + CRUD fetchers, `<SavedOutcomeSelectControl>`, the Outcomes library list+editor pages, `logged_in`/`has_product`/identity fields on the Test panel.

- [ ] **Step 1: API client + zod schema**

`frontend/src/lib/api/savedOutcomes.ts` — mirror `componentTemplates.ts`'s plain-fetcher style:
```ts
import { z } from "zod";

import { apiGet, apiSend, Page } from "@/lib/api/client";
import { SLUG_RE } from "@/lib/api/features";

const BASE = "/api/v1/saved-outcomes";

export const SavedOutcomeRead = z.object({
  id: z.string(),
  slug: z.string(),
  name: z.string(),
  component_id: z.string(),
  component_name: z.string(),
  version_number: z.number().nullable(),
  variables: z.record(z.string(), z.string()),
  created_at: z.string(),
  updated_at: z.string(),
});
export type SavedOutcomeRead = z.infer<typeof SavedOutcomeRead>;

export const SavedOutcomeCreate = z.object({
  slug: z.string().min(3).max(120).regex(SLUG_RE, "Use lowercase kebab-case"),
  name: z.string().min(1).max(200),
  component_id: z.string(),
  version_number: z.number().nullable().optional(),
  variables: z.record(z.string(), z.string()).default({}),
});
export type SavedOutcomeCreate = z.infer<typeof SavedOutcomeCreate>;

export const SavedOutcomeUpdate = z.object({
  name: z.string().min(1).max(200).optional(),
  version_number: z.number().nullable().optional(),
  variables: z.record(z.string(), z.string()).optional(),
});
export type SavedOutcomeUpdate = z.infer<typeof SavedOutcomeUpdate>;

export const ResolvedSavedOutcomeRead = z.object({ html_body: z.string() });
export type ResolvedSavedOutcomeRead = z.infer<typeof ResolvedSavedOutcomeRead>;

export interface ListSavedOutcomesParams {
  page: number;
  page_size: number;
  q?: string;
}

export const listSavedOutcomes = (p: ListSavedOutcomesParams = { page: 1, page_size: 20 }) => {
  const params = new URLSearchParams({ page: String(p.page), page_size: String(p.page_size) });
  if (p.q && p.q.trim() !== "") params.set("q", p.q.trim());
  return apiGet(`${BASE}?${params.toString()}`, Page(SavedOutcomeRead));
};

export const getSavedOutcome = (id: string) => apiGet(`${BASE}/${id}`, SavedOutcomeRead);

export const createSavedOutcome = (b: SavedOutcomeCreate) =>
  apiSend("POST", BASE, SavedOutcomeRead, b);

export const updateSavedOutcome = (id: string, b: SavedOutcomeUpdate) =>
  apiSend("PATCH", `${BASE}/${id}`, SavedOutcomeRead, b);

export const deleteSavedOutcome = (id: string) => apiSend("DELETE", `${BASE}/${id}`, z.void());

export const resolveSavedOutcome = (id: string) =>
  apiGet(`${BASE}/${id}/resolve`, ResolvedSavedOutcomeRead);

const SEARCH_PAGE_SIZE = 10;
export const searchSavedOutcomes = (q: string) =>
  listSavedOutcomes({ page: 1, page_size: SEARCH_PAGE_SIZE, q });

export const savedOutcomeKeys = {
  list: (p: ListSavedOutcomesParams) => ["saved-outcomes", "list", p] as const,
  detail: (id: string) => ["saved-outcomes", "detail", id] as const,
};
```

- [ ] **Step 2: `SavedOutcomeSelectControl.tsx`**

Copy `SiteSelectControl.tsx` and adapt: rename, swap `getSite`/`searchSites` for `getSavedOutcome`/`searchSavedOutcomes`, stored value is the saved outcome's `id` (a UUID, not a slug), display label is `${outcome.name}` (no slug suffix needed since the id isn't human-meaningful — show `${outcome.name} (${outcome.component_name})` instead for context), query keys `["saved-outcomes", ...]`.

- [ ] **Step 3: Add `saved_outcome_select` to the manifest enum + `GenericNodeForm`**

In `nodeTypes.ts`, add `"saved_outcome_select"` to `NodeFieldControl`.

In `GenericNodeForm.tsx`, add the control branch (mirroring the `product_select` addition from Task 9):
```tsx
) : field.control === "saved_outcome_select" ? (
  <SavedOutcomeSelectControl
    id={inputId}
    value={asInputValue(draft[field.name])}
    onChange={(id) => setField(field, id)}
    disabled={disabled}
    placeholder={field.placeholder}
  />
```
No Variables sub-form is needed for `apply_saved_outcome`/`apply_saved_outcome_json` nodes (their manifest fields are `saved_outcome_id`/`target_selector`/`placement_mode` or `target_path` — all handled by the generic `select`/`text` branches already, plus the new `saved_outcome_select` branch above). Do NOT add these two kinds to `COMPONENT_KINDS`.

- [ ] **Step 4: Extract a shared `VariablesForm` (optional but recommended reuse)**

If the editor in Step 5 needs the same "one input per declared variable, labelled by title/description" rendering that `GenericNodeForm.tsx` already has inline (lines building the Variables sub-form for `apply_component`/`apply_component_json`), extract that block into `frontend/src/components/shared/VariablesForm.tsx` with props `{ declaredVars: ComponentVariable[], values: Record<string,string>, onChange: (name: string, value: string) => void, disabled?: boolean }`, and use it from BOTH `GenericNodeForm.tsx` (replacing its inline block) and the new `SavedOutcomeEditorPage.tsx` (Step 5). Update `GenericNodeForm.test.tsx` if it directly tested the inline markup (test ids like `data-testid="component-variables"` must be preserved on the extracted component so existing tests keep passing).

- [ ] **Step 5: Outcomes Library list + editor pages**

`frontend/src/components/outcomes-library/SavedOutcomeLibraryClient.tsx` — list page mirroring `ComponentLibraryClient.tsx`'s structure (search, grid of cards showing name + component_name + version_number-or-"Latest", "New outcome" button opening a create modal that just asks for `slug` + `name` + a `component_select` dropdown, since the version/variables are configured after creation in the editor).

`frontend/src/components/outcomes-library/SavedOutcomeEditorPage.tsx` — editor mirroring `ComponentEditorPage.tsx`'s split-layout spirit but simpler (no HTML editing — the HTML lives on the Component, not here):
- A component picker (reuse the same `component_select` fetch pattern from `GenericNodeForm.tsx`: `listComponentTemplates`).
- A version picker: "Latest" (stored as `version_number: null`) + each `version_number` of the chosen component (reuse `getComponentTemplate(componentId).versions`).
- A Variables form (the extracted `VariablesForm` from Step 4) driven by `resolveComponent(componentId, selector)`'s declared `variables[]`, with values bound to local editor state.
- A live `<ComponentPreview html={resolved.html_body} values={variableValues} />` (client-side mustache, same as everywhere else — the backend `resolve` endpoint is only hit by the proxy, not the editor's live preview).
- Save button calling `createSavedOutcome`/`updateSavedOutcome` with `{ slug, name, component_id, version_number, variables }`.

`frontend/src/app/products/outcomes/page.tsx` and `.../[slug]/page.tsx` — server shells rendering the two client components (mirror `app/products/components/page.tsx` and `[slug]/page.tsx`'s shell style; note the editor route is keyed by `slug`, so `SavedOutcomeEditorPage` needs a way to resolve slug → id, e.g. by fetching the list and finding the matching slug, or by adding a `GET /saved-outcomes/by-slug/{slug}` lookup — check whether `component_templates.rs`'s `by-slug` route (seen in `componentTemplates.ts`'s `getComponentTemplateBySlug`) has a backend equivalent worth mirroring; if Task 4 didn't add a `by-slug` route, either add one there or have this page fetch-then-filter the list client-side for the matching slug).

- [ ] **Step 6: Nav + copy**

Add `outcomes: "Outcomes",` to `uiCopy.ts`'s `UI` const. Add a nav link in `TopNav.tsx` after Catalogue:
```tsx
<Link href="/products/outcomes" className="text-sm font-medium text-nav-muted hover:text-fg">
  {UI.outcomes}
</Link>
```

- [ ] **Step 7: Test panel identity inputs**

Find the Test panel component carrying device/UA/path/meta-tag inputs (grep `frontend/src/components` for the "Test a rule" tab's form). Add two inputs: a "Logged in" checkbox/toggle and a products multi-input (a simple tag-style input backed by `searchProducts`, or a plain comma-separated text field if the existing panel favors simplicity for similar multi-value fields — check how `meta_tags` are entered in the same panel and match that pattern). Wire both into the `EvalContext` payload sent to `/__rre/eval`/`/__rre/eval-url` as `logged_in`/`products`.

- [ ] **Step 8: Write component tests**

`SavedOutcomeSelectControl.test.tsx`, `SavedOutcomeLibraryClient.test.tsx` (list + create), `SavedOutcomeEditorPage.test.tsx` (component/version pick → variables render → preview updates → save calls the right payload). Update the Test panel's existing test file to cover the new logged-in/products inputs round-tripping into the eval request.

- [ ] **Step 9: Typecheck, lint, test**

Run: `cd frontend && npm run typecheck && npm run lint && npm run test`
Expected: clean.

- [ ] **Step 10: Manual smoke**

Run the dev stack, create a component in Library, create a saved outcome referencing it with a variable value, confirm the live preview renders, then open a Rule Canvas, add an `apply_saved_outcome` node, pick that saved outcome, save the version, and use "Preview as a visitor" / "Test a rule" to confirm the rendered HTML appears in the response body.

- [ ] **Step 11: Commit**

```bash
git add frontend/src/lib/api/savedOutcomes.ts frontend/src/components/canvas/config/SavedOutcomeSelectControl.tsx \
  frontend/src/components/shared/VariablesForm.tsx frontend/src/components/outcomes-library/ \
  frontend/src/app/products/outcomes/ frontend/src/lib/api/nodeTypes.ts \
  frontend/src/components/canvas/config/GenericNodeForm.tsx frontend/src/components/nav/TopNav.tsx \
  frontend/src/lib/uiCopy.ts
git commit -m "feat(frontend): add Outcomes Library UI + apply_saved_outcome node integration"
```

---

## Task 11: Full-stack verification + Opus review pass

**Files:** none created; this task runs the project-wide gate and a final adversarial review across every file touched by Tasks 1–10.

**Interfaces:** none — this is a verification-only task.

- [ ] **Step 1: Run the full gate**

Run: `make check`
Expected: all per-service gates (backend, proxy, frontend) pass. If any fails, return to the owning task and fix before proceeding — do not patch around a failure here.

- [ ] **Step 2: Fresh-stack smoke test**

Run: `make down && make up`
Confirm: the seed demo feature (`demo-article`) still loads at `http://localhost:9000/article.html` post-migration (the single-canvas collapse must not have broken the seeded rule), and the new nav links (Catalogue, Outcomes) are visible and load without error at `http://localhost:3000/products/catalogue` and `/products/outcomes`.

- [ ] **Step 3: End-to-end new-feature smoke**

Using the running stack: create a product in the Catalogue, create a saved outcome in the Outcomes library referencing an existing (or newly created) Library component, open a feature's Rule Canvas, build a small graph using `logged_in` → `has_product` → `apply_saved_outcome` → End, save as a draft version, and use "Preview as a visitor" / "Test a rule" with the logged-in toggle and a matching product to confirm the graph routes correctly and the saved outcome's rendered HTML appears in the test output.

- [ ] **Step 4: Dispatch the Opus review**

Launch an Opus-model code-reviewer agent (or invoke the `code-review` skill at `high` effort) scoped to the full diff introduced by Tasks 1–10 (`git diff master...HEAD` or the equivalent range). Ask it to specifically check: (a) every backend `AppError` variant added has a correct status-code mapping, (b) the `rule_graph_service::validate` parameter threading (outcome/component/product/saved-outcome id sets) is complete at every call site, (c) no leftover `Canvas`/`CanvasKey`/`classifier` references survive anywhere in `proxy/src` or `frontend/src`, (d) every new proxy applier path fails open and never panics, (e) frontend zod schemas match the backend DTOs field-for-field.

- [ ] **Step 5: Address findings**

Fix any BLOCKER/HIGH findings from the review inline, re-run `make check`, and re-request review on just the changed files if the fixes were non-trivial.

- [ ] **Step 6: Final commit**

```bash
git add -A
git commit -m "chore: address final review findings"
```
(Skip this commit if Step 5 found nothing to fix.)
