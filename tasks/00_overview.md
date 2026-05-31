# Task Plan — Response Rule Engine (RRE)

Sequential build plan derived from `features.md`. Each task is independently mergeable, has explicit acceptance criteria, and ends with a visible artifact (a running screen, a passing API call, or a working end-to-end flow).

## Tech Stack

- **Frontend:** Node.js (active LTS), Next.js (latest stable, App Router), TypeScript (latest stable), TailwindCSS, React Flow, Zustand (state), TanStack Query (server state), Vitest + React Testing Library, Playwright (smoke E2E)
- **Backend:** Axum, Rust (2021 edition), SQLx (async Postgres, compile-time-checked), sqlx migrate, serde + validator, utoipa (OpenAPI/Swagger), PostgreSQL 16, cargo test (`#[tokio::test]`), reqwest
- **Proxy Runtime:** Axum/tower middleware, reqwest (upstream), lol_html + scraper (HTML transform), ammonia (HTML sanitize)
- **Tooling:** clippy + rustfmt (backend/proxy), ESLint + Prettier + tsc (frontend), pre-commit hooks
- **Infra (dev):** Docker Compose (Postgres + backend + frontend + proxy + demo upstream)

## Repository Layout

```
rule_builder/
├── frontend/                  # Next.js admin dashboard
├── backend/                   # Axum admin API
├── proxy/                     # Rust proxy runtime
├── infra/
│   ├── docker-compose.yml
│   └── demo-upstream/         # Static HTML server for end-to-end demo
├── tasks/                     # This plan
└── README.md
```

## Phase Map

| Phase | Tasks | Visible Output at End of Phase |
|---|---|---|
| 1. Foundation | 01–04 | Frontend ↔ Backend ↔ DB all wired, migration runs |
| 2. Feature & Version CRUD | 05–08 | Features list + Version list pages match §4.2/§4.3 |
| 3. Rule Builder | 09–14 | Drag-drop canvas with MetaTags/DeviceType nodes, persistent per canvas |
| 4. Outcome Editor | 15–16 | Edit Outcome page + Component config modal match §4.6/§4.7 |
| 5. Proxy Runtime | 17–20 | Real backend HTML response modified by rules authored in dashboard |

## Conventions for Every Task

- **Branch:** `task/{NN}-{slug}` off `main`
- **Commits:** Conventional Commits (`feat:`, `fix:`, `chore:`, `test:`, `docs:`)
- **PR template:** Goal · Changes · How to verify · Screenshots / curl output
- **Definition of Done:**
  1. Acceptance criteria pass
  2. New code has unit tests; total coverage ≥ 80% on touched modules
  3. `cargo fmt --check`, `cargo clippy -- -D warnings`, `eslint`, `tsc --noEmit` clean
  4. Manual smoke per task's "Verify" section
  5. README updated when developer setup changes

## Out of Scope for This Plan

Deferred to post-MVP (per §9): Analytics, Access Permissions, Split Tests, Sub Rules, Rule Templates with multi-output, JSON feature type, Component Registry UI, Gift/Campaign Tokens, Custom Segments, Webhook integrations.
