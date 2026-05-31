# Task 02 — Backend Scaffold

## Goal
Stand up an Axum app with a layered structure (api/services/repositories/models/schemas), strict typing, and a cargo test harness. Expose a `/health` endpoint that returns build metadata.

## Dependencies
Task 01 (independent, but starts at this point).

## Acceptance Criteria
- `cd backend && cargo run` (or `cargo watch -x run`) boots and listens on port 8000.
- `GET http://localhost:8000/health` returns `{"status":"ok","version":"<semver>","commit":"<short_sha>"}`.
- `GET http://localhost:8000/docs` renders OpenAPI Swagger UI (via `utoipa-swagger-ui`).
- `cargo test` passes with ≥1 test.
- `cargo clippy --all-targets -- -D warnings` and `cargo fmt --check` pass clean.

## Implementation Steps
1. Initialize with cargo: package name `rre-backend`, Rust 2021 edition (`cargo init --name rre-backend`).
2. Dependencies (`Cargo.toml`): `axum`, `tokio` (`features = ["full"]`), `tower`, `tower-http` (`trace`, `cors`), `serde`, `serde_json`, `dotenvy`, `envy`, `tracing`, `tracing-subscriber` (`json`), `utoipa`, `utoipa-swagger-ui`. Dev: `reqwest`, `tower` (`util` for `oneshot`), `http-body-util`.
3. Folder layout:
   ```
   backend/
   ├── src/
   │   ├── main.rs              # binary entry: load config, build router, axum::serve
   │   ├── lib.rs               # app factory: `pub fn build_app(state) -> Router`
   │   ├── config.rs            # Settings via envy + dotenvy (.env)
   │   ├── telemetry.rs         # tracing-subscriber setup
   │   ├── api/
   │   │   ├── mod.rs
   │   │   └── v1/
   │   │       ├── mod.rs
   │   │       └── health.rs
   │   └── schemas/
   │       ├── mod.rs
   │       └── health.rs
   ├── tests/
   │   └── health.rs            # integration test driving the Router
   ├── Cargo.toml
   ├── rustfmt.toml
   ├── clippy.toml
   └── .env.example
   ```
4. `build_app(state) -> Router` factory pattern — lets tests instantiate isolated routers without binding a socket.
5. `Settings` reads `APP_ENV`, `APP_VERSION`, `GIT_COMMIT` (injected at deploy) via `envy::from_env::<Settings>()`.
6. `/health` returns a `HealthResponse { status, version, commit }` struct (`#[derive(Serialize, utoipa::ToSchema)]`).
7. Integration test uses `tower::ServiceExt::oneshot` against `build_app(...)` over an in-memory request — no network bind.

## Files
- `backend/src/main.rs`
- `backend/src/lib.rs`
- `backend/src/api/v1/health.rs`
- `backend/src/schemas/health.rs`
- `backend/src/config.rs`
- `backend/tests/health.rs`
- `backend/Cargo.toml`

## Tests
- `tests/health.rs::health_returns_ok` — asserts 200, status="ok", version present.

## Verify
1. `curl localhost:8000/health` → JSON with `status: "ok"`.
2. Open `localhost:8000/docs` → Swagger shows `/health`.
3. `cargo test` → 1 test passes; `cargo llvm-cov --summary-only` shows > 80% on touched code.

## Done When
PR merged with screenshot of Swagger UI and curl output in description.
