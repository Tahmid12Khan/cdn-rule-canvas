# Task 04 — Database, Migrations, Docker Compose

## Goal
Add PostgreSQL via Docker Compose, wire an async SQLx `PgPool`, and create the first sqlx migration. Add a `/healthz/db` endpoint that confirms the DB connection.

## Dependencies
Task 02, 03.

## Acceptance Criteria
- `docker compose up postgres` starts Postgres 16 with persisted volume.
- `sqlx migrate run` applies the baseline migration (creates schema `rre` and the `_sqlx_migrations` ledger).
- `GET /healthz/db` returns `{"db":"ok","latency_ms":N}`.
- Test DB uses a `testcontainers`-crate-managed Postgres container OR a separate `rre_test` schema (decide and document).
- Repository trait scaffold added (`src/repositories/mod.rs`) but no domain tables yet.

## Implementation Steps
1. **Compose:**
   - `infra/docker-compose.yml` with `postgres:16-alpine`, env `POSTGRES_DB=rre`, `POSTGRES_USER=rre`, `POSTGRES_PASSWORD=rre`, port `5432`, named volume `rre-pg-data`.
   - Add `adminer` service on port 8080 for visual confirmation.
2. **Backend deps:** `sqlx` with `features = ["runtime-tokio-rustls", "postgres", "uuid", "chrono", "json", "migrate"]`; dev: `testcontainers` + `testcontainers-modules` (`postgres` feature).
3. **Config:** `Settings.database_url` = `postgres://rre:rre@localhost:5432/rre` (SQLx connection string; loaded from `DATABASE_URL`).
4. **Pool + state:**
   - `src/db.rs`: `pub async fn connect(url: &str) -> sqlx::PgPool` via `PgPoolOptions::new().max_connections(..).connect(url)`.
   - Schema: SQLx has no ORM `Base`; create schema `rre` in the baseline migration and use schema-qualified table names (or set `search_path=rre`).
   - `src/state.rs`: `AppState { pool: PgPool, settings: Settings }` shared with handlers via `State<AppState>` (replaces the `get_db()` dependency — handlers acquire connections from the pool).
5. **sqlx migrations:**
   - Create the migrations dir + first migration: `sqlx migrate add -r baseline`.
   - Write `migrations/0001_baseline.up.sql` (`CREATE SCHEMA IF NOT EXISTS rre;`) and `migrations/0001_baseline.down.sql` (`DROP SCHEMA rre CASCADE;`).
   - Embed migrations in the binary with `sqlx::migrate!("./migrations")` run at startup, or run via `sqlx migrate run` in CI/entrypoint.
6. **Health DB endpoint:** `src/api/v1/health.rs::healthz_db` — runs `sqlx::query("SELECT 1").execute(&pool)`, returns latency.
7. **Test harness:** `tests/common/mod.rs` spins up a `testcontainers` Postgres, runs `sqlx::migrate!()` once, and hands out a `PgPool`.

## Files
- `infra/docker-compose.yml`
- `backend/src/db.rs`, `src/state.rs`
- `backend/src/repositories/mod.rs`
- `backend/migrations/0001_baseline.up.sql`, `0001_baseline.down.sql`
- `backend/src/api/v1/health.rs` (extended)
- `backend/tests/db_health.rs`
- `backend/tests/common/mod.rs`

## Tests
- `tests/db_health.rs::db_healthz` — uses the testcontainers pool, asserts 200, `db == "ok"`.
- `tests/migrations.rs::migrate_up_then_down_roundtrip` — full forward + reverse roundtrip.

## Verify
1. `docker compose up -d postgres adminer` → adminer at `localhost:8080` connects.
2. `sqlx migrate run` from `backend/` → applies `0001_baseline`.
3. `curl localhost:8000/healthz/db` → ok with latency.
4. `cargo test` green including testcontainer-backed tests.

## Done When
PR merged with compose file, migration committed, and proof of `sqlx migrate run` in description.
