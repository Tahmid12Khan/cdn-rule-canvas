# ===================== RRE CONTRACT (single source of truth) =====================

> AUTHORITATIVE. Every implementation agent (backend, frontend, proxy) follows this verbatim.
> If a task `.md` and this document disagree on a name/type/route/shape, THIS document wins.
> Verified against zen source on 2026-05-31.

This file is the persisted canonical contract for the Response Rule Engine (RRE). It contains three
sections — BACKEND, FRONTEND, PROXY — plus the shared zen-engine integration facts and buildability
rules. The proxy scaffold mirrors the PROXY section verbatim; where a shared name/type/route/shape
appears (rule_graph JSON, `ActiveVersionRead`, zen-engine integration), the BACKEND section is the
source of truth and the PROXY section mirrors it.

---

## BACKEND (canonical — source of truth)

### 0. Global Conventions

- Crate `rre-backend`, standalone (`Cargo.toml` MUST contain an empty `[workspace]` table, edition 2021).
- SQLx: RUNTIME queries ONLY — `sqlx::query_as::<_, T>(...)`, `sqlx::query(...)`, `.bind(...)`. NEVER
  `query!`/`query_as!` macros. Migrations run at startup via `sqlx::migrate!("./migrations")`.
- All tables live in Postgres schema `rre`; all refs schema-qualified (`rre.features`). Never rely on
  `search_path`.
- Timestamps: `TIMESTAMPTZ` ⇄ `chrono::DateTime<Utc>`, default `now()`. UUIDs `uuid::Uuid` generated
  app-side with `Uuid::new_v4()` (DB default `gen_random_uuid()` as a safety net).
- Config: a single `Settings` struct via `envy::from_env::<Settings>()` + `dotenvy`. No scattered
  `std::env::var` in app code.
- Layering: `api/v1` (routers, `State<AppState>`) → `services` (business logic, tx boundary, domain
  errors) → `repositories` (SQLx, returns models) → `models` (`FromRow`). `schemas` holds serde DTOs.
  Routers never serialize `FromRow` structs — always map to a `*Read` DTO. Services never write SQL
  outside repositories.
- OpenAPI via `utoipa` + `utoipa-swagger-ui` at `/docs`.
- Pagination: `?page=<1-based>&page_size=<n>`; `page_size` capped at 100, default 20; envelope
  `{items, page, page_size, total}`.

### 1. Error Envelope (uniform, ALL endpoints)

```json
{ "error": { "code": "VERSION_NOT_FOUND", "message": "Version 4 not found for feature 'dn-article'",
  "details": [ { "loc": "rule_graph.anonymous.edges[1]", "msg": "edge target 'n9' not found in nodes", "rule_id": "edge_endpoint_exists" } ] } }
```

- `error.code`: SCREAMING_SNAKE_CASE stable string. `error.details` present ONLY for 422; each item
  `{ loc, msg, rule_id }`; omitted/null otherwise.

| `code` | HTTP | Domain trigger |
|---|---|---|
| `VALIDATION_ERROR` | 422 | request DTO / `validator` failure, or rule_graph validation |
| `FEATURE_NOT_FOUND` | 404 | feature slug missing |
| `VERSION_NOT_FOUND` | 404 | version (by number or id) missing |
| `OUTCOME_NOT_FOUND` | 404 | outcome id missing |
| `COMPONENT_NOT_FOUND` | 404 | component id missing |
| `NO_LIVE_VERSION` | 404 | active-version requested, none LIVE/STAGING |
| `SLUG_CONFLICT` | 409 | duplicate feature slug |
| `VERSION_EDIT_LOCKED` | 409 | mutate rule_graph/outcomes/components on non-DRAFT version |
| `INVALID_STATUS_TRANSITION` | 409 | illegal publish/unpublish/delete transition |
| `BUILTIN_OUTCOME_PROTECTED` | 409 | delete builtin ShowContent outcome |
| `INTERNAL_ERROR` | 500 | unexpected (raw `sqlx::Error`, panic-guard). Never leak DB text. |

`error::AppError` (`thiserror`) implements `axum::response::IntoResponse`. `From<sqlx::Error>` →
`Internal`; unique violation (`23505`) on features slug → `SLUG_CONFLICT` (inspected in
`feature_service`, not in `From`). `RowNotFound` is NOT used for 404 (services check explicitly).

```rust
#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("validation error")] Validation { details: Vec<ValidationDetail> },
    #[error("{0}")] FeatureNotFound(String),
    #[error("{0}")] VersionNotFound(String),
    #[error("{0}")] OutcomeNotFound(String),
    #[error("{0}")] ComponentNotFound(String),
    #[error("{0}")] NoLiveVersion(String),
    #[error("{0}")] SlugConflict(String),
    #[error("{0}")] VersionEditLocked(String),
    #[error("{0}")] InvalidStatusTransition(String),
    #[error("{0}")] BuiltinOutcomeProtected(String),
    #[error(transparent)] Internal(#[from] anyhow::Error),
}
pub struct ValidationDetail { pub loc: String, pub msg: String, pub rule_id: String }
```

### 2. Postgres Enums (Rust ⇄ DB, `src/models/enums.rs`)

`sqlx::Type` + `serde` + `utoipa::ToSchema`. DB string forms lowercase/snake; API JSON uses the DB form.

| Rust enum | DB type | Variants (Rust → DB/JSON) |
|---|---|---|
| `FeatureType` | `rre.feature_type` | `Html`→`html`, `Json`→`json` |
| `VersionStatus` | `rre.version_status` | `Draft`→`draft`, `Staging`→`staging`, `Live`→`live`, `Prev`→`prev` |
| `Placement` | `rre.placement` | `Inline`→`inline`, `StickyFooter`→`sticky_footer`, `Popup`→`popup` |

`FeatureType` uses `rename_all="lowercase"`; `VersionStatus`/`Placement` use `rename_all="snake_case"`
(so `sticky_footer` matches) for BOTH sqlx and serde. Request-only enum `PublishEnvironment`
(`schemas/version.rs`, NOT a DB type): `Staging`→`staging`, `Live`→`live` (`rename_all="lowercase"`).

### 3. Database Schema (migrations, paired `.up.sql`/`.down.sql`)

- `0001_baseline` (SCAFFOLD): `CREATE SCHEMA rre`; `CREATE EXTENSION pgcrypto`; three enum types
  (`feature_type`, `version_status`, `placement`). Down: `DROP SCHEMA rre CASCADE`.
- `0002_features`: `rre.features (id VARCHAR(64) PK /*slug*/, name VARCHAR(200), type rre.feature_type,
  staging_version_id UUID NULL, live_version_id UUID NULL, created_at, updated_at)`. No FK to versions
  yet (versions table absent — wired in 0003).
- `0003_versions`: `rre.versions (id UUID PK DEFAULT gen_random_uuid(), feature_id VARCHAR(64) REFERENCES
  rre.features(id) ON DELETE CASCADE, version_number INTEGER, description TEXT?, status rre.version_status
  DEFAULT 'draft', rule_graph JSONB DEFAULT three-empty-canvas, created_by, last_updated_by,
  last_updated_at, created_at, UNIQUE(feature_id, version_number))`; INDEX `(feature_id, status)`. Then
  ALTER features ADD the two deferrable FKs (`staging_version_id`, `live_version_id`) →
  `rre.versions(id) ON DELETE SET NULL DEFERRABLE INITIALLY DEFERRED`. Partial unique indexes:
  `versions_one_live_per_feature (feature_id) WHERE status='live'`,
  `versions_one_staging_per_feature (feature_id) WHERE status='staging'`.
- `0004_outcomes_components`: `rre.outcomes (id UUID PK, version_id UUID REFERENCES rre.versions(id) ON
  DELETE CASCADE, title VARCHAR(100), description VARCHAR(500)?, is_builtin BOOL DEFAULT false,
  order_index INTEGER DEFAULT 0, created_at, updated_at)`, INDEX `(version_id, order_index)`.
  `rre.components (id UUID PK, outcome_id UUID REFERENCES rre.outcomes(id) ON DELETE CASCADE, slug
  VARCHAR(120), type VARCHAR(60), config JSONB DEFAULT '{}', placement rre.placement DEFAULT 'inline',
  order_index INTEGER DEFAULT 0, created_at, updated_at)`, INDEX `(outcome_id, order_index)`.
- `0005_component_config`: `ALTER TABLE rre.components ADD CONSTRAINT components_type_known CHECK (type IN
  ('html_injection','content_truncation'))`.

Migration order is load-bearing (0002 features WITHOUT versions FK; 0003 ALTERs in the deferrable FK
after creating versions). Never reorder.

### 4. Domain Models (`FromRow`, `src/models/`)

```rust
pub struct Feature { pub id: String, pub name: String, pub r#type: FeatureType,
  pub staging_version_id: Option<Uuid>, pub live_version_id: Option<Uuid>,
  pub created_at: DateTime<Utc>, pub updated_at: DateTime<Utc> }
pub struct Version { pub id: Uuid, pub feature_id: String, pub version_number: i32,
  pub description: Option<String>, pub status: VersionStatus, pub rule_graph: serde_json::Value,
  pub created_by: String, pub last_updated_by: String,
  pub last_updated_at: DateTime<Utc>, pub created_at: DateTime<Utc> }
pub struct Outcome { pub id: Uuid, pub version_id: Uuid, pub title: String,
  pub description: Option<String>, pub is_builtin: bool, pub order_index: i32,
  pub created_at: DateTime<Utc>, pub updated_at: DateTime<Utc> }
pub struct Component { pub id: Uuid, pub outcome_id: Uuid, pub slug: String, pub r#type: String,
  pub config: serde_json::Value, pub placement: Placement, pub order_index: i32,
  pub created_at: DateTime<Utc>, pub updated_at: DateTime<Utc> }
```

`version_number` and all `order_index` are `i32`. Never serialize `FromRow` structs on the boundary.

### 5. DTO Schemas (`src/schemas/`)

- Pagination (SCAFFOLD): `PageParams { page: Option<u32>, page_size: Option<u32> }` with
  `resolve() -> (limit i64, offset i64, page u32, page_size u32)` (default 20, cap 100);
  `Page<T> { items: Vec<T>, page: u32, page_size: u32, total: i64 }`.
- Feature: `FeatureCreate { id (SLUG_RE, 3..=64), name (1..=200), r#type }`, `FeatureUpdate { name? }`,
  `FeatureRead { id, name, r#type, staging_version_id?, live_version_id?, created_at, updated_at }`.
  `SLUG_RE = ^[a-z0-9]+(?:-[a-z0-9]+)*$`.
- Version: `VersionCreate { description? (<=2000) }`, `VersionUpdate { description? (<=2000),
  rule_graph?: RuleGraph }`, `VersionRead { id, feature_id, version_number, description?, status,
  rule_graph: RuleGraph, created_by, last_updated_by, last_updated_at, created_at }`,
  `VersionSummary` (same minus rule_graph + created_by), `PublishRequest { environment:
  PublishEnvironment }`, `VersionListQuery { status?: VersionStatus, search?: String, #[flatten] page }`.
- Outcome: `OutcomeCreate { title (1..=100), description? (<=500) }`, `OutcomeUpdate { title?,
  description?, order_index? }`, `OutcomeRead { id, version_id, title, description?, is_builtin,
  order_index, components: Vec<ComponentRead>, created_at, updated_at }`, `ReorderItem { id: Uuid,
  order_index: i32 }` (reorder body is `Vec<ReorderItem>`).
- Component: `ComponentCreate { slug (1..=120), r#type, config: ComponentConfig, placement,
  order_index? }`, `ComponentUpdate { slug?, r#type?, config?: ComponentConfig, placement?,
  order_index? }`, `ComponentRead { id, outcome_id, slug, r#type, config: serde_json::Value,
  placement, order_index, created_at, updated_at }`.
- `ComponentConfig` (`#[serde(tag="type", rename_all="snake_case")]`): `HtmlInjection { target_selector,
  placement_mode: HtmlPlacementMode, html_body, theme? }` (`html_injection`); `ContentTruncation {
  target_selector, word_count: u32 (1..=10000), fade_out: bool }` (`content_truncation`).
  `HtmlPlacementMode (snake_case) = replace|append|prepend|before|after`. The persisted
  `components.config` MUST include `type` equal to the row's `type` column; service deserializes into
  `ComponentConfig` before persist; mismatch → `VALIDATION_ERROR`.

### ActiveVersionRead (§5 — proxy-facing, EXACT)

```rust
pub struct ActiveVersionRead { pub version_number: i32, pub rule_graph: RuleGraph, pub outcomes: Vec<ActiveOutcome> }
pub struct ActiveOutcome { pub id: Uuid, pub title: String, pub is_builtin: bool, pub order_index: i32, pub components: Vec<ActiveComponent> }
pub struct ActiveComponent { pub id: Uuid, pub slug: String, pub r#type: String, pub config: serde_json::Value, pub placement: Placement, pub order_index: i32 }
```

`outcomes` and `outcomes[*].components` are ordered by `order_index ASC`. Derives `Serialize +
Deserialize + ToSchema` so the proxy can mirror/reuse it.

### rule_graph JSON schema (§6 — canonical, STABLE)

```rust
pub struct RuleGraph { pub anonymous: CanvasGraph, pub registered: CanvasGraph, pub customer: CanvasGraph }
pub struct CanvasGraph { pub nodes: Vec<Node>, pub edges: Vec<Edge>, #[serde(default)] pub root_node_id: Option<String> }

#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Node {
    Decision { id: String, processor: ProcessorConfig, position: Position },   // kind = "decision"
    Outcome  { id: String, outcome_id: Uuid, position: Position },             // kind = "outcome"
}

#[serde(tag = "type", rename_all = "snake_case")]
pub enum ProcessorConfig {
    MetaTags  { tag_name: String, operator: MetaTagsOperator, #[serde(default)] value: Option<String> }, // "meta_tags"
    DeviceType { operator: DeviceOperator, value: DeviceValue },                                          // "device_type"
    ArticleUrl { operator: ArticleUrlOperator, value: String },                                           // "article_url"
}

#[serde(rename_all = "snake_case")] pub enum MetaTagsOperator { Contains, Equals, Exists }
#[serde(rename_all = "snake_case")] pub enum DeviceOperator { Equals, Contains }
#[serde(rename_all = "snake_case")] pub enum DeviceValue { Mobile, Desktop, Tablet }
#[serde(rename_all = "snake_case")] pub enum ArticleUrlOperator { Contains, Matches, StartsWith, Equals }

pub struct Edge { pub id: String, pub source_node_id: String, pub target_node_id: String, pub branch: Branch }
#[serde(rename_all = "snake_case")] pub enum Branch { Yes, No }
pub struct Position { pub x: f64, pub y: f64 }
```

Processor-key mapping (snake `type` -> camel JDM kind): `meta_tags` -> `"metaTags"`,
`device_type` -> `"deviceType"`, `article_url` -> `"articleUrl"`. The camelCase keys
equal `CanvasProcessor::kind()`. The `article_url` processor matches `operator`/`value`
against the request path (`request_path`); `matches` is a regex.

#### rule_graph validation rules (`rule_graph_service::validate(version_id, &RuleGraph)`)

Run per-canvas (anonymous/registered/customer). Each failure emits `ValidationDetail { loc, msg,
rule_id }`; `loc` format `rule_graph.<canvas>.<field>[idx]`. Invalid → 422 `VALIDATION_ERROR`.
Stable `rule_id` values:

| `rule_id` | Rule |
|---|---|
| `edge_endpoint_exists` | every `source_node_id`/`target_node_id` exists in `nodes` |
| `branch_unique` | a Decision node has at most ONE outgoing edge per `branch` |
| `no_cycles` | DFS detects no cycle |
| `outcome_terminal` | Outcome nodes have ZERO outgoing edges |
| `outcome_ref_exists` | every Outcome node's `outcome_id` exists in `rre.outcomes` for this version |
| `root_in_nodes` | if `root_node_id` set, it exists in `nodes` |
| `outcome_branch_forbidden` | edges may only originate from Decision nodes |

### 7. REST Routes (router tree → handler → service → repo)

Base prefix `/api/v1`; built in `lib.rs::build_app(state)`. CORS allows `settings.frontend_origin`.
`/health`, `/healthz/db`, `/docs` live at the root.

```
# Features
POST   /api/v1/features                       -> features::create        -> feature_service::create
GET    /api/v1/features                       -> features::list          -> feature_service::list
GET    /api/v1/features/{fid}                 -> features::get           -> feature_service::get
PATCH  /api/v1/features/{fid}                 -> features::update        -> feature_service::update
DELETE /api/v1/features/{fid}                 -> features::delete        -> feature_service::delete
GET    /api/v1/features/{fid}/active-version  -> features::active_version-> version_service::active_version

# Versions (nested under feature; {vnum} = version_number i32)
POST   /api/v1/features/{fid}/versions                -> versions::create    -> version_service::create_version
GET    /api/v1/features/{fid}/versions                -> versions::list      -> version_service::list
GET    /api/v1/features/{fid}/versions/{vnum}         -> versions::get       -> version_service::get
PATCH  /api/v1/features/{fid}/versions/{vnum}         -> versions::update    -> version_service::update
POST   /api/v1/features/{fid}/versions/{vnum}/publish -> versions::publish   -> version_service::publish
POST   /api/v1/features/{fid}/versions/{vnum}/unpublish-> versions::unpublish-> version_service::unpublish
DELETE /api/v1/features/{fid}/versions/{vnum}         -> versions::delete    -> version_service::delete

# Outcomes (nested + flat; {vid}=version UUID, {oid}/{cid}=UUID)
GET    /api/v1/versions/{vid}/outcomes  -> outcomes::list_for_version -> outcome_service::list
POST   /api/v1/versions/{vid}/outcomes  -> outcomes::create           -> outcome_service::create
GET    /api/v1/outcomes/{oid}           -> outcomes::get              -> outcome_service::get_with_components
PATCH  /api/v1/outcomes/{oid}           -> outcomes::update           -> outcome_service::update
DELETE /api/v1/outcomes/{oid}           -> outcomes::delete           -> outcome_service::delete
POST   /api/v1/outcomes/{oid}/clone     -> outcomes::clone            -> outcome_service::clone_outcome
POST   /api/v1/outcomes/{oid}/reorder   -> outcomes::reorder_components-> outcome_service::reorder
POST   /api/v1/outcomes/{oid}/components-> outcomes::add_component     -> outcome_service::add_component
PATCH  /api/v1/components/{cid}         -> components::update         -> outcome_service::update_component
DELETE /api/v1/components/{cid}         -> components::delete         -> outcome_service::delete_component
```

Canonical responses: POST/clone/add → 201; DELETE → 204; others → 200. `POST /features` dup slug → 409
`SLUG_CONFLICT`. `GET active-version?env=live|staging` (default live) → `ActiveVersionRead` / 404
`NO_LIVE_VERSION`. `POST versions` clones rule_graph from current LIVE (else empty 3-canvas) and seeds
builtin `ShowContent` outcome. `PATCH version` with rule_graph on non-DRAFT → 409 `VERSION_EDIT_LOCKED`;
invalid graph → 422. DELETE version on LIVE/STAGING → 409 `INVALID_STATUS_TRANSITION` (only DRAFT/PREV
deletable). Outcome create/delete on non-DRAFT version → 409 `VERSION_EDIT_LOCKED`; delete builtin → 409
`BUILTIN_OUTCOME_PROTECTED`. Clone → deep copy incl. components, new UUIDs, `is_builtin=false`, title
`"{title} (copy)"`.

#### Status lifecycle (service-enforced)

- publish `live`: target must be DRAFT|STAGING|PREV. Current LIVE (other row) → PREV. Target → LIVE.
  `features.live_version_id = target.id`.
- publish `staging`: target → STAGING. Previous STAGING (other row) → PREV UNLESS it's also LIVE (keeps
  LIVE). `features.staging_version_id = target.id`.
- unpublish `live`: target must be LIVE → PREV; `features.live_version_id = NULL`.
- unpublish `staging`: target must be STAGING → PREV (or LIVE if also live row);
  `features.staging_version_id = NULL`.
- Any transition not listed → 409 `INVALID_STATUS_TRANSITION`.

### 10. Transaction Boundaries

Service layer owns `pool.begin()` / `tx.commit()`; one unit of work per method. Read paths use
`&PgPool`. `create_version`: TX with `SELECT MAX(version_number) ... FOR UPDATE` → INSERT version →
INSERT builtin ShowContent outcome → commit. `publish/unpublish`: TX with `SELECT ... FOR UPDATE` →
demotions → UPDATE target → UPDATE `features.*_version_id` → commit (deferrable FK + partial unique
indexes satisfied at commit). `active_version`: read, no tx, batch components by `outcome_id = ANY($1)`
to avoid N+1. `clone_outcome`/`reorder`: TX.

### Backend File Ownership Map

SCAFFOLD-owned (shared infra; one agent writes, everyone imports): `Cargo.toml`, `.env.example`,
`rustfmt.toml`, `clippy.toml`, `src/main.rs`, `src/lib.rs`, `src/config.rs`, `src/db.rs`, `src/state.rs`,
`src/error.rs`, `src/telemetry.rs`, `src/openapi.rs`, every `mod.rs`, `src/models/enums.rs`,
`src/schemas/health.rs`, `src/schemas/pagination.rs`, `src/api/v1/health.rs`, `migrations/0001_baseline.*`,
`tests/common/mod.rs`, `tests/{health,cors,db_health,migrations}.rs`.

DOMAIN-owned (leaf files, filled by domain agents): `src/api/v1/{features,versions,outcomes,components}.rs`,
`src/models/{feature,version,outcome,component}.rs`,
`src/schemas/{feature,version,outcome,component,rule_graph,active_version}.rs`,
`src/repositories/{feature,version,outcome,component}_repository.rs`,
`src/services/{feature,version,outcome,rule_graph}_service.rs`, `src/bin/{seed_demo,export_schema}.rs`,
`migrations/000{2,3,4,5}_*.{up,down}.sql`, the remaining `tests/*.rs`. Each `api/v1/*` handler module
exposes `pub fn router() -> Router<AppState>` merged in `api/v1/mod.rs`.

### ActiveVersionRead (§5 — proxy-facing, EXACT)

```rust
pub struct ActiveVersionRead { pub version_number: i32, pub rule_graph: RuleGraph, pub outcomes: Vec<ActiveOutcome> }
pub struct ActiveOutcome { pub id: Uuid, pub title: String, pub is_builtin: bool, pub order_index: i32, pub components: Vec<ActiveComponent> }
pub struct ActiveComponent { pub id: Uuid, pub slug: String, pub r#type: String, pub config: serde_json::Value, pub placement: Placement, pub order_index: i32 }
```

`outcomes` and `outcomes[*].components` are ordered by `order_index ASC`.

---

## PROXY (canonical — source of truth for Tasks 17/18/19/20 proxy side)

Status: AUTHORITATIVE for the `proxy/` crate. Subordinate to BACKEND where shared.
The proxy is the production hot path. Latency overhead and correctness errors here are user-visible.

### 0. Global Conventions

- Crate `rre-proxy`. Standalone (empty `[workspace]`, edition 2021). zen deps via PATH:
  `zen-engine = { path = "../core/engine" }`, `zen-expression = { path = "../core/expression" }`.
  zen-types is re-exported via `zen_engine::model::*` — do NOT add a direct `zen-types` path dep.
- Bind addr `0.0.0.0:9000` (`PROXY_BIND_ADDR`).
- Config: single `Settings` via `envy::from_env` + `dotenvy`. No scattered `std::env::var`.
- Logging: `tracing` + json layer (pretty in dev). NEVER log raw bodies / cookie values.
- Layering: `middleware` -> `forwarder` (the only network IO + pipeline glue) -> `domain`
  (pure eval + pure transform) -> `infra` (feature_map, backend_client, caches, encoding).
- Async-runtime rule (HARD): NO sync IO in the request path. zen evaluation runs in
  `tokio::task::spawn_blocking` on a `Builder::new_current_thread().enable_all().build()` runtime
  (zen `Variable` is `!Send`). `scraper::Html` is `!Send`, reconstructed INSIDE the closure from a
  `Send` HTML `String`. Only `serde_json::Value` / `String` cross the boundary.
- Errors: typed `ProxyError` (thiserror) implements `IntoResponse`. All failures map to a typed
  response or fail-open — NEVER panic into the client.

### 1. Sequence

```
Client -> [axum fallback] -> forwarder::forward
  middleware/trace: ensure X-RRE-Trace-Id (Uuid v4), bind span, echo on response
  1. feature_map.resolve(host, path) -> Option<feature_id>     miss -> PASS-THROUGH (skipped)
  2. classifier::classify(&headers) -> Canvas                  cookie rre_user_type, default anonymous
  3. backend.active_version(feature_id, Live) -> Arc<ActiveVersionRead>  none/err -> PASS-THROUGH (fail-open)
  4. send_upstream(reqwest) -> upstream Response               err/timeout -> typed 502/504, never panic
  5. content-type gate: text/html -> modify; else pass-through
  6. EvaluationContextParts::from_request(headers, path, cookies, html_string)   html kept as Send String
  7. GraphEvaluator::evaluate(canvas_graph, ctx, feature_id, version_number, canvas)
       - compiled_cache.get_or_compile((feature_id, version_number, canvas)) -> Arc<DecisionContent> (<=256)
       - spawn_blocking + current_thread rt: parse scraper::Html; adapter = CanvasNodeAdapter{registry, ctx};
         engine = DecisionEngine::default().with_adapter(...); decision = engine.create_decision(content);
         resp = decision.evaluate_with_opts(Variable::from(ctx_json), EvaluationOptions{trace:false,max_depth:10}).await;
         serialize resp.result -> Value, extract {outcomeId} -> Option<Uuid>
       None -> serve upstream untouched (skipped)
  8. resolve outcome: av.find_outcome(id); builtin ShowContent -> no-op (skipped)
  9. applier::orchestrator::apply_outcome(html, &outcome, &sanitizer) -> ModificationResult
       components order_index ASC; inline first, then placement; per-component fail-open; any error -> original
 10. encoding::reencode(body, content_encoding)   gzip via flate2; br/deflate -> pass-through + warn
 11. rebuild Response: modified body, recalc Content-Length, X-RRE-Apply-Status, X-RRE-Trace-Id
```

### 2. Hot-Path Budget (p99)

| Stage | p99 target |
|---|---|
| feature_map resolve | < 50us |
| classify | < 10us |
| active-version (cache hit) | < 50us |
| translate + compile (cache hit) | < 20us |
| eval (zen, spawn_blocking) | **< 5ms** |
| transform (applier, lol_html) | **< 20ms** |
| re-encode (gzip) | < 5ms |
| total proxy overhead | **< 30ms p99** |

### 3. Failure Modes

Invariant: a matched-feature request can ALWAYS degrade to "serve upstream untouched". The applier is
wrapped so any error returns the original `html` and sets `X-RRE-Apply-Status: error`.

| Failure | Behavior | Header |
|---|---|---|
| Feature not mapped | pass-through | skipped |
| Upstream connect timeout (2s) | `ProxyError::UpstreamTimeout` | 504 |
| Upstream read timeout (10s) | `ProxyError::UpstreamTimeout` | 504 |
| Upstream refused / DNS | `ProxyError::UpstreamUnavailable` | 502 |
| Backend active-version fails / NO_LIVE_VERSION | fail-open (pass-through) | skipped |
| Non-HTML content-type | pass-through | skipped |
| Eval cycle/runaway depth | zen `max_depth:10` bounds it | fail-open |
| Eval no `outcomeId` | no modification | skipped |
| Unknown processor kind | `NodeError` -> fail-open | warn |
| Processor bad/missing config | `ProcessorError::Config` -> `NodeError` -> fail-open (never default a branch) | warn |
| Selector miss (MetaTags) | fail-open `Branch::No`, WARN | — |
| Selector miss (applier) | component skipped, others continue | ok |
| Oversized/illegal selector | reject before interpolation | warn |
| Malformed upstream HTML | catch, return ORIGINAL | error |
| Outcome id not in payload | no modification | skipped |
| Unsupported encoding (br/deflate) | pass-through unchanged | skipped |
| spawn_blocking task panic | caught -> fail-open | error |

### 4. State Plan

| Cache | Type | Key | Value | Bound |
|---|---|---|---|---|
| active-version | `moka::future::Cache` | `(feature_id, Env)` | `Arc<ActiveVersionRead>` | TTL 30s |
| compiled graph | `moka::sync::Cache` | `(feature_id, version_number, Canvas)` | `Arc<DecisionContent>` | <=256 LRU |
| reqwest pool | `reqwest::Client` | — | — | process |
| ProcessorRegistry | `Arc<ProcessorRegistry>` | — | immutable | process |

Canvas isolation (HARD): the compiled-cache key includes `Canvas`; the classifier picks exactly one
canvas; no code path lets one class's request reach another class's graph.

### 5. Risk Callouts

- Blocking the runtime: zen `Variable` + `scraper::Html` are `!Send`; everything runs in
  `spawn_blocking` + `new_current_thread` rt. Never `.await` `decision.evaluate` on the multi-thread rt.
- Memory: compiled cache <=256 LRU; HTML buffered only for text/html under 1 MB (larger passed through).
- Canvas isolation (HARD): anonymous request evaluates only `rule_graph.anonymous`.
- Selector injection: `tag_name` (MetaTags) and `target_selector` (applier) length-capped (<=200) +
  char-whitelisted (`[A-Za-z0-9_\-\[\]="' .#:]`) before interpolation; reject -> typed error / skip.
- Untrusted upstream HTML: parse defensively; malformed -> serve original. Component `html_body`
  sanitized via `ammonia` (allow-list from `proxy/config/sanitizer.yaml`).
- PII: never log bodies / cookie values / full header maps.
- Idempotent transforms (HARD): every renderer idempotent; placement wrappers + fade `<style>` injected
  only if absent (marker guards). Second pass is a no-op.
- Eval purity (HARD): `GraphEvaluator::evaluate` + every `CanvasProcessor` are pure functions of
  `(graph, ctx)`; registry read-only after startup.

### 6. AppState + Settings

```rust
#[derive(Clone)]
pub struct AppState {
    pub settings: Arc<Settings>,
    pub http: reqwest::Client,
    pub feature_map: Arc<FeatureMap>,
    pub backend: Arc<BackendClient>,
    pub compiled: Arc<CompiledCache>,
    pub registry: Arc<ProcessorRegistry>,
    pub sanitizer: Arc<ammonia::Builder<'static>>,
}
```

`Settings`: `proxy_bind_addr` (default `0.0.0.0:9000`), `upstream_base_url`
(default `http://demo-upstream:8081`), `backend_base_url` (default `http://backend:8000`), `app_env`,
`active_version_ttl_secs` (default 30), `compiled_cache_capacity` (default 256),
`upstream_connect_timeout_secs` (default 2), `upstream_read_timeout_secs` (default 10),
`feature_map_path` (default `proxy/config/feature_map.yaml`),
`sanitizer_config_path` (default `proxy/config/sanitizer.yaml`).

### 7. Module Paths + Signatures (key)

- `forwarder::forward(State<AppState>, Request) -> Response`; `strip_hop_by_hop(&mut HeaderMap)`;
  `send_upstream(&AppState, Request) -> Result<UpstreamResponse, ProxyError>`.
- `middleware::trace::layer()`; `TRACE_HEADER = "x-rre-trace-id"`.
- `infra::feature_map::FeatureMap::{load, resolve}`.
- `infra::backend_client::{BackendClient, Env, ActiveVersionRead, ActiveOutcome, ActiveComponent, Placement}`.
- `infra::compiled_cache::CompiledCache::{new, get_or_compile}`.
- `infra::encoding::{gunzip, gzip, decode_for_modify, reencode}`.
- `domain::graph` — verbatim rule_graph mirror + `Canvas` + `ProcessorConfig::{kind_key, to_config_value}`.
- `domain::translator::to_decision_content(&CanvasGraph) -> DecisionContent`.
- `domain::processors` — `CanvasProcessor` trait, `ProcessorRegistry`, `ProcessorOutcome`, `Branch`,
  `ProcessorError`, `default_registry()`.
- `domain::processors::{meta_tags::MetaTagsProcessor, device_type::DeviceTypeProcessor}`.
- `domain::adapter::CanvasNodeAdapter` (impl zen `CustomNodeAdapter`).
- `domain::evaluator::GraphEvaluator::evaluate(...)`.
- `domain::classifier::classify(&HeaderMap) -> Canvas`.
- `domain::context::{EvaluationContext, EvaluationContextParts, DeviceType}`.
- `domain::applier` — `ComponentRenderer` trait, `ModificationResult`, `ApplyError`,
  `orchestrator::apply_outcome`, `html_injection`, `content_truncation`, `placement_sticky_footer`,
  `placement_popup`, `html_sanitizer::{load_sanitizer, sanitize}`.

### 8. zen Integration Invariants (verified)

- JDM types from `zen_engine::model::*` (re-exports `zen_types::decision::*`); engine from
  `zen_engine::{DecisionEngine, EvaluationOptions}`; adapter from
  `zen_engine::nodes::custom::{CustomNodeAdapter, CustomNodeRequest}`; results from
  `zen_engine::nodes::result::{NodeError, NodeResponse, NodeResult}`; `Variable` from
  `zen_expression::variable::Variable`.
- `CustomNodeAdapter: Debug + Send`. `CanvasNodeAdapter` derives `Debug` (all fields `Debug`).
- `decision.evaluate_with_opts(context: Variable, EvaluationOptions { trace, max_depth })`.
- `DecisionGraphResponse { performance, result: Variable, trace }` — read `resp.result`.
- `DecisionContent.nodes: Vec<Arc<DecisionNode>>`, `edges: Vec<Arc<DecisionEdge>>`; `.compile(&mut self)`.
- Branch routing: SwitchNode statement ids (`"<id>:yes"`/`"<id>:no"`) MUST equal the edge
  `source_handle`; the walker drops edges whose handle is not a valid statement id.

#### 8.2 Translator mapping (CanvasGraph -> DecisionContent)

- Decision `<id>` -> CustomNode `<id>__proc` (`kind=cfg.kind_key()`, `config=cfg.to_config_value()`) +
  SwitchNode `<id>__switch` (statements `<id>:yes` / `<id>:no`, conditions `$.branch == 'yes'|'no'`) +
  internal edge `<id>__proc -> <id>__switch`.
- Outcome `<id>` -> ExpressionNode `<id>__expr` (expr key `outcomeId` value `'<uuid>'`) +
  OutputNode `<id>__out` + internal edge `<id>__expr -> <id>__out`.
- Canvas edge -> DecisionEdge from `<src>__switch` (`source_handle = "<src>:yes"|"<src>:no"`) to the
  target entry (`__proc` for decision, `__expr` for outcome).
- One InputNode `input` -> edge to the root node entry.
- Round-trip: `from_value::<DecisionContent>(to_value(&dc))` must succeed.

### 9. Backend Dependency

Proxy calls `GET {backend_base_url}/api/v1/features/{id}/active-version?env=live` and deserializes the
exact `ActiveVersionRead` shape. 404 / NO_LIVE_VERSION / any error -> fail open (pass-through).
`Placement` mirrored as `#[serde(rename_all="snake_case")] { Inline, StickyFooter, Popup }`.

### 10. Observability

Per-request JSON log (NEVER bodies/cookies): `trace_id`, `feature_id`, `canvas`, `outcome_id` (or `none`),
`eval_ms`, `transform_ms`, `apply_status`. Metrics (`metrics` + `metrics-exporter-prometheus`, `/metrics`):
histograms `proxy_eval_ms` / `proxy_transform_ms` / `proxy_e2e_ms`; counters `proxy_requests_total{feature}`,
`proxy_outcomes_total{outcome_id}`, `proxy_apply_errors_total`.

---

## FRONTEND (summary)

Next.js App Router + TypeScript strict + TailwindCSS. Mirrors BACKEND on wire shapes. Server Components by
default; client only for interactivity. TanStack Query owns server state; Zustand `ruleBuilderStore` owns
canvas working state only. Single `NEXT_PUBLIC_API_BASE` (default `http://localhost:8000`). The full
canonical FRONTEND contract (route map, component inventory, React Flow node/edge model,
serialize/deserialize invariants, zod API clients, design tokens, a11y, risks) is maintained alongside this
file by the frontend scaffold; the proxy does not consume frontend types.

---

## ZEN-ENGINE INTEGRATION FACTS (verified)

- Depend via PATH (playground/Cargo.toml template): `zen-engine = { path = "../core/engine" }`,
  `zen-expression = { path = "../core/expression" }`.
- Evaluate:
  ```rust
  let engine = zen_engine::DecisionEngine::default().with_adapter(Arc::new(my_adapter));
  let decision = engine.create_decision(Arc::new(decision_content));
  let out = decision.evaluate_with_opts(Variable::from(input_value), EvaluationOptions{trace:false,max_depth:10}).await?;
  ```
- Pluggable node = `CustomNodeAdapter`:
  ```rust
  pub trait CustomNodeAdapter: Debug + Send {
      fn handle(&self, request: CustomNodeRequest) -> Pin<Box<dyn Future<Output = NodeResult> + '_>>;
  }
  CustomNodeRequest { input: Variable, node: CustomDecisionNode { id: Arc<str>, name: Arc<str>, kind: Arc<str>, config: Arc<Value> } }
  NodeResult = Result<NodeResponse, NodeError>;  NodeResponse { output: Variable, trace_data: Option<Variable> }
  NodeError { node_id: Arc<str>, trace: Option<Variable>, source: Box<dyn std::error::Error> }
  ```
- JDM model: `DecisionContent { nodes: Vec<Arc<DecisionNode>>, edges: Vec<Arc<DecisionEdge>>, compiled_cache }`.
  `DecisionNode { id, name, kind: DecisionNodeKind }`. `DecisionNodeKind` includes `InputNode`,
  `OutputNode`, `SwitchNode`, `ExpressionNode`, `CustomNode { content: CustomNodeContent { kind, config } }`.

---

## BUILDABILITY RULES

- The whole tree is generated in parallel and compiled ONCE at the very end. Forward references in
  `mod.rs` / imports are EXPECTED.
- `backend/` and `proxy/` are each a STANDALONE crate: empty `[workspace]` table, edition 2021,
  depend on zen via relative path.
- SQLx (backend): RUNTIME queries ONLY — never the `query!`/`query_as!` macros. Migrations via
  `sqlx::migrate!("./migrations")` at startup.
- Frontend: Next.js App Router + TS strict + Tailwind; only declared npm deps.
