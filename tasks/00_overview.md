# Task Plan — Response Rule Engine (RRE)

Sequential build plan derived from `features.md`. Each task is independently mergeable, has explicit acceptance criteria, and ends with a visible artifact (a running screen, a passing API call, or a working end-to-end flow).

## Tech Stack

- **Frontend:** Node.js (active LTS), a fresh **component-based** Next.js (latest stable, App Router) + React + TailwindCSS app, TypeScript (latest stable), React Flow, Zustand (state), TanStack Query (server state), Vitest + React Testing Library, Playwright (smoke E2E). Note: `playground/static/flow.html` is a **reference-only** monolith — do not reuse or port it; build the UI from fresh, reusable components.
- **Backend:** Axum, Rust (2021 edition), SQLx (async Postgres, compile-time-checked), sqlx migrate, serde + validator, utoipa (OpenAPI/Swagger), PostgreSQL 16, cargo test (`#[tokio::test]`), reqwest
- **Rule Evaluation:** the existing **zen-engine** (`zen_engine::DecisionEngine`) — depend on it by path (`zen-engine = { path = "../core/engine" }`, `zen-expression = { path = "../core/expression" }`). The proxy translates a stored canvas `rule_graph` into a JDM `DecisionContent` and evaluates it via the engine. We do **not** hand-roll a bespoke graph walker.
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
| 3. Rule Builder | 09–14 | Drag-drop canvas with MetaTags/DeviceType nodes (each backed by a `CanvasProcessor`), persistent per canvas |
| 4. Outcome Editor | 15–16 | Edit Outcome page + Component config modal match §4.6/§4.7 |
| 5. Proxy Runtime | 17–20 | Real backend HTML response modified by rules — canvas `rule_graph` compiled to JDM `DecisionContent` and evaluated through `zen_engine::DecisionEngine` |

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

## Cross-cutting Decisions

These apply across every task. They reflect choices the team adopted to stay surgical and avoid reinventing engine internals.

### 1. Rule evaluation runs on the existing zen-engine

Rules are **not** evaluated by a hand-rolled graph walker. The proxy translates a stored canvas `rule_graph` into a JDM `DecisionContent` and evaluates it through `zen_engine::DecisionEngine`.

- Depend by path (see `playground/Cargo.toml` for the working template): `zen-engine = { path = "../core/engine" }`, `zen-expression = { path = "../core/expression" }`. If `zen-types` is needed directly: `zen-types = { path = "../core/types" }`.
- Evaluation flow:
  ```rust
  use std::sync::Arc;
  let engine = zen_engine::DecisionEngine::default().with_adapter(Arc::new(rre_adapter));
  let decision = engine.create_decision(Arc::new(decision_content)); // zen_engine::model::DecisionContent
  let out = decision.evaluate_with_opts(Variable::from(input_value), opts).await?;
  ```
- `DecisionContent { nodes: Vec<Arc<DecisionNode>>, edges: Vec<Arc<DecisionEdge>> }`. The engine walks the graph; our code never re-implements traversal, hit policies, or edge resolution.
- `zen_engine::model::DecisionContent` deserializes directly from JDM JSON; `Variable` (zen) is `!Send`, so keep parse → evaluate → serialize inside the blocking/current-thread boundary as `playground/src/main.rs` already demonstrates.
- **RRE → JDM mapping:** an RRE canvas `DecisionNode` (carrying a processor such as `metaTags` / `deviceType`) maps to a JDM `CustomNode` whose `content.kind` = the processor key. An RRE `OutcomeNode` maps to a node that emits `{ outcomeId }`. YES/NO branch edges map to JDM `SwitchNode`/edges (or a branch key returned by the processor and consumed by edge conditions).

### 2. The frontend is a fresh, component-based Next.js app

The admin dashboard and rule builder are built from scratch as reusable React components with TailwindCSS in the App Router.

- `playground/static/flow.html` is **reference only** — a single-file prototype. Do not import, copy, or port it. Use it to understand intended behavior/layout, then build proper components.
- State lives in Zustand; server state via TanStack Query; the visual canvas uses React Flow with our own node components.

### 3. New canvas node types are added in Rust via a CanvasProcessor trait

Adding a new node type (beyond MetaTags/DeviceType) requires **no engine changes**. It is a Rust-only extension implemented against zen's pluggable custom-node mechanism.

- Define the behavior by implementing a `CanvasProcessor` trait and register it in a `ProcessorRegistry` keyed by processor name.
- The `ProcessorRegistry` is fronted by a single RRE adapter that implements zen's `CustomNodeAdapter` (`core/engine/src/nodes/custom/adapter.rs`):
  ```rust
  pub trait CustomNodeAdapter: Debug + Send {
      fn handle(&self, request: CustomNodeRequest) -> Pin<Box<dyn Future<Output = NodeResult> + '_>>;
  }
  ```
  `CustomNodeRequest { input: Variable, node: CustomDecisionNode { id, name, kind: Arc<str>, config: Arc<Value> } }`. The adapter dispatches on `request.node.kind` → the matching `CanvasProcessor` in the registry.
- Processors read config via `request.get_field("config.<path>")` (renders templates against `input`) and return `NodeResult = Result<NodeResponse, NodeError>` where `NodeResponse { output: Variable, trace_data: Option<Variable> }` (`core/engine/src/nodes/result.rs`).
- Because the engine already invokes the adapter for every JDM `CustomNode`, registering a processor is the only step — no changes to `DecisionEngine`, graph traversal, or the JDM model.

> Agents implementing any of the above MUST first Read the exact signatures in: `core/engine/src/nodes/custom/adapter.rs`, `core/engine/src/nodes/custom/mod.rs`, `core/engine/src/nodes/result.rs`, `core/engine/src/engine.rs`, `core/engine/src/model/decision_content.rs`, `core/types/src/decision/mod.rs`, `playground/Cargo.toml`, `playground/src/main.rs`.

## Out of Scope for This Plan

Deferred to post-MVP (per §9): Analytics, Access Permissions, Split Tests, Sub Rules, Rule Templates with multi-output, JSON feature type, Component Registry UI, Gift/Campaign Tokens, Custom Segments, Webhook integrations.
