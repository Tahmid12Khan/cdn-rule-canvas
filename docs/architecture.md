# RRE Architecture

The **Response Rule Engine (RRE)** lets product teams author visual decision
graphs ("rule graphs") that, at request time, transform an upstream HTML
response — injecting paywalls, registration walls, truncating content, etc. —
based on page metadata, device, and user class.

It has three runtime services plus a database, all orchestrated by
`infra/docker-compose.yml`.

```
                        ┌──────────────────────────────────────────────┐
   author (browser)     │                  admin plane                  │
        │               │                                               │
        ▼               │   ┌────────────┐        ┌────────────────┐    │
  ┌────────────┐  HTTP  │   │  frontend  │  REST  │    backend     │    │
  │  frontend  │◀───────┼──▶│ (Next.js)  │───────▶│ (axum + sqlx)  │    │
  │   :3000    │        │   │   :3000    │        │     :8000      │    │
  └────────────┘        │   └────────────┘        └───────┬────────┘    │
                        │                                  │            │
                        │                          ┌───────▼────────┐   │
                        │                          │   postgres     │   │
                        │                          │  schema `rre`  │   │
                        │                          │     :5432      │   │
                        │                          └───────┬────────┘   │
                        └──────────────────────────────────┼────────────┘
                                                            │ active-version (TTL 30s)
   reader (browser)                                         │
        │               ┌────────────────────────────────────────────┐
        ▼               │                  data plane                  │
  ┌────────────┐  HTTP  │   ┌────────────┐        ┌────────────────┐  │
  │   reader   │◀───────┼──▶│   proxy    │───────▶│ demo-upstream  │  │
  │            │        │   │ (axum +    │ upstream│   (nginx)      │  │
  │            │        │   │  zen-engine│  fetch  │     :8081      │  │
  │            │        │   │   :9000    │◀────────│                │  │
  └────────────┘        │   └────────────┘  HTML   └────────────────┘  │
                        └────────────────────────────────────────────┘
```

## Services

| Service | Tech | Port (host) | Role |
|---|---|---|---|
| `frontend` | Next.js App Router, TypeScript, Tailwind | 3000 | Admin UI: author features, versions, rule graphs, outcomes, components |
| `backend` | Rust, axum, sqlx (runtime queries) | 8000 | REST API + Postgres persistence + rule-graph validation; serves `/active-version` to the proxy |
| `proxy` | Rust, axum, [zen-engine](../core/engine) | 9000 | Production hot path: classify → evaluate rule graph (JDM) → transform upstream HTML |
| `demo-upstream` | nginx static | 9001 (→8081) | Static demo articles for the end-to-end demo |
| `postgres` | postgres:16-alpine | 5432 | All RRE state in schema `rre` |
| `adminer` | adminer | 8080 | Local DB browser |

## Admin plane (author-time)

1. The author uses the **frontend** to create a `feature`, then `version`s. Each
   version owns three independent canvases (`anonymous`, `registered`,
   `customer`), a set of `outcomes`, and per-outcome `components`.
2. The **backend** persists everything in Postgres (schema `rre`), validates the
   rule graph (cycle/branch/endpoint/outcome-reference rules), and enforces the
   publish lifecycle (`draft → staging/live → prev`).
3. Publishing flips a version to `LIVE`; the feature's `live_version_id` points
   at it.

See [`CONTRACTS.md`](../CONTRACTS.md) for the authoritative API + schema
contract.

## Data plane (request-time)

For every inbound reader request the **proxy** runs this pipeline
(`proxy/src/forwarder.rs`):

1. **feature_map** resolves `(host, path)` → `feature_id` (miss → pass-through).
2. **classifier** picks exactly one canvas from the `rre_user_type` cookie
   (default `anonymous`). This is the canvas-isolation boundary.
3. **backend_client** fetches the cached `active-version` payload (moka, TTL 30s;
   backend error → fail-open pass-through).
4. The proxy fetches the **upstream** response. Non-HTML → streamed untouched.
5. The rule graph for the classified canvas is **translated to a zen JDM
   `DecisionContent`** (cached, compiled) and **evaluated** inside a
   `spawn_blocking` current-thread runtime (zen `Variable` / `scraper::Html` are
   `!Send`). The result yields an `outcomeId` (or none).
6. The matched outcome's **components** are applied to the HTML via streaming
   `lol_html` rewriters (idempotent; selector misses fail-open).
7. The (optionally gzip re-encoded) response is returned with
   `X-RRE-Apply-Status: ok|skipped|error` and an `X-RRE-Trace-Id`.

Hard invariant: a matched request can always degrade to "serve upstream
untouched" — the applier and evaluator never panic into the client.

## zen-engine integration

The proxy reuses the zen JDM decision engine (the same engine that powers the
playground). Each canvas Decision node maps to a `CustomNode` + `SwitchNode`
pair; each Outcome node to an `ExpressionNode` + `OutputNode`. A single
`CanvasNodeAdapter` dispatches custom nodes to pure `CanvasProcessor`
implementations (`metaTags`, `deviceType`). Branch routing relies on
`SwitchNode` statement ids matching edge `source_handle`s. Details in
[`CONTRACTS.md` §8](../CONTRACTS.md).

## Persistence model (schema `rre`)

```
features (slug PK) ──┐
                     ├─< versions (one LIVE + one STAGING max per feature)
                     │      └─< outcomes ──< components
                     └── live_version_id / staging_version_id  (deferrable FK)
```

- `versions.rule_graph` is JSONB holding the three canvases.
- `components.config` is JSONB; the `type` discriminator is CHECK-constrained.
- Migrations live in `backend/migrations/*` and run at startup via
  `sqlx::migrate!`.

## Configuration

Each service reads a single `Settings` struct from the environment
(`envy` + `dotenvy`). Templates: `backend/.env.example`, `proxy/.env.example`,
`frontend/.env.local.example`, and the compose-level `infra/.env.example`. The
`docker-compose.yml` service envs are kept consistent with those templates.
Secrets (e.g. `POSTGRES_PASSWORD`) live only in `infra/.env`, never in committed
files.
