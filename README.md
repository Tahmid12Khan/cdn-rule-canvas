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
`dn-article` (a LIVE version the proxy serves + an editable DRAFT). Tear down
with `make down` (`make down-clean` also drops the database volume). No Make?
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
