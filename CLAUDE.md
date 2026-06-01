# CLAUDE.md

This repo holds **two projects**:
- **zen** — upstream GoRules Business Rules Engine (Rust). Cargo workspace: `core/*`, `bindings/*`, `playground`. Docs: `README.md`.
- **RRE** (Response Rule Engine) — app built ON zen: visual paywall/regwall rule authoring + a request-time HTML-rewriting proxy. Dirs: `backend/ proxy/ frontend/ infra/`. Docs: `RRE_README.md`, `docs/architecture.md`.

**`CONTRACTS.md` (repo root) is the authoritative spec** for DB schema, REST routes, rule_graph JSON, and zen integration. It wins over any `tasks/*.md` on conflicts. Read it before backend/frontend/proxy work.

## Commands

```bash
make up            # build + start full stack, seed demo feature `dn-article`
make down          # stop (down-clean also drops the pg volume)
make check         # all per-service gates (mirrors CI)
make backend-check / proxy-check / frontend-check
```

Frontend (in `frontend/`): `npm run dev | build | lint | typecheck | test` (vitest), `npm run e2e` (Playwright).

Ports: frontend 3000 · backend 8000 · proxy 9000 · demo-upstream 9001 · postgres 5432 · adminer 8080.

## Build gotchas (RRE)

- **`backend/` and `proxy/` are STANDALONE crates** — each `Cargo.toml` has an empty `[workspace]` table detaching it from zen's root workspace. They are NOT in root `Cargo.toml` members; `cargo build` from the repo root won't touch them. Build with `cargo build --manifest-path backend/Cargo.toml`.
- **`cargo` is at `~/.cargo/bin` but not on PATH** in non-interactive shells → prefix commands with `PATH="$HOME/.cargo/bin:$PATH"`.
- **SQLx uses RUNTIME queries only** (`sqlx::query_as::<_, T>(...)`, never the `query!` compile-time macros) → the backend builds without a live DB.
- **proxy depends on zen via path** (`../core/engine`, `../core/expression`). zen-types is re-exported through `zen_engine::model::*` — do NOT add a direct `zen-types` dep.
- **proxy `reqwest` has no `gzip` feature** — the proxy controls encoding itself (`infra::encoding`) so it can rewrite gzipped bodies; auto-decompression would strip `content-encoding`.

## zen integration (the point of the proxy)

Rule eval = translate canvas `rule_graph` → JDM `DecisionContent` (`proxy/src/domain/translator.rs`) → `DecisionEngine` → `.evaluate()`. Add a node type: impl the `CanvasProcessor` trait in `proxy/src/domain/processors/` + one `register()` line in `ProcessorRegistry` — no engine changes. zen SwitchNode conditions reference input fields directly (`branch == 'yes'`), NOT JSONPath.

## Local fast-debug loop (no full compose)

**`make dev`** (= `./scripts/dev.sh`) runs the whole loop in one command: frees native ports 3000/8000/9000, starts postgres + demo-upstream in docker, runs backend/frontend/proxy natively (logs → `.devlogs/`, tailed live), and seeds nothing unless `--seed` is passed (`./scripts/dev.sh --seed`). It's re-runnable (kills the prior run's port owners first); Ctrl-C stops the native services but leaves the docker db + upstream up (`make down` to stop those). The manual steps below are the equivalent done by hand:

1. **db + upstream**: `docker compose --env-file infra/.env -f infra/docker-compose.yml up -d postgres demo-upstream` (postgres :5432; demo-upstream is the proxy's upstream at :9001)
2. `cp backend/.env.example backend/.env && cd backend && PATH="$HOME/.cargo/bin:$PATH" cargo run --bin rre-backend` (embedded `sqlx::migrate!` runs at startup — restart the backend to apply a new migration; `cargo run --bin seed_demo` seeds `dn-article`)
3. `cp frontend/.env.local.example frontend/.env.local && cd frontend && npm run dev` → dashboard at **`/products/features`** (not `/features`)
4. **proxy** (from repo root): `PATH="$HOME/.cargo/bin:$PATH" UPSTREAM_BASE_URL=http://localhost:9001 BACKEND_BASE_URL=http://localhost:8000 cargo run --manifest-path proxy/Cargo.toml`
   - **The two env overrides are REQUIRED natively**: `proxy/config/default.json` points at docker hostnames (`demo-upstream:8081`, `backend:8000`) that don't resolve outside compose → upstream 502 + `active_version=miss` in the log. (Inside `make up` they resolve, so no overrides needed there.)

Demo: proxy applies rules at http://localhost:9000/article.html (default UA→paywall, mobile UA→regwall); raw upstream at http://localhost:9001/article.html.
