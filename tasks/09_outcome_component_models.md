# Task 09 — Outcome & Component Domain (Backend)

## Goal
Implement the Outcome (§3.3) and Component (§3.4) aggregates, scoped to a Version. Provide CRUD endpoints and the built-in `ShowContent` seed outcome auto-created on version creation.

## Dependencies
Task 07.

## Acceptance Criteria
- Migration creates `rre.outcomes` and `rre.components`:
  - `outcomes`: `id` UUID PK, `version_id` FK→versions, `title`, `description` (nullable), `is_builtin` bool, `order_index` int, timestamps.
  - `components`: `id` UUID PK, `outcome_id` FK→outcomes (cascade), `slug`, `type` (string), `config` JSONB, `placement` enum (`inline`, `sticky_footer`, `popup`), `order_index` int, timestamps.
- On `POST /versions`, service auto-creates a `ShowContent` outcome (`is_builtin=true`). It cannot be deleted.
- Endpoints (nested):
  - `GET /api/v1/versions/{vid}/outcomes` — list ordered.
  - `POST /api/v1/versions/{vid}/outcomes` — create.
  - `GET /api/v1/outcomes/{oid}` — detail with components.
  - `PATCH /api/v1/outcomes/{oid}` — title/description/order.
  - `DELETE /api/v1/outcomes/{oid}` — 409 if `is_builtin`.
  - `POST /api/v1/outcomes/{oid}/clone` — deep-clone including components.
  - `POST /api/v1/outcomes/{oid}/components` — add component.
  - `PATCH /api/v1/components/{cid}` — update slug/type/config/placement/order.
  - `DELETE /api/v1/components/{cid}`.
  - `POST /api/v1/outcomes/{oid}/reorder` — body `[{id, order_index}]`.
- Version mutation guard: outcomes/components on a non-DRAFT version → 409 "Version is published; clone to edit". Service enforces.

## Implementation Steps
1. Models + enums (`Placement`).
2. Migration `0004_outcomes_components.up.sql` / `0004_outcomes_components.down.sql`.
3. Schemas with `OutcomeRead` nesting `Vec<ComponentRead>`.
4. Repositories + services. `outcome_service::clone_outcome(...)` reuses the repo with new IDs.
5. Hook into `version_service::create_version` to seed `ShowContent`.
6. Router file: `src/api/v1/outcomes.rs` (mounted with and without version prefix).
7. Tests:
   - Seed: version create yields one builtin outcome.
   - Cannot delete builtin.
   - Clone deep-copies components.
   - Reorder updates `order_index`.
   - Edit-locked when version not DRAFT.

## Files
- `backend/src/models/outcome.rs`, `component.rs`
- `backend/src/schemas/outcome.rs`, `component.rs`
- `backend/src/repositories/outcome_repository.rs`, `component_repository.rs`
- `backend/src/services/outcome_service.rs`
- `backend/src/api/v1/outcomes.rs`
- `backend/migrations/0004_outcomes_components.up.sql`, `0004_outcomes_components.down.sql`
- `backend/tests/outcome_service.rs` (unit)
- `backend/tests/outcomes_api.rs` (integration)

## Tests
- See above bullet list — minimum 8 tests.

## Verify
1. Create version → `GET /versions/{vid}/outcomes` returns 1 item titled "Show Content".
2. Create outcome "DN Regwall 1.0" with description; clone it → new outcome with cloned components.
3. Publish version → component PATCH returns 409.

## Done When
PR merged with Swagger sequence captured in description.
