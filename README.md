[![License: FSL-1.1-MIT](https://img.shields.io/badge/license-FSL--1.1--MIT-blue.svg)](./LICENSE)

# Response Rule Engine (RRE)

RRE turns visual decision graphs into request-time response transformations.
Editors build rule graphs in a web admin; a high-throughput proxy evaluates them
against live traffic and rewrites the upstream response — paywalls, registration
walls, content truncation, JSON field edits, and more.

Rule evaluation is delegated to **zen**, the open-source GoRules Business Rules
Engine (vendored in `core/*`, MIT — see [License](#license)). The proxy
translates each canvas `rule_graph` into a zen JDM `DecisionContent` and runs it
through `zen_engine::DecisionEngine`.

> Deeper docs: [`RRE_README.md`](./RRE_README.md) ·
> [`docs/architecture.md`](docs/architecture.md) ·
> [`CONTRACTS.md`](./CONTRACTS.md) (authoritative API / schema / engine contract).

## Quick start

```bash
make up
```

Then open:

- Admin UI — http://localhost:3000
- Proxy demo (transformed article) — http://localhost:9000/article.html
- API docs — http://localhost:8000/docs

`make up` builds every image, waits for the backend, and seeds the demo feature
`demo-article` (a LIVE version the proxy serves + an editable DRAFT) and a demo
Site (localhost:9000 → demo-upstream:8081). Tear down with `make down` 
(`make down-clean` also drops the database volume). No Make?
Use `./scripts/up.sh` / `./scripts/down.sh` directly.

## Services & ports

| Service | Port | Description |
|---|---|---|
| frontend | 3000 | Next.js admin UI |
| backend | 8000 | axum REST API + Postgres (`/docs` for OpenAPI) |
| proxy | 9000 | request-time evaluator + response rewriter |
| demo-upstream | 9001 | static demo articles (nginx) |
| postgres | 5432 | state (schema `rre`) |
| adminer | 8080 | DB browser |

## Architecture

RRE has **two planes** that meet only at the database. The **admin plane**
(authors → frontend → backend → Postgres) is where rules are *written*. The
**data plane** (readers → proxy → upstream) is where rules are *applied* to live
traffic. The proxy never touches the editing API directly — it reads only the
published `active-version` snapshot, cached for 30s.

```
   AUTHOR (browser)                                       READER (browser / app)
        │                                                        │
        │ build rule graph                            GET /article.html
        ▼                                                        ▼
 ┌──────────────┐     REST /api/v1        ┌──────────────┐      ┌───────────────────┐
 │   frontend   │◀───────────────────────▶│   backend    │      │       proxy       │
 │ Next.js +    │  create / validate /    │ axum + sqlx  │      │   axum + zen      │
 │ React Flow   │  publish versions       │   :8000      │      │     :9000         │
 │   :3000      │                         └──────┬───────┘      └──┬─────────────┬──┘
 └──────────────┘                       persist  │                 │             │
                                                 │   GET active-   │             │ fetch
                                                 │   version(live) │             │ upstream
                                          ┌──────▼───────┐◀────────┘             ▼
                                          │  postgres    │    cached 30s   ┌───────────────┐
                                          │ schema `rre` │                 │   upstream    │
                                          │    :5432     │                 │    origin     │
                                          └──────────────┘                 │    :9001      │
                                                                           └───────────────┘
            ── admin plane (write) ──                    ── data plane (read) ──
```

The actual rule evaluation is **not** implemented by RRE. The proxy translates
each canvas into a [zen](https://github.com/gorules/zen) JDM `DecisionContent`
(GoRules Business Rules Engine, vendored in `core/*`) and runs it through
`zen_engine::DecisionEngine`.

## How it works

### 1. The domain model — how a rule is structured

A **feature** is the unit an author owns (e.g. `demo-article`), keyed by slug and
typed `html` or `json`. A feature holds many **versions**; each version has a
status (`draft → staging → live → prev`) and carries the actual rule logic.
Everything below a version is JSONB or child rows in Postgres schema `rre`:

```
feature  (slug PK, type: html | json)
  │  live_version_id / staging_version_id   (which version is served)
  └─< version  (version_number, status: draft|staging|live|prev)
        ├── rule_graph  (JSONB) ─┬─ anonymous  ┐
        │                        ├─ registered ├─ 3 independent canvases
        │                        └─ customer   ┘   (one per user class)
        │     each canvas = nodes[] + edges[]:
        │       START ─▶ decision … (branch yes/no) ─▶ expression (apply_outcome) ─▶ END
        ├── applicability (JSONB)   html_selector? / json_selector?
        └─< outcome  (title, is_builtin, order_index)
              └─< component  (type, config JSONB, placement, order_index)
                    html_injection │ content_truncation │ json_set/remove/replace
```

- A **canvas** is a directed graph. **Decision** nodes branch the flow on a test
  (page meta tag, device type, URL, JSON field); **expression** nodes run an
  action — the key one being `apply_outcome`, which attaches an outcome's
  components to the response and continues.
- An **outcome** is a named bundle of **components**. A component is one concrete
  edit: inject HTML (paywall/regwall block), truncate content, or set/remove/
  replace a JSON field. `placement` + `order_index` decide where and in what
  order edits land.
- Three canvases per version means one rule set can route **anonymous**,
  **registered**, and **customer** readers down entirely separate paths — the
  hard isolation boundary at request time.

Authoritative schema, routes, and JSON shapes live in [`CONTRACTS.md`](./CONTRACTS.md).

### 2. How a rule is created (admin plane)

1. Author creates a feature, then a **DRAFT** version (cloned from the current
   LIVE rule graph, or an empty 3-canvas if none).
2. In the **React Flow** rule builder, the author drags decision/expression nodes
   onto a canvas, wires edges, and defines outcomes + components. The palette is
   driven by the backend's **node-type manifest** (`GET /api/v1/node-types`) —
   adding a node type is one manifest entry, no frontend code.
3. On save, the backend **validates** the rule graph (no cycles, edges resolve,
   exactly one START, every path reaches END, `apply_outcome` references a real
   outcome, processor fields satisfy the manifest) → 422 with per-node detail on
   failure.
4. **Publish** flips the version's status and points the feature at it:
   `publish live` → target becomes LIVE (old LIVE → PREV) and
   `features.live_version_id` updates. A `staging` track lets you preview before
   going live. Only DRAFT versions are editable (`VERSION_EDIT_LOCKED` otherwise).

### 3. How a rule is applied (data plane)

For every reader request, the proxy runs this pipeline (`proxy/src/forwarder.rs`).
The hard invariant: it can **always** degrade to "serve upstream untouched" — no
failure panics into the client.

```
 reader request
      │
      ▼
 1. parse Host header ─▶ site_map.resolve(source_host:port) ─▶ upstream destination
                           miss ─▶ fallback to default upstream
 2. classifier(headers) ─▶ Canvas (cookie rre_user_type, default anonymous)
 3. backend.fetch(ALL features, live) ─▶ cached list (moka cache, TTL 30s)
                                           err/none ─▶ fail-open
 4. fetch upstream response                               non-HTML ─▶ pass-through
 5. for each feature: translate canvas ─▶ zen DecisionContent (compiled cache, ≤256 LRU)
       └─ evaluate in spawn_blocking + current-thread rt  ─▶ outcomeId? (or none)
 6. apply matched outcome's components to body            per-component fail-open
       (order_index ASC; html via lol_html, json via path edits; idempotent)
 7. re-encode (gzip) ─▶ response + X-RRE-Apply-Status + X-RRE-Trace-Id
```

- **Step 1**: Sites provide host-based routing; the `site_match` decision node
  enables per-site branching in rule graphs.
- **Step 2 is the isolation boundary**: an anonymous request evaluates *only*
  `rule_graph.anonymous` — no code path lets one class reach another's graph.
- **Step 5** is the zen handoff. Each Decision node → a `CustomNode` +
  `SwitchNode` pair; each `apply_outcome` expression → an `ExpressionNode` +
  `OutputNode` emitting `{ outcomeId }`. A single `CanvasNodeAdapter` dispatches
  custom nodes to pure `CanvasProcessor` impls (`meta_tags`, `device_type`,
  `site_match`, …). zen evaluation is `!Send`, so it runs off the async runtime
  in `spawn_blocking`.
- **Step 6** turns the matched outcome's components into edits. HTML rewrites
  stream through `lol_html`; injected HTML is sanitized (`ammonia`). Every
  renderer is idempotent (marker-guarded), so a re-run is a no-op.

Latency budget is **< 30ms p99** of added proxy overhead; eval < 5ms, transform
< 20ms. See [`CONTRACTS.md` PROXY §1–§3](./CONTRACTS.md) for the full sequence,
failure-mode table, and cache plan.

## Repository layout

```
backend/    Rust REST API + sqlx migrations + seed_demo binary
proxy/      Rust request-time proxy (zen-engine evaluator + response applier)
frontend/   Next.js admin (App Router, TypeScript, Tailwind)
infra/      docker-compose.yml, demo-upstream/, .env templates
scripts/    up.sh / down.sh
docs/       architecture.md, runbook.md
core/       vendored GoRules zen engine (MIT) — the rule evaluator
CONTRACTS.md  authoritative API / schema / engine-integration contract
Makefile      make up / down / seed / check
```

## Testing & CI

```bash
make check          # fmt/lint + typecheck + test + build, every service
make backend-check  # backend crate only
make proxy-check    # proxy crate only
make frontend-check # frontend only
```

`make check` is the single gate — run it before pushing. There is no CI
workflow; quality gates run locally.

## License

RRE is **source-available** under the **Functional Source License, Version 1.1,
MIT Future License (FSL-1.1-MIT)** — see [`LICENSE`](./LICENSE). You may use,
modify, and redistribute it for any purpose **except a Competing Use**: offering
a product or service that substitutes for, or provides substantially the same
functionality as, RRE. Two years after each version is published, that version
also becomes available to you under the MIT License.

The vendored GoRules **zen** engine (`core/*`) remains under the **MIT License**
© GoRules.io — see [`NOTICE`](./NOTICE).
