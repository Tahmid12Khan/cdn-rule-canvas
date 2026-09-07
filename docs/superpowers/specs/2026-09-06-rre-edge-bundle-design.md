# RRE edge bundle: `rre-core` crate and site export

Date: 2026-09-06. Companion to
`dngroup-fastly/docs/superpowers/specs/2026-09-06-intrafish-rre-edge-rules-design.md`,
which owns the Fastly Compute side. This spec owns everything in this repo.

## Goal

Let a Fastly Compute service evaluate RRE rules and rewrite a response body
in-process, with no call to the RRE backend or proxy at request time. Two
things are needed here:

1. **`rre-core`**, a new crate holding the rule evaluator and body applier that
   the proxy already runs, extracted so it builds for `wasm32-wasip1` and can be
   depended on from another repo. The proxy keeps using it, so the Test panel,
   the proxy and the edge evaluate identically.
2. **Edge bundle export**, a backend endpoint that serialises everything a
   site's live (or staging) rules need into one JSON document, with component
   references resolved inline. For now it is fetched manually by a script in
   the Fastly repo. Pushing it to a Fastly KV Store on publish is a later step
   and is sketched, not built.

## Non-goals

- Automatic publish to Fastly KV. Designed below as "later", not implemented.
- Any change to the rule model, the editor, node types or outcome types.
- Auth on the backend API. The export endpoint is as open as every other
  route; it is read-only and exposes nothing the `active-version` route does
  not already expose.
- A "decision only" HTTP endpoint. Superseded by in-process evaluation.

## Part 1: `rre-core`

### What moves

A new workspace-detached crate at `rre-core/` (same standalone pattern as
`backend/` and `proxy/`: its own `Cargo.toml` with an empty `[workspace]`
table, depending on `../core/engine` and `../core/expression` by path). The
following move out of `proxy/src/` into `rre-core/src/` with their tests:

| From `proxy/src/` | To `rre-core/src/` | Notes |
|---|---|---|
| `domain/graph.rs` | `graph.rs` | unchanged |
| `domain/translator.rs` | `translator.rs` | unchanged |
| `domain/adapter.rs` | `adapter.rs` | unchanged |
| `domain/processors/*` | `processors/*` | unchanged |
| `domain/context.rs` | `context.rs` | verbatim |
| `domain/identity.rs` | `identity.rs` | verbatim |
| `domain/evaluator.rs` | `evaluator.rs` | runtime and cache dependencies removed (below) |
| `domain/applier/*` | `applier/*` | `component_ref` resolution split (below) |
| `infra/backend_client.rs` types `ActiveVersionRead`, `ActiveOutcome`, `ActiveComponent`, `Placement`, `Applicability`, `VersionSelector`, `ResolvedComponent`; `infra/saved_outcome_cache.rs` type `ResolvedSavedOutcome` | `bundle.rs` | the HTTP clients and their moka caches stay in the proxy |

The proxy re-exports these from `rre_core::*` at the old paths so its own
imports stay short. `proxy/src/domain/` shrinks to `features_matched.rs` and
`mod.rs`.

Note on the rule model: as of commit `4540042` the `RuleGraph` is a SINGLE
canvas (`RuleGraph { canvas: CanvasGraph }`), not the former
anonymous/registered/customer triple. Visitor segmentation is expressed
in-graph with the `logged_in` and `has_product` decision nodes against a
resolved `Identity`. There is no canvas classifier and no `Canvas` enum
anywhere in this design.

### Dependency rules

`rre-core` may depend on: `zen-engine`, `zen-expression`, `serde`,
`serde_json`, `serde_json_path`, `lol_html`, `ammonia`, `scraper`, `mustache`,
`regex`, `http`, `uuid`, `thiserror`, `futures` (executor only), `serde_yaml`.

It may not depend on: `tokio`, `axum`, `hyper`, `reqwest`, `moka`,
`tracing-subscriber`, `metrics`, `config`, `dotenvy`. `tracing` is allowed for
spans and events; the host installs the subscriber.

**Verified 2026-09-06** by building a probe crate for `wasm32-wasip1`:
`scraper`, `ammonia`, `lol_html`, `mustache`, `serde_json_path`, `regex`,
`http`, `uuid` and `futures` all compile for that target, as does `zen-engine`
with its QuickJS dependency. This removes two rewrites an earlier draft of this
spec assumed were necessary: `scraper` does NOT need replacing with `lol_html`,
and `http::HeaderMap` can stay in `EvaluationContext`. The extraction is
therefore a near-verbatim file move, with `evaluator.rs` the only module whose
body changes.

`make check` gains `cargo build --manifest-path rre-core/Cargo.toml --target
wasm32-wasip1` so the constraint cannot regress silently. `rustup target add
wasm32-wasip1` becomes part of the toolchain setup in `RRE_README.md`.

### Evaluator without tokio or moka

Today `evaluator.rs` owns a thread-local current-thread tokio runtime, spawns
the eval on `tokio::task::spawn_blocking`, and takes `&CompiledCache` (moka).
In `rre-core`:

- The crate exposes a SYNCHRONOUS `evaluate_sync(content, canvas, ctx) ->
  Vec<MatchedAction>`. It builds the adapter, the `DecisionEngine` and the
  `Variable` locally and drives zen's async `evaluate_with_opts` with
  `futures::executor::block_on`. No tokio, no `spawn_blocking`, no moka.
- Compilation is the caller's job: `rre_core::compile(canvas) ->
  DecisionContent` is today's `to_decision_content`. The proxy keeps its moka
  `CompiledCache` wrapped around that call; Compute compiles per request
  (microseconds for a canvas of this size, and Compute has no cross-request
  memory anyway).
- The proxy's own threading is unchanged: `proxy/src/domain/evaluator.rs`
  keeps its `spawn_blocking` + thread-local runtime and calls
  `rre_core::evaluate_sync` inside the closure, because zen's `Variable` and
  `scraper::Html` are `!Send` and must not cross an await point.
- `EvaluationOptions { trace: true, max_depth: 10 }` and the `__expr` marker
  recovery are unchanged.

**Risk to verify in Task 2 of the plan:** `zen-engine` depends on `tokio` with
the `sync` and `time` features. `tokio::sync` primitives are runtime-agnostic,
but anything touching `tokio::time` panics under
`futures::executor::block_on` with no reactor installed. The first
implementation task asserts a real evaluation completes under `block_on`. If it
panics, the fallback is a `tokio` current-thread runtime with only the `rt`
feature, which also builds for `wasm32-wasip1`; the public API does not change
either way.

### Meta tags and the applicability gate

Unchanged. `context.rs::extract_meta_tags` keeps its `scraper` implementation
and moves verbatim, as does the `html_selector` applicability check, which
moves from `proxy/src/forwarder.rs::html_selector_matches` into
`rre-core::apply`. `scraper` builds for `wasm32-wasip1` (see Dependency rules),
so no rewrite is needed.

### Applier: component references

`applier/component_ref.rs` today resolves a `component_ref` action by calling
`BackendClient` to fetch the template. In `rre-core` the applier receives a
`ResolvedComponentMap` and never fetches. The two hosts fill the map
differently:

- Proxy: unchanged behaviour, it resolves refs through `component_cache` before
  calling the applier.
- Edge: the bundle already has every ref resolved inline (Part 2), so the map is
  built from the bundle.

### Public API

```rust
pub struct RequestFacts {
    pub headers: http::HeaderMap,
    pub path: String,
    pub cookies: HashMap<String, String>,
    pub site: Option<String>,
    pub identity: Identity,           // logged_in + product labels
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum BodyKind { Html, Json }

pub struct FeatureReport {
    pub feature_id: String,
    pub version_number: i32,
    pub matched: bool,      // the canvas routed to >=1 expression node
    pub changed: bool,      // an action actually modified the body
    pub skipped_reason: Option<&'static str>,   // "applicability", "no_match", "error"
}

pub struct ApplyOutcome {
    pub body: String,
    pub changed: bool,
    pub features: Vec<FeatureReport>,
}

/// Evaluate every feature in `bundle` whose `kind` matches `kind`, in bundle
/// order, folding each feature's matched actions over the body. Never panics
/// on untrusted `facts` or `body`; a per-feature failure is recorded in the
/// report and skipped, exactly as the proxy's loop does today.
pub fn apply(
    bundle: &EdgeBundle,
    facts: &RequestFacts,
    kind: BodyKind,
    body: String,
    sanitizer: &ammonia::Builder<'static>,
) -> ApplyOutcome;
```

`Identity` is resolved by the host with `rre_core::identity::resolve(headers,
cookies, &IdentitySettings)`, because the cookie and header NAMES that carry it
are a deployment concern: the proxy reads them from its config, Compute from
constants. The resolution logic itself is shared, so a visitor who is
`logged_in` in the Test panel is `logged_in` at the edge.

The proxy's `forwarder.rs` per-feature loop becomes a call to `apply` with a
bundle assembled on the fly from its cached `ActiveVersionRead`s plus the
component and saved-outcome maps it already resolves. This is the parity
guarantee: there is one loop, in one crate.

### The sanitizer

`applier/html_sanitizer.rs::load_sanitizer` reads an allow-list YAML from disk,
which Compute cannot do. `rre-core` keeps `load_sanitizer(path)` for the proxy
and adds `rre_core::default_sanitizer() -> ammonia::Builder<'static>`, built
from the same YAML embedded with `include_str!` at
`rre-core/config/sanitizer.yaml` (moved from `proxy/config/sanitizer.yaml`, with
the proxy's config default repointed at the new location). Both hosts therefore
sanitize identically by default.

### Panic safety

`apply` must not panic on any input. Concretely: `expect`/`unwrap`/indexing on
data derived from the body, headers or bundle is replaced with error returns;
the JSON path walker, mustache render and `lol_html` rewrite errors are
already `Result`s. A fuzz-style test feeds truncated, non-UTF-8 and deeply
nested bodies. A panic in Compute is a 500 for the reader, so this is a hard
requirement, not hygiene.

### Golden fixtures

`rre-core/tests/golden/<case>/{bundle.json, facts.json, input.(json|html),
expected.(json|html), canvas}`. A test iterates the directory and asserts
`apply` output equals `expected` byte for byte. The Compute crate runs the same
directory through its wrapper (`include_dir` or a build-script copy) so a
fixture is the parity contract between repos. Initial cases: json truncation
via `trim_json`, `json_set` and `json_remove`, html injection at a selector
with each placement mode, content truncation with fade, `component_ref`
resolved inline, `site_match` mismatch (no change), malformed JSON body
(unchanged, error reported), applicability gate miss.

## Part 2: edge bundle export

**Who calls this, and when.** An operator, manually, at export time, from a
machine that can reach the RRE backend (or its database). The output is a JSON
file that is committed into the Fastly repo and compiled into the Compute
package. Nothing at the edge ever calls the RRE backend or proxy; after
deployment the edge has no dependency on RRE being up.

Two equivalent front doors to the same service, pick one or keep both:

- HTTP: `GET /api/v1/sites/{slug}/edge-bundle?env=…` on the backend (below).
- CLI: `cargo run --bin export_bundle -- --site <slug> --env live > bundle.json`,
  a binary in `backend/src/bin/` next to `seed_demo`, reading the DB directly
  through the same service. Use this when the backend's HTTP port is not
  reachable from where the export is run.

### Endpoint

```
GET /api/v1/sites/{slug}/edge-bundle?env=live|staging
→ 200 application/json  EdgeBundle
→ 404 SITE_NOT_FOUND
→ 400 VALIDATION  (bad env)
```

`env` defaults to `live`. Features with no published version for `env` are
omitted, so a site with nothing published returns `features: []`, which is a
valid bundle that applies nothing.

### Shape

```json
{
  "schema_version": 1,
  "site": { "slug": "intrafish-com", "source_host": "test.intrafish.com" },
  "environment": "live",
  "generated_at": "2026-09-06T10:00:00Z",
  "features": [
    {
      "id": "paywall-truncate",
      "type": "json",
      "execution_order": 1,
      "version_number": 7,
      "applicability": { "html_selector": null, "json_selector": null },
      "rule_graph": { "canvas": { "nodes": [], "edges": [], "root_node_id": null } },
      "outcomes": [ { "id": "…", "title": "…", "is_builtin": false, "order_index": 0,
                      "components": [ { "id": "…", "slug": "…", "type": "html_injection",
                                        "config": {}, "placement": "inline", "order_index": 0 } ] } ],
      "saved_outcomes": { "<uuid>": { "html_body": "…", "css_body": "…" } },
      "resolved_components": { "<uuid>|default": { "version_number": 3, "html_body": "…",
                                                    "css_body": "…", "variables": {} } }
    }
  ]
}
```

- `features` sorted by `(type, execution_order)`: all `html` features, then all
  `json` features, each ascending. Consumers run in array order.
- `rule_graph`, `applicability`, `outcomes` are exactly `ActiveVersionRead`'s
  fields, so `EdgeBundle` in `rre-core::bundle` is `Vec<ActiveVersionRead>`
  plus feature identity and site metadata. No second schema to keep in sync.
- **Component and saved-outcome resolution.** The canvas is left untouched;
  what the export adds are two lookup tables per feature, mirroring the maps
  the proxy builds per request:
  - `resolved_components`: every `component_ref` reachable from the canvas
    (an `apply_component*` action, or a `component_ref*` component inside an
    outcome the canvas can apply), keyed `"<component_uuid>|<version>"` where
    version is `default` or a number, matching `VersionSelector::as_query`.
    Values are `ResolvedComponent { version_number, html_body, css_body,
    variables }`, exactly what `GET /component-templates/{cid}/resolve` returns.
  - `saved_outcomes`: every Outcomes-Library outcome referenced by an
    `apply_saved_outcome*` action, keyed by outcome uuid, holding
    `ResolvedSavedOutcome`.

  Reachability is computed statically: every expression node in the canvas is
  inspected, regardless of whether a given request would route to it, so the
  bundle is request-independent. `rre-core` turns these two tables into the
  `ResolvedComponentMap` / `ResolvedSavedOutcomeMap` the applier already takes.
  After export nothing at the edge needs to fetch anything.
- `schema_version` is bumped on any incompatible change to the above. `rre-core`
  rejects a bundle whose version it does not know.

Feature-to-site scoping: RRE features are not scoped to a site (a rule graph
gates on `site_match` if it wants to be). The export therefore includes every
feature with a published version for `env`, and the `site` block tells the edge
which slug to put into `RequestFacts.site`. This mirrors what the proxy does
today for a request to that host.

### Implementation

- `backend/src/services/edge_bundle_service.rs`: load site by slug; list
  features ordered by `(type, execution_order)`; for each with a version in
  `env`, reuse `version_service`'s active-version loader; resolve component
  refs through `component_template_service`; assemble.
- `backend/src/api/v1/sites.rs`: one route, delegating to the service.
- `backend/src/schemas/edge_bundle.rs`: the response type, mirrored in
  `rre-core::bundle::EdgeBundle` (same rule as every proxy/backend mirror in
  CONTRACTS.md: the two must agree; a golden fixture is deserialised by both in
  tests).
- CONTRACTS.md gains the route, the schema, and the sort rule.
- OpenAPI: added to `openapi.rs` like every other route.

### Consumer

The Fastly repo's `scripts/export-rules.sh` calls this endpoint and commits the
result under `compute/intrafish-edge/rules/`. No tooling for that lives here.

### Later: push to Fastly KV on publish

Not built now. When it is:

- `Site` gains `fastly_kv_store_id: Option<String>` and `fastly_kv_key_prefix`.
- `version_service::publish/unpublish` enqueue an `edge_publish_outbox` row
  after commit; a background task builds the bundle via
  `edge_bundle_service` and `PUT`s it to
  `https://api.fastly.com/resources/stores/kv/{store_id}/keys/{prefix}/{env}`
  with a token from config, retrying with backoff and surfacing failures on
  the site page.
- The bundle format is the same document this endpoint returns, so the Compute
  `rre_kv` loader written now needs no change.

## Impact on the proxy

Behaviourally none. Structurally: `domain/` shrinks, `forwarder.rs`'s
per-feature loop is replaced by `rre_core::apply`, `infra/compiled_cache.rs`
wraps `rre_core::compile`, and `component_cache.rs` produces a
`ResolvedComponentMap`. All existing proxy tests must pass unchanged;
`/__rre/eval*` endpoints keep their contracts.

## Testing

- `rre-core`: moved unit tests; golden fixtures; panic-safety inputs;
  `wasm32-wasip1` build in `make check`.
- Backend: service test that a site with html and json features exports in
  the right order with refs resolved; 404 and 400 cases; a fixture round-trip
  test that `schemas::EdgeBundle` and `rre_core::bundle::EdgeBundle` accept the
  same JSON.
- Proxy: full existing suite green after the extraction; one new test that a
  request through the forwarder and `rre_core::apply` with an equivalent
  bundle produce the same body for a golden case.

## Decisions taken in this spec

1. Extract a crate rather than copy code into the Fastly repo. Parity is the
   whole point.
2. Ship the canvas `rule_graph` in the bundle and translate at the edge; do
   not pre-compile to JDM.
3. Resolve `component_ref` at export time; keep sanitising and rendering at the
   edge so the code path matches the proxy.
4. Replace `scraper` with `lol_html` in the two places it is used rather than
   trying to build `scraper` for wasm.
5. Drive zen with `futures::executor::block_on` inside `rre-core`; the proxy
   keeps its own `spawn_blocking` around the call.
6. The export endpoint includes all published features, not a per-site subset,
   because RRE has no per-site feature scoping.

## Known limits and open questions

1. The export endpoint is unauthenticated like the rest of the API. Fine on a
   private network; must be revisited with the rest of API auth before the
   backend is reachable from anywhere else.
2. `futures::executor::block_on` on `wasm32-wasip1` has not been exercised yet.
   If zen's evaluate ever awaits a tokio primitive (it uses `tokio::sync`), the
   fallback is a tokio `current_thread` runtime with the `rt` feature, which
   also builds on wasip1. Verify in the first implementation task.
3. `ammonia` and `mustache` have not been built for `wasm32-wasip1` yet. Both
   are pure Rust and expected to build; verify alongside item 2.
4. Bundle size is unbounded. A site with many features or large component
   templates produces a large document that Compute parses per request. No
   limit is enforced here; the Fastly spec notes the cost.
5. The proxy's `moka` compiled cache key changes from `(feature, version,
   canvas)` to whatever wraps `rre_core::compile`; behaviour is the same but the
   CONTRACTS.md cache section needs updating.
