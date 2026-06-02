# Response Rule Engine (RRE)

> Deep-dive companion to the top-level [`README.md`](./README.md). RRE reuses the
> vendored `zen` rules engine (`core/*`, MIT) as a library — see
> [`docs/architecture.md`](docs/architecture.md).

RRE turns visual decision graphs into request-time HTML transformations. Editors
build rule graphs in a web admin; a high-throughput proxy evaluates them against
live traffic (using the zen JDM engine) and rewrites the upstream response —
paywalls, registration walls, content truncation, and more.

## Rules engine (zen)

RRE does not implement its own rules engine. All rule evaluation is delegated to
**zen**, the open-source GoRules Business Rules Engine, whose source lives in this
same repository under `core/*` (and is documented by the main
[`README.md`](./README.md)).

- Upstream project: **https://github.com/gorules/zen**
- The `proxy/` crate translates each canvas `rule_graph` into a zen JDM
  `DecisionContent`, then runs it through `zen_engine::DecisionEngine` —
  see [`docs/architecture.md`](docs/architecture.md) and
  [`CONTRACTS.md`](./CONTRACTS.md).

## Quick start (one command)

```bash
make up
```

Then open:

- Admin UI — http://localhost:3000
- Proxy demo (transformed article) — http://localhost:9000/article.html
- API docs — http://localhost:8000/docs

`make up` builds every image, waits for the backend, and seeds the demo feature
`dn-article` (a LIVE version the proxy serves + an editable DRAFT). Tear down
with `make down` (or `make down-clean` to also drop the database volume).

No Make? Use the scripts directly:

```bash
./scripts/up.sh        # boot + seed
./scripts/down.sh      # stop  (add --volumes to wipe the DB)
```

## Services & ports

| Service | Host port | Description |
|---|---|---|
| frontend | 3000 | Next.js admin UI |
| backend | 8000 | axum REST API + Postgres (`/docs` for OpenAPI) |
| proxy | 9000 | request-time evaluator + HTML rewriter |
| demo-upstream | 9001 | static demo articles (nginx) |
| postgres | 5432 | state (schema `rre`) |
| adminer | 8080 | DB browser |

## Repository layout (RRE)

```
backend/         Rust REST API + sqlx migrations + seed_demo binary
proxy/           Rust request-time proxy (zen-engine evaluator + HTML applier)
frontend/        Next.js admin (App Router, TypeScript, Tailwind)
infra/           docker-compose.yml, demo-upstream/, compose .env.example
scripts/         up.sh / down.sh
docs/            architecture.md, runbook.md
CONTRACTS.md     authoritative API / schema / engine-integration contract
Makefile         make up / down / seed / check
```

## Configuration

Each service loads one typed `Settings` struct from the environment. Copy the
templates and edit as needed (never commit a real `.env`):

```bash
cp infra/.env.example     infra/.env       # compose-level (POSTGRES_PASSWORD, …)
cp backend/.env.example   backend/.env     # host-side backend runs
cp proxy/.env.example     proxy/.env       # host-side proxy runs
cp frontend/.env.local.example frontend/.env.local
```

The compose service environments are kept consistent with these templates.

## Testing & CI

```bash
make check          # fmt/lint + typecheck + test + build, every service
make backend-check  # just the backend crate
make proxy-check    # just the proxy crate
make frontend-check # just the frontend
```

There is no CI workflow — `make check` runs the per-service gate chain locally;
run it before pushing.

End-to-end:

- `frontend/e2e/full-demo.spec.ts` — drives the running stack (admin UI + the
  proxy transforming the seeded article) via Playwright.
- `frontend/e2e/canvas-roundtrip.spec.ts` — rule-builder canvas round-trip.
- `proxy/tests/proxy_e2e_html.rs` — proxy pipeline against fake upstream/backend.
- `backend/tests/seed_idempotent.rs` — proves the seeder is idempotent.

## How the demo flows

1. `dn-article`'s LIVE version routes the **anonymous** canvas:
   `meta[name=paywall]=true?` → if mobile, **Registration Wall**; else
   **Paywall** (truncate + subscribe block); if no paywall meta, **Show Content**
   (untouched).
2. The proxy resolves the feature from `(host, path)`, fetches the cached
   active-version from the backend, evaluates the canvas with zen, and applies
   the matched outcome's components to the upstream HTML.
3. Responses carry `X-RRE-Trace-Id` and `X-RRE-Apply-Status`.

See [`docs/architecture.md`](docs/architecture.md) for the full design and
[`docs/runbook.md`](docs/runbook.md) for operations.
