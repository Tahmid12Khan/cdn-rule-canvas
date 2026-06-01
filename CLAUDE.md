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

1. `docker compose --env-file infra/.env -f infra/docker-compose.yml up -d postgres`
2. `cp backend/.env.example backend/.env && cd backend && PATH="$HOME/.cargo/bin:$PATH" cargo run --bin rre-backend` (embedded `sqlx::migrate!` runs at startup; `cargo run --bin seed_demo` seeds `dn-article`)
3. `cp frontend/.env.local.example frontend/.env.local && cd frontend && npm run dev` → dashboard at **`/products/features`** (not `/features`)

Demo: proxy applies rules at http://localhost:9000/article.html (default UA→paywall, mobile UA→regwall); raw upstream at http://localhost:9001/article.html.
