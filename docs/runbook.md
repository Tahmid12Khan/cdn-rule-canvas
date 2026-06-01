# RRE Runbook

Operational guide for running, seeding, debugging, and tearing down the RRE
stack locally (and the same shape in CI).

## Prerequisites

- Docker + Docker Compose v2 (`docker compose`, not `docker-compose`)
- GNU Make (for the `make` shortcuts; the scripts work without it)
- For host-side dev: Rust (stable), Node 20+

## One-command boot

```bash
make up
```

This:
1. copies `infra/.env.example` → `infra/.env` on first run (edit secrets there),
2. builds all images and starts the stack detached,
3. waits for the backend to report healthy,
4. runs the **idempotent** demo seeder (`seed_demo`).

Equivalent without Make:

```bash
./scripts/up.sh
# or, fully manual:
cp infra/.env.example infra/.env
docker compose --env-file infra/.env -f infra/docker-compose.yml up --build
```

### Endpoints after boot

| URL | What |
|---|---|
| http://localhost:3000 | Admin frontend |
| http://localhost:8000 | Backend REST API |
| http://localhost:8000/docs | OpenAPI / Swagger UI |
| http://localhost:9000/article.html | **Proxy demo** (transformed article) |
| http://localhost:9001/article.html | Raw demo-upstream (untransformed) |
| http://localhost:8080 | Adminer (DB browser) |

## Tear down

```bash
make down          # stop services, keep the postgres volume
make down-clean    # stop AND drop rre-pg-data (fresh DB next boot)
# or:
./scripts/down.sh [--volumes]
```

## Seeding

The seeder is idempotent — safe to re-run anytime:

```bash
make seed
# or against the running container:
docker compose --env-file infra/.env -f infra/docker-compose.yml exec backend seed_demo
```

It creates feature `dn-article` with:
- **LIVE v1** — the worked-example anonymous canvas
  (`paywall` meta → device type → Registration Wall / Paywall / Show Content)
  with three outcomes and their components. This is what the proxy serves.
- **DRAFT v2** — an editable clone for the rule-builder UI.

## Verifying the end-to-end demo

```bash
# Anonymous desktop reader -> Paywall outcome (truncate + subscribe block):
curl -s -H 'Host: localhost:9000' http://localhost:9000/article.html | grep rre-paywall

# Anonymous mobile reader -> Registration Wall:
curl -s -H 'Host: localhost:9000' \
     -H 'User-Agent: iPhone' http://localhost:9000/article.html | grep rre-regwall

# Non-matching path -> pass-through untouched:
curl -si -H 'Host: localhost:9000' http://localhost:9000/free.html | grep -i x-rre-apply-status
```

Every proxied response carries `X-RRE-Trace-Id` and `X-RRE-Apply-Status`
(`ok` | `skipped` | `error`).

## Observability

- **Proxy metrics**: `GET http://localhost:9000/metrics` (Prometheus text;
  histograms `proxy_eval_ms` / `proxy_transform_ms` / `proxy_e2e_ms`, counters
  `proxy_requests_total`, `proxy_outcomes_total`, `proxy_apply_errors_total`).
- **Logs**: `make logs` (or `docker compose ... logs -f <service>`). Structured
  JSON in non-dev (`APP_ENV`); pretty in dev. The proxy never logs bodies or
  cookie values.

## Common issues

| Symptom | Likely cause | Fix |
|---|---|---|
| `POSTGRES_PASSWORD is required` on `up` | no `infra/.env` | `cp infra/.env.example infra/.env` (or rerun `make up`) |
| Proxy returns the untransformed article | feature not LIVE / not seeded | `make seed`; check `GET /api/v1/features/dn-article/active-version?env=live` |
| Proxy pass-through on `/article.html` | Host header ≠ `localhost:9000` | send `Host: localhost:9000` (the feature_map host); use port 9000 |
| Backend unhealthy on boot | postgres not ready / bad `DATABASE_URL` | `make logs`; confirm `postgres` is `healthy` (`make ps`) |
| Frontend shows backend offline | `NEXT_PUBLIC_API_BASE` baked wrong at build | rebuild with the correct build-arg (`make restart`) |
| Stale data after schema change | old volume | `make down-clean` then `make up` |

## CI

`.github/workflows/rre-ci.yml` runs the per-service gate chain
(fmt/lint → typecheck → test → build) for `backend`, `proxy`, and `frontend`,
then builds all three Docker images. The aggregate `ci-ok` job is the one to
require in branch protection for `main`. Rust caches key on `Cargo.lock`
(`Swatinem/rust-cache`); the frontend caches `~/.npm` on the lockfile hash.

## Manual host-side runs (without Docker)

```bash
# Postgres only:
docker compose --env-file infra/.env -f infra/docker-compose.yml up -d postgres

# Backend (host):
cd backend && DATABASE_URL=postgres://rre:rre@localhost:5432/rre cargo run
cd backend && DATABASE_URL=postgres://rre:rre@localhost:5432/rre cargo run --bin seed_demo

# Proxy (host): point upstream/backend at host ports
cd proxy && UPSTREAM_BASE_URL=http://localhost:9001 BACKEND_BASE_URL=http://localhost:8000 cargo run

# Frontend (host):
cd frontend && npm install && npm run dev
```
