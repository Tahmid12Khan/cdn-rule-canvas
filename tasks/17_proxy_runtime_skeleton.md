# Task 17 — Proxy Runtime Skeleton

## Goal
Stand up the Rust proxy server (Axum/tower) that transparently forwards HTTP requests to an upstream and returns the response unchanged. Add a demo upstream (static HTML server) to Docker Compose so the chain is visibly working end-to-end.

## Dependencies
Task 04.

## Acceptance Criteria
- New service `proxy/` running on port `9000`.
- `GET http://localhost:9000/<any-path>` is forwarded to `UPSTREAM_BASE_URL` (default `http://demo-upstream:8081`) preserving method, headers (minus hop-by-hop), and body. Response (status, headers, body) passed through.
- Demo upstream `infra/demo-upstream/` serves a static HTML file `article.html` with `<meta name="paywall" content="true">` and a `<div id="article-body">…</div>` placeholder.
- `docker compose up` brings up: postgres, adminer, backend, frontend, proxy, demo-upstream.
- Proxy adds an `X-RRE-Trace-Id` header to outgoing requests + responses (UUID per request).
- Logs each request with method, path, upstream status, latency.
- Unit + integration tests using `wiremock` (fake upstream) + `reqwest`.

## Implementation Steps
1. **Proxy structure:**
   ```
   proxy/
   ├── src/
   │   ├── main.rs             # binary entry: load config, build router, axum::serve
   │   ├── lib.rs              # app factory: `pub fn build_app(state) -> Router`
   │   ├── forwarder.rs        # upstream client
   │   ├── config.rs           # Settings via envy + dotenvy
   │   ├── telemetry.rs        # tracing-subscriber (json) setup
   │   └── middleware/
   │       ├── mod.rs
   │       └── trace.rs
   ├── tests/
   ├── Cargo.toml
   └── Dockerfile
   ```
2. Deps: `axum`, `tokio` (`full`), `tower`, `tower-http` (`trace`), `hyper`, `reqwest` (`stream`), `serde`, `envy`, `dotenvy`, `tracing`, `tracing-subscriber` (`json`), `uuid` (`v4`). Dev: `wiremock`, `http-body-util`.
   - **zen rule engine (added now so later tasks don't churn `Cargo.toml`).** Depend on zen via **path**, mirroring the working `playground/Cargo.toml` template:
     ```toml
     zen-engine     = { path = "../core/engine" }
     zen-expression = { path = "../core/expression" }
     # zen-types is re-exported through zen-engine (`zen_engine::model`, etc.).
     # Only add `zen-types = { path = "../core/types" }` if a type must be named directly.
     ```
   - **Runtime constraint to bake in from day one:** `zen_expression::variable::Variable` uses `Rc` internally and is **`!Send`**, so a zen `decision.evaluate(...)` cannot be `.await`ed directly on the multi-threaded Axum runtime. Evaluation (build `Variable` → evaluate → serialize back to `serde_json::Value`) must run inside `tokio::task::spawn_blocking` on a `new_current_thread` runtime, with only `Send` `serde_json::Value` crossing the boundary — exactly as `playground/src/main.rs` does today. The skeleton's `AppState` / forwarder pipeline should be structured to accommodate this (Task 18 fills in the call); do not assume the engine can be awaited inline.
3. `build_app(state)` returns an Axum `Router` with one catch-all `fallback` route → `forwarder::forward(state, req)`.
4. `forwarder.rs` uses a shared `reqwest::Client` (built once at startup, held in `AppState`, reused for connection pooling).
5. Hop-by-hop header list per RFC 7230 §6.1 stripped both directions.
6. Demo upstream: `nginx:alpine` mounting the static HTML file (pick nginx for simplicity).
7. Compose service additions with healthchecks.
8. Tests:
   - Unit: hop-by-hop stripper.
   - Integration: `wiremock` simulates upstream; proxy forwards GET/POST, preserves body + headers, returns trace id.

## Files
- `proxy/src/main.rs`, `lib.rs`, `forwarder.rs`, `config.rs`, `middleware/trace.rs`
- `proxy/tests/forward_passthrough.rs`
- `proxy/Dockerfile`
- `proxy/Cargo.toml`
- `infra/demo-upstream/article.html`
- `infra/docker-compose.yml` (extend)

## Tests
- `forward_passthrough.rs::get_forwards_unchanged`
- `forward_passthrough.rs::post_body_preserved`
- `forward_passthrough.rs::hop_by_hop_stripped`
- `trace_middleware.rs::trace_id_header_present`

## Verify
1. `docker compose up` → all services healthy.
2. `curl -i localhost:9000/article.html` → 200 with `<meta name="paywall" content="true">` HTML body and `X-RRE-Trace-Id` header.
3. `docker logs proxy` shows structured log per request.
4. `cargo test` green for `proxy/`.

## Done When
PR merged with curl output proving end-to-end forward through the chain.
