# Task 20 — End-to-End Demo + Hardening

## Goal
Tie everything together into a polished demo, add the operational essentials, and produce a one-command developer setup. This task is the visible MVP: a stakeholder can build a paywall rule in the dashboard and see it take effect on a real HTML page within minutes.

## Dependencies
Task 19.

## Acceptance Criteria
- `make up` (or `./scripts/up.sh`) starts the full stack via Docker Compose: postgres, adminer, backend, frontend, proxy, demo-upstream. Runs `sqlx migrate run` and seeds demo data automatically.
- Seed data (`backend/src/bin/seed_demo.rs`):
  - Feature `dn-article` (html).
  - Three versions; v3 LIVE.
  - Outcomes: "Show Content" (builtin), "Show Paywall" (with Content Truncation + HTML Injection), "Show Regwall" (sticky footer).
  - Anonymous canvas graph: `MetaTags(paywall, contains, true)` YES → DeviceType(equals, mobile) YES → "Show Regwall"; DeviceType NO → "Show Paywall"; outer NO → "Show Content".
  - Registered canvas: `MetaTags(paywall, contains, true)` YES → "Show Regwall"; NO → "Show Content".
  - Customer canvas: "Show Content" only.
- Demo upstream serves two pages: `article.html` (`paywall=true`) and `free.html` (no paywall meta).
- **Observability:**
  - Structured JSON logs across backend, proxy. Correlation `X-RRE-Trace-Id` propagated.
  - `GET /metrics` Prometheus endpoint on backend and proxy (`metrics` + `metrics-exporter-prometheus`).
  - Counters: requests by feature, outcomes by outcome_id, processor calls, apply errors. Histograms: end-to-end latency.
- **CI (GitHub Actions):**
  - Workflow `.github/workflows/ci.yml` runs on PR: lint (clippy + eslint), format (rustfmt + prettier), typecheck (tsc), unit tests (backend, frontend, proxy), integration tests with the `testcontainers` crate, build images.
  - Coverage uploaded as artifact.
- **Documentation:**
  - Root `README.md` with architecture diagram (ASCII or PNG), quickstart, demo walkthrough.
  - `docs/architecture.md` capturing service boundaries and data flow.
  - `docs/runbook.md` with common ops actions (publish, rollback by republishing PREV).
- **Playwright E2E test** `frontend/e2e/full-demo.spec.ts`:
  1. Loads dashboard.
  2. Opens version 3, asserts a node count in Anonymous canvas.
  3. Hits proxy `/article.html` via `request` API and asserts response contains "SUBSCRIBE NOW".
  4. Hits proxy `/free.html` and asserts response is unmodified.
- **Smoke checks** in CI: spin up compose, curl proxy, assert "SUBSCRIBE NOW" appears on the paywalled article only.

## Implementation Steps
1. Seed binary: `cargo run --bin seed_demo` invoked from the compose entrypoint when `RRE_SEED_DEMO=true`.
2. `scripts/up.sh`, `scripts/down.sh`, `Makefile` targets.
3. Metrics: add `metrics` + `metrics-exporter-prometheus`, expose `/metrics` from both services, register counters/histograms in service modules.
4. Logging: shared `telemetry.rs` (`tracing-subscriber`) configured via env (json in prod, pretty in dev).
5. CI workflow with test matrix split (backend / frontend / proxy parallel jobs).
6. Docs.
7. Playwright E2E.

## Files
- `Makefile`, `scripts/up.sh`, `scripts/down.sh`
- `backend/src/bin/seed_demo.rs`
- `backend/src/observability.rs`, `proxy/src/observability.rs`
- `.github/workflows/ci.yml`
- `README.md`, `docs/architecture.md`, `docs/runbook.md`
- `frontend/e2e/full-demo.spec.ts`
- `infra/demo-upstream/free.html`

## Tests
- Backend: `backend/tests/seed_idempotent.rs` — running seed twice doesn't duplicate.
- Proxy: `proxy/tests/metrics_endpoint.rs` — counters increment on request.
- Frontend: full Playwright spec.

## Verify
1. Fresh machine: `git clone … && make up && open http://localhost:3000`.
2. Inspect seeded "DN Article" feature → version 3 is LIVE → open canvas → see seeded graph.
3. `curl localhost:9000/article.html` → paywall applied.
4. `curl localhost:9000/free.html` → unmodified.
5. Set cookie `rre_user_type=customer` → no modification anywhere.
6. CI passes for `main`.

## Done When
PR merged. Architecture diagram + README walkthrough committed. Tag release `v0.1.0-mvp`.

---

# Post-MVP Backlog (not in this plan)

Tracked in `docs/backlog.md`. Includes Analytics views, Access Permissions/RBAC, Split Tests, Sub Rules, multi-output Rule Templates, JSON feature type, Component Registry UI, Gift/Campaign Tokens, Custom Segments, Webhook integrations, full audit log, multi-tenant isolation, environment promotion workflows, and the Layout View for the outcome editor.
