# Task 05 — Feature Domain: Model, Schema, API

## Goal
Implement the `Feature` aggregate (§3.1) end-to-end on the backend: SQLx model, sqlx migration, serde schemas, repository, service, and REST endpoints. No frontend yet — visibility comes via Swagger UI.

## Dependencies
Task 04.

## Acceptance Criteria
- Migration creates `rre.features` with columns: `id` (slug PK, max 64), `name`, `type` (enum: `html`, `json`), `staging_version_id` (nullable FK to versions, deferrable), `live_version_id` (nullable FK), `created_at`, `updated_at`.
- `POST /api/v1/features`, `GET /api/v1/features`, `GET /api/v1/features/{id}`, `PATCH /api/v1/features/{id}`, `DELETE /api/v1/features/{id}` all implemented.
- Slug validated: kebab-case, 3–64 chars, lowercase. Duplicate slug → 409.
- Pagination on list: `?page=1&page_size=20`, response envelope `{items, page, page_size, total}`.
- Versions FK columns added as `DEFERRABLE INITIALLY DEFERRED` so we can backfill in Task 07.
- Unit + integration tests ≥ 90% coverage on feature module.

## Implementation Steps
1. **Model** `src/models/feature.rs`:
   ```rust
   #[derive(Debug, Clone, sqlx::FromRow)]
   pub struct Feature {
       pub id: String,                        // slug PK, max 64
       pub name: String,
       pub r#type: FeatureType,               // feature_type enum
       pub staging_version_id: Option<Uuid>,  // FK → rre.versions (deferrable)
       pub live_version_id: Option<Uuid>,
       pub created_at: DateTime<Utc>,
       pub updated_at: DateTime<Utc>,
   }
   ```
2. **Enum** `FeatureType` in `src/models/enums.rs`:
   ```rust
   #[derive(Debug, Clone, Copy, sqlx::Type, Serialize, Deserialize)]
   #[sqlx(type_name = "feature_type", rename_all = "lowercase")]
   pub enum FeatureType { Html, Json }
   ```
3. **Schemas** `src/schemas/feature.rs`: `FeatureCreate`, `FeatureUpdate`, `FeatureRead`, `FeatureList`. Slug regex via `#[validate(regex(...))]` (`validator` crate).
4. **Repository** `src/repositories/feature_repository.rs` — async CRUD using `&PgPool` and `sqlx::query_as!`.
5. **Service** `src/services/feature_service.rs` — business rules (slug uniqueness via unique-violation mapping, etc.).
6. **Router** `src/api/v1/features.rs` — handlers take `State<AppState>` and the feature service.
7. **Migration** `0002_features.up.sql` / `0002_features.down.sql` (creates the `feature_type` enum + `rre.features`).
8. **Tests:**
   - Unit: service-level with a mocked repository trait (slug collision returns a domain error).
   - Integration: full CRUD against testcontainer Postgres, pagination, 404, 409.

## Files
- `backend/src/models/feature.rs`, `enums.rs`
- `backend/src/schemas/feature.rs`, `pagination.rs`
- `backend/src/repositories/feature_repository.rs`
- `backend/src/services/feature_service.rs`
- `backend/src/api/v1/features.rs`
- `backend/migrations/0002_features.up.sql`, `0002_features.down.sql`
- `backend/tests/feature_service.rs` (unit)
- `backend/tests/features_api.rs` (integration)

## Tests
- Service unit tests: slug normalization, collision, not-found.
- API integration: 201 create, 200 list with pagination, 200 get, 200 patch (partial), 204 delete, 404/409 paths.

## Verify
1. `sqlx migrate run` applies the migration.
2. Open Swagger, create feature `dn-article` of type `html`.
3. List → see it in items.
4. Duplicate POST → 409.
5. `cargo test` green.

## Done When
PR merged with Swagger screenshot showing two created features.
