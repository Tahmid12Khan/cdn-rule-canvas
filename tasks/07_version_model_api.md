# Task 07 — Version Domain: Model, Status Lifecycle, API

## Goal
Implement the `Version` aggregate (§3.2, §7) including status lifecycle (`DRAFT`/`STAGING`/`LIVE`/`PREV`), publish/unpublish actions, and per-canvas `rule_graph` JSONB column initialized to empty.

## Dependencies
Task 05.

## Acceptance Criteria
- Migration creates `rre.versions`: `id` (UUID), `feature_id` (FK→features.id, cascade delete), `version_number` (int, unique per feature, monotonic), `description`, `created_by`, `last_updated_by`, `last_updated_at` (auto), `status` (enum), `rule_graph` (JSONB), `created_at`.
- `rule_graph` JSON shape: `{"anonymous": {"nodes":[],"edges":[]}, "registered": {...}, "customer": {...}}`. Validated by a serde schema.
- Endpoints:
  - `POST /api/v1/features/{fid}/versions` — body `{description}`, creates DRAFT version cloned from current LIVE (or empty graph if none).
  - `GET /api/v1/features/{fid}/versions?status=&search=&page=` — paginated list, filterable.
  - `GET /api/v1/features/{fid}/versions/{vnum}` — detail.
  - `PATCH /api/v1/features/{fid}/versions/{vnum}` — edit description, rule_graph (in edit mode), last_updated_at touched.
  - `POST /api/v1/features/{fid}/versions/{vnum}/publish` — body `{environment: "staging"|"live"}`. Transitions per §7.
  - `POST /api/v1/features/{fid}/versions/{vnum}/unpublish` — body `{environment}`.
  - `DELETE /api/v1/features/{fid}/versions/{vnum}` — only allowed if status is `DRAFT` or `PREV`.
- Publishing rules:
  - Publish-to-live: any previously-live becomes `PREV`. New version becomes `LIVE`. `features.live_version_id` updated.
  - Publish-to-staging: previously-staging becomes `PREV` (unless it is also live, then keep `LIVE`). New becomes `STAGING`. The "+1" badge concept is derived in the UI when same id sits in both slots.
- Concurrency: publish transitions guarded by a row lock + service-level transaction.
- 95% coverage on lifecycle service.

## Implementation Steps
1. **Model** `src/models/version.rs`. `version_number` set by service using `SELECT MAX(version_number) FROM rre.versions WHERE feature_id = $1 FOR UPDATE` inside a `pool.begin()` transaction.
2. **Status enum:** `DRAFT`, `STAGING`, `LIVE`, `PREV` (`#[derive(sqlx::Type)]`, Postgres `version_status` enum).
3. **serde schemas** including an internally-tagged enum for graph node payloads (kept loose for now: `serde_json::Value` validated only structurally).
4. **Service `version_service.rs`:**
   - `create_version(feature_id, description, user)` — clones from current LIVE.
   - `publish(feature_id, version_number, environment, user)` — handles transitions atomically.
   - `unpublish(...)`, `delete(...)` with status guard.
   - `update_rule_graph(...)` — used in Task 14.
5. **Router** `src/api/v1/versions.rs` nested under feature.
6. Migration `0003_versions.up.sql` / `0003_versions.down.sql` — also wires the deferrable FK columns on features to enforce now.
7. **Tests:**
   - Service unit: publish-to-live demotes previous live to PREV; publish-to-staging when same version was live keeps `LIVE` (model "+1" by recording the same id in both slots).
   - Integration: full publish workflow + unpublish + delete guards.

## Files
- `backend/src/models/version.rs`
- `backend/src/schemas/version.rs`
- `backend/src/repositories/version_repository.rs`
- `backend/src/services/version_service.rs`
- `backend/src/api/v1/versions.rs`
- `backend/migrations/0003_versions.up.sql`, `0003_versions.down.sql`
- `backend/tests/version_service.rs` (unit)
- `backend/tests/versions_api.rs` (integration)

## Tests
- `version_service.rs::create_clones_from_live`
- `version_service.rs::publish_to_live_demotes_previous`
- `version_service.rs::publish_to_staging_keeps_live`
- `version_service.rs::unpublish_marks_prev`
- `versions_api.rs::delete_prev_allowed`
- `versions_api.rs::delete_live_forbidden`

## Verify
1. Create feature, create 3 versions, publish v2 to staging, v1 to live → statuses: v1=LIVE, v2=STAGING, v3=DRAFT.
2. Publish v2 to live → v1 becomes PREV, v2 becomes LIVE.
3. Delete v3 → 204; delete v2 (LIVE) → 409.
4. `cargo test` green.

## Done When
PR merged with Swagger trace of publish sequence in description.
