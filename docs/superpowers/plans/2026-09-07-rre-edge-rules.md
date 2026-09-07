# RRE Rules at the Fastly Edge — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Run RRE rules inside the `intrafish-edge` Fastly Compute service with no network call except the existing Varnish fetch, selectable per request against the current Zephr flow so both can be A/B tested.

**Architecture:** Extract the proxy's rule evaluator and body applier into a new `rre-core` crate that builds for `wasm32-wasip1`. The RRE backend gains an "edge bundle" export that serialises a site's published rules, with component templates and saved outcomes resolved inline, into one JSON document. That document is exported manually, vendored into the Fastly repo, and compiled into the Wasm package, where `rre-core` evaluates it in-process. The Compute service grows three sibling flow directories (`zephr`, `rre_export`, `rre_kv`) with a router that picks one from a header or cookie.

**Tech Stack:** Rust (zen-engine/JDM, axum, sqlx, lol_html, ammonia, scraper, mustache), Fastly Compute SDK 0.13, Postgres, `wasm32-wasip1`.

**Specs:**
- `zen/docs/superpowers/specs/2026-09-06-rre-edge-bundle-design.md` (this repo)
- `dngroup-fastly/docs/superpowers/specs/2026-09-06-intrafish-rre-edge-rules-design.md` (Fastly repo)

## Global Constraints

- **Two repos.** `ZEN = /Users/tahmid/IdeaProjects/zen`, `FASTLY = /Users/tahmid/IdeaProjects/dngroup-fastly`. Tasks say which.
- **Cargo is not on PATH** in non-interactive shells. Prefix every cargo command with `PATH="$HOME/.cargo/bin:$PATH"`.
- **`backend/`, `proxy/` and the new `rre-core/` are standalone crates** — each `Cargo.toml` has an empty `[workspace]` table detaching it from the zen root workspace. Build with `--manifest-path`.
- **SQLx runtime queries only** (`sqlx::query_as::<_, T>(...)`), never the `query!` macros.
- **`CONTRACTS.md` is authoritative** for schema, routes, rule_graph JSON. Update it in the same commit as any change it describes.
- **The rule model is a SINGLE canvas** (`RuleGraph { canvas: CanvasGraph }`) as of commit `4540042`. There is no `Canvas` enum and no classifier. Visitor segmentation uses the `logged_in` / `has_product` decision nodes against a resolved `Identity`.
- **`rre-core` may depend on:** `zen-engine`, `zen-expression`, `serde`, `serde_json`, `serde_json_path`, `lol_html`, `ammonia`, `scraper`, `mustache`, `regex`, `http`, `uuid`, `thiserror`, `futures`, `serde_yaml`, `tracing`.
- **`rre-core` may NOT depend on:** `tokio`, `axum`, `hyper`, `reqwest`, `moka`, `metrics`, `config`, `dotenvy`, `anyhow`.
- **`rre-core` must never panic** on untrusted input (response bodies, request headers, bundle contents). A panic in Compute is a 500 for a reader.
- **Conventional Commits**, imperative subject ≤50 chars. No attribution trailers beyond the session line below.
- Every commit message ends with:
  `Claude-Session: https://claude.ai/code/session_01BwARVnYgVWNnHc1WUSu6Dw`
- **Gate:** `make check` in `ZEN` (backend + proxy + frontend) and `cargo test` in `FASTLY/compute/intrafish-edge`. No CI exists; these are the only gates.
- **Branch:** work on `feat/rre-edge-rules` in both repos, cut from `master` (`ZEN`) and `development/test` (`FASTLY`).

---

## Task 1: Scaffold `rre-core` and move the leaf modules

**Files:**
- Create: `ZEN/rre-core/Cargo.toml`, `ZEN/rre-core/src/lib.rs`
- Move: `ZEN/proxy/src/domain/graph.rs` → `ZEN/rre-core/src/graph.rs`
- Move: `ZEN/proxy/src/domain/identity.rs` → `ZEN/rre-core/src/identity.rs`
- Modify: `ZEN/Makefile` (add `rre-core-check`)

**Interfaces:**
- Produces: crate `rre_core` exporting `graph::{RuleGraph, CanvasGraph, Node, Edge, Branch, Position, ProcessorRef}` and `identity::{Identity, IdentitySettings, resolve}`, all with their current signatures and their existing unit tests.

- [ ] **Step 1: Create the crate manifest**

```toml
# ZEN/rre-core/Cargo.toml
# Standalone crate: the empty [workspace] table detaches this from the zen root
# workspace, matching backend/ and proxy/. MUST build for wasm32-wasip1 — see
# the dependency allow-list in docs/superpowers/specs/2026-09-06-rre-edge-bundle-design.md.
[workspace]

[package]
name = "rre-core"
version = "0.1.0"
edition = "2021"
publish = false

[dependencies]
zen-engine = { path = "../core/engine" }
zen-expression = { path = "../core/expression" }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
serde_json_path = "0.7"
serde_yaml = "0.9"
http = "1"
uuid = { version = "1", features = ["serde"] }
thiserror = "2"
tracing = "0.1"
futures = { version = "0.3", default-features = false, features = ["executor"] }
scraper = "0.27"
lol_html = "2"
ammonia = "4"
mustache = "0.9"
regex = "1"
```

- [ ] **Step 2: Move the two leaf modules verbatim**

```bash
cd /Users/tahmid/IdeaProjects/zen
mkdir -p rre-core/src
git mv proxy/src/domain/graph.rs rre-core/src/graph.rs
git mv proxy/src/domain/identity.rs rre-core/src/identity.rs
```

Then in `rre-core/src/identity.rs` only, change the two `crate::domain::identity` self-references if any exist to `crate::identity`. `graph.rs` has no crate-internal imports.

- [ ] **Step 3: Write `lib.rs`**

```rust
//! Shared RRE rule evaluation and response-body application.
//!
//! One crate, two hosts: the RRE proxy (native, tokio) and the Fastly Compute
//! edge service (`wasm32-wasip1`, single-threaded, no I/O). Everything a rule
//! decision depends on lives here so the Test panel, the proxy and the edge
//! can never disagree.
//!
//! MUST NOT depend on tokio, reqwest, moka or any I/O crate — see the spec's
//! dependency allow-list. `make rre-core-check` enforces the wasm build.

pub mod graph;
pub mod identity;
```

- [ ] **Step 4: Add the Makefile gate**

```makefile
rre-core-check: ## fmt + clippy + test + wasm build the shared rule crate
	cd rre-core && cargo fmt --check && cargo clippy --all-targets -- -D warnings \
		&& cargo test && cargo build --target wasm32-wasip1
```

and add `rre-core-check` as the FIRST prerequisite of the `check` target:
`check: rre-core-check backend-check proxy-check frontend-check`.

- [ ] **Step 5: Verify it builds natively and for wasm, with tests passing**

```bash
cd /Users/tahmid/IdeaProjects/zen/rre-core
PATH="$HOME/.cargo/bin:$PATH" cargo test
PATH="$HOME/.cargo/bin:$PATH" cargo build --target wasm32-wasip1
```

Expected: the moved `identity` tests pass; both builds succeed. The proxy is BROKEN at this point (it still references `domain::graph`); Task 5 repairs it. Do not try to build the proxy yet.

- [ ] **Step 6: Commit**

```bash
git add rre-core Makefile proxy/src/domain
git commit -m "feat(rre-core): scaffold shared crate with graph + identity

Claude-Session: https://claude.ai/code/session_01BwARVnYgVWNnHc1WUSu6Dw"
```

---

## Task 2: Move the evaluation core and prove `block_on` works

**Files:**
- Move: `ZEN/proxy/src/domain/{context.rs,adapter.rs,translator.rs}` → `ZEN/rre-core/src/`
- Move: `ZEN/proxy/src/domain/processors/` → `ZEN/rre-core/src/processors/`
- Create: `ZEN/rre-core/src/evaluator.rs` (from the proxy's, made synchronous)
- Modify: `ZEN/rre-core/src/lib.rs`

**Interfaces:**
- Consumes: `graph::{CanvasGraph, Node}`, `identity::Identity` from Task 1.
- Produces:
  - `compile(canvas: &CanvasGraph) -> zen_engine::model::DecisionContent`
  - `evaluate_sync(content: DecisionContent, canvas: &CanvasGraph, ctx: EvaluationContextParts, registry: Arc<ProcessorRegistry>) -> Vec<MatchedAction>`
  - `MatchedAction { node_id: String, action: serde_json::Value }`
  - `context::{EvaluationContext, EvaluationContextParts, DeviceType}`
  - `processors::ProcessorRegistry` with `ProcessorRegistry::default()` registering all seven kinds.

- [ ] **Step 1: Move the files**

```bash
cd /Users/tahmid/IdeaProjects/zen
git mv proxy/src/domain/context.rs rre-core/src/context.rs
git mv proxy/src/domain/adapter.rs rre-core/src/adapter.rs
git mv proxy/src/domain/translator.rs rre-core/src/translator.rs
git mv proxy/src/domain/processors rre-core/src/processors
git mv proxy/src/domain/evaluator.rs rre-core/src/evaluator.rs
```

Rewrite every `use crate::domain::X` to `use crate::X` across the moved files. In `context.rs` that includes the two fully-qualified `crate::domain::identity::Identity` references in the struct fields and `from_request`'s parameter.

- [ ] **Step 2: Write the failing test that proves `block_on` drives zen**

This is the spec's flagged risk: `zen-engine` pulls `tokio` with `sync` + `time`. If any evaluation path touches `tokio::time`, `futures::executor::block_on` panics with no reactor. Add to `rre-core/src/evaluator.rs`:

```rust
#[cfg(test)]
mod block_on_tests {
    use super::*;
    use crate::graph::{Branch, CanvasGraph, Edge, Node, Position, ProcessorRef};
    use std::sync::Arc;

    fn pos() -> Position { Position { x: 0.0, y: 0.0 } }

    /// A canvas of start -> expression -> end. No decision node, so the only
    /// thing under test is that zen's async evaluate completes under
    /// `futures::executor::block_on` with NO tokio runtime installed.
    fn linear_canvas() -> CanvasGraph {
        CanvasGraph {
            nodes: vec![
                Node::Start { id: "s".into(), position: pos() },
                Node::Expression {
                    id: "e".into(),
                    action: ProcessorRef {
                        kind: "json_set".into(),
                        config: serde_json::json!({ "json_path": "$.x", "value": 1 }),
                    },
                    custom_label: None,
                    position: pos(),
                },
                Node::End { id: "n".into(), position: pos() },
            ],
            edges: vec![
                Edge { id: "e1".into(), source_node_id: "s".into(), target_node_id: "e".into(), branch: Branch::Yes },
                Edge { id: "e2".into(), source_node_id: "e".into(), target_node_id: "n".into(), branch: Branch::Yes },
            ],
            root_node_id: Some("s".into()),
        }
    }

    #[test]
    fn evaluate_sync_completes_without_a_tokio_runtime() {
        let canvas = linear_canvas();
        let content = compile(&canvas);
        let ctx = EvaluationContextParts::from_request(
            &http::HeaderMap::new(),
            "/article",
            &std::collections::HashMap::new(),
            "{}".to_string(),
            true,
            crate::identity::Identity::default(),
        );
        let actions = evaluate_sync(content, &canvas, ctx, Arc::new(ProcessorRegistry::default()));
        assert_eq!(actions.len(), 1, "the expression node on the routed path must be recovered");
        assert_eq!(actions[0].node_id, "e");
    }
}
```

- [ ] **Step 3: Run it and watch it fail**

```bash
cd /Users/tahmid/IdeaProjects/zen/rre-core
PATH="$HOME/.cargo/bin:$PATH" cargo test evaluate_sync_completes
```

Expected: FAIL to compile — `evaluate_sync` and `compile` do not exist yet.

- [ ] **Step 4: Make the evaluator synchronous**

Replace the moved `evaluator.rs` body. Delete `EVAL_RT`, the `thread_local!`, the `spawn_blocking`, the `CompiledCache` parameter and the `GraphEvaluator` struct. Keep `MatchedAction`, `TraceStep`, `EvalTrace`, `RawTraceRow`, `trace_rows`, `ordered_actions`, `action_to_value` exactly as they are. Add:

```rust
/// Translate a canvas to JDM. The caller owns caching: the proxy wraps this in
/// its moka `CompiledCache`; Compute calls it per request (microseconds, and a
/// Wasm instance has no memory across requests anyway).
pub fn compile(canvas: &CanvasGraph) -> DecisionContent {
    to_decision_content(canvas)
}

/// Evaluate one canvas synchronously. Returns the matched expression actions in
/// trace order; an empty vec on dead-end, empty canvas or eval error (fail-open,
/// same contract as the proxy's async evaluator).
///
/// Driven with `futures::executor::block_on`: zen's `evaluate` is `async` but
/// performs no I/O for the node kinds RRE emits (input / switch / custom /
/// expression / output), so no reactor is required. `Variable` and
/// `scraper::Html` are `!Send` and never escape this function.
pub fn evaluate_sync(
    content: DecisionContent,
    canvas: &CanvasGraph,
    ctx: EvaluationContextParts,
    registry: Arc<ProcessorRegistry>,
) -> Vec<MatchedAction> {
    let input_value = ctx.to_input_value();
    let action_map: HashMap<String, serde_json::Value> = canvas
        .nodes
        .iter()
        .filter_map(|n| match n {
            Node::Expression { id, action, .. } => Some((id.clone(), action_to_value(action))),
            _ => None,
        })
        .collect();

    let rows: Vec<RawTraceRow> = futures::executor::block_on(async move {
        let eval_ctx = ctx.into_context();
        let adapter = CanvasNodeAdapter { registry, ctx: Arc::new(eval_ctx) };
        let engine = DecisionEngine::default().with_adapter(Arc::new(adapter));
        let decision = engine.create_decision(content);
        let opts = EvaluationOptions { trace: true, max_depth: 10 };
        match decision.evaluate_with_opts(Variable::from(input_value), opts).await {
            Ok(resp) => resp.trace.map_or_else(Vec::new, trace_rows),
            Err(e) => {
                tracing::warn!(error = %e, "eval=error");
                Vec::new()
            }
        }
    });

    ordered_actions(rows, &action_map)
}
```

Keep `evaluate_with_trace` too, as a sync function with the same transformation (it is used by `/__rre/eval`); it takes `content` and `Arc<EvaluationContext>` as it does today and returns `Result<EvalTrace, String>`.

Add the new modules to `lib.rs`:

```rust
pub mod adapter;
pub mod context;
pub mod evaluator;
pub mod processors;
pub mod translator;

pub use evaluator::{compile, evaluate_sync, EvalTrace, MatchedAction, TraceStep};
```

- [ ] **Step 5: Run the test**

```bash
cd /Users/tahmid/IdeaProjects/zen/rre-core
PATH="$HOME/.cargo/bin:$PATH" cargo test
```

Expected: PASS, including every moved processor and translator test.

**If `evaluate_sync_completes_without_a_tokio_runtime` panics with "there is no reactor running":** the fallback is a tokio current-thread runtime. Add `tokio = { version = "1", default-features = false, features = ["rt"] }` to `rre-core` (it builds for `wasm32-wasip1`), replace `futures::executor::block_on(...)` with a `tokio::runtime::Builder::new_current_thread().build()` + `rt.block_on(...)`, and record the deviation in the spec's "Known limits". The public API does not change. Do NOT enable any other tokio feature.

- [ ] **Step 6: Verify the wasm build still passes**

```bash
PATH="$HOME/.cargo/bin:$PATH" cargo build --target wasm32-wasip1
```

Expected: success.

- [ ] **Step 7: Commit**

```bash
git add -A rre-core proxy/src/domain
git commit -m "feat(rre-core): move evaluator core, drop tokio from eval

- evaluate_sync drives zen with futures::executor::block_on
- compile() exposed so the caller owns graph caching

Claude-Session: https://claude.ai/code/session_01BwARVnYgVWNnHc1WUSu6Dw"
```

---

## Task 3: Move the bundle types, the applier and the sanitizer

**Files:**
- Create: `ZEN/rre-core/src/bundle.rs`
- Move: `ZEN/proxy/src/domain/applier/` → `ZEN/rre-core/src/applier/`
- Move: `ZEN/proxy/config/sanitizer.yaml` → `ZEN/rre-core/config/sanitizer.yaml`
- Modify: `ZEN/rre-core/src/lib.rs`, `ZEN/proxy/config/default.json`

**Interfaces:**
- Consumes: `graph::RuleGraph` from Task 1.
- Produces:
  - `bundle::{Placement, Applicability, ActiveVersionRead, ActiveOutcome, ActiveComponent, VersionSelector, ResolvedComponent, ResolvedSavedOutcome}` — the exact struct definitions currently in `proxy/src/infra/backend_client.rs` and `proxy/src/infra/saved_outcome_cache.rs`, moved with their derives and helper impls (`canvas()`, `find_outcome()`, `is_builtin_show_content()`, `from_action_value()`, `as_query()`).
  - `applier::json_apply::{apply_action_html, apply_action_json, ResolvedComponentMap, ResolvedSavedOutcomeMap, component_ref}` — unchanged signatures.
  - `default_sanitizer() -> ammonia::Builder<'static>`
  - `load_sanitizer(path: &str) -> Result<ammonia::Builder<'static>, SanitizerError>`

- [ ] **Step 1: Move the applier and the sanitizer config**

```bash
cd /Users/tahmid/IdeaProjects/zen
git mv proxy/src/domain/applier rre-core/src/applier
mkdir -p rre-core/config
git mv proxy/config/sanitizer.yaml rre-core/config/sanitizer.yaml
```

Point the proxy's config at the new location: in `proxy/config/default.json`, change `sanitizer_config_path` to `"../rre-core/config/sanitizer.yaml"`.

- [ ] **Step 2: Create `bundle.rs` by moving the shared types**

Cut from `proxy/src/infra/backend_client.rs`: `Placement`, `Applicability`, `ActiveVersionRead` (with its `impl`), `ActiveOutcome`, `ActiveComponent`, `VersionSelector` (with its `impl`), `ResolvedComponent`. Cut from `proxy/src/infra/saved_outcome_cache.rs`: `ResolvedSavedOutcome`. Paste them verbatim into `rre-core/src/bundle.rs` under this header:

```rust
//! Types shared by the RRE backend's published-rule payloads, the proxy's
//! runtime caches and the edge bundle. Moved out of the proxy's
//! `infra::backend_client` so both hosts deserialize the SAME structs — a
//! backend/proxy/edge schema skew is the failure mode this prevents.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

use crate::graph::{CanvasGraph, RuleGraph};
```

Then in the applier, rewrite `use crate::infra::backend_client::X` to `use crate::bundle::X` and `crate::infra::saved_outcome_cache::ResolvedSavedOutcome` to `crate::bundle::ResolvedSavedOutcome`.

- [ ] **Step 3: Write the failing sanitizer test**

Add to `rre-core/src/applier/html_sanitizer.rs`:

```rust
#[test]
fn default_sanitizer_is_embedded_and_strips_scripts() {
    // Compute cannot read files; the allow-list must be compiled in.
    let b = crate::default_sanitizer();
    let out = sanitize(&b, "<p>keep</p><script>steal()</script>");
    assert!(out.contains("keep"));
    assert!(!out.contains("script"), "script tag must not survive: {out}");
}
```

- [ ] **Step 4: Run it and watch it fail**

```bash
cd /Users/tahmid/IdeaProjects/zen/rre-core
PATH="$HOME/.cargo/bin:$PATH" cargo test default_sanitizer_is_embedded
```

Expected: FAIL — `default_sanitizer` not found.

- [ ] **Step 5: Add the embedded sanitizer and swap `anyhow` for `thiserror`**

`anyhow` is not on the allow-list. In `html_sanitizer.rs` replace the `anyhow::Result` signature:

```rust
#[derive(Debug, thiserror::Error)]
pub enum SanitizerError {
    #[error("reading sanitizer config {path}: {source}")]
    Read { path: String, source: std::io::Error },
    #[error("parsing sanitizer config: {0}")]
    Parse(#[from] serde_yaml::Error),
}

/// The allow-list compiled into the binary. The ONLY sanitizer available at the
/// edge, and the default the proxy falls back to, so both hosts sanitize
/// identically.
const EMBEDDED_ALLOWLIST: &str = include_str!("../../config/sanitizer.yaml");

pub fn default_sanitizer() -> ammonia::Builder<'static> {
    // The embedded file is a build-time constant and is covered by
    // `default_sanitizer_is_embedded_and_strips_scripts`; a parse failure here
    // is a build error in disguise, so fall back to ammonia's own defaults
    // rather than panicking in a Wasm request.
    match serde_yaml::from_str(EMBEDDED_ALLOWLIST) {
        Ok(cfg) => builder_from_config(cfg),
        Err(e) => {
            tracing::error!(error = %e, "embedded sanitizer allow-list failed to parse");
            ammonia::Builder::default()
        }
    }
}

pub fn load_sanitizer(path: &str) -> Result<ammonia::Builder<'static>, SanitizerError> {
    let raw = std::fs::read_to_string(path)
        .map_err(|source| SanitizerError::Read { path: path.to_string(), source })?;
    Ok(builder_from_config(serde_yaml::from_str(&raw)?))
}
```

Extract whatever the existing `load_sanitizer` does after parsing into `builder_from_config(cfg) -> ammonia::Builder<'static>` so both entry points share it. Re-export `default_sanitizer` and `load_sanitizer` from `lib.rs`, and add `pub mod applier; pub mod bundle;`.

- [ ] **Step 6: Run the tests**

```bash
PATH="$HOME/.cargo/bin:$PATH" cargo test
PATH="$HOME/.cargo/bin:$PATH" cargo build --target wasm32-wasip1
```

Expected: all applier tests pass, wasm build succeeds.

- [ ] **Step 7: Commit**

```bash
git add -A rre-core proxy
git commit -m "feat(rre-core): move applier, bundle types, embedded sanitizer

Claude-Session: https://claude.ai/code/session_01BwARVnYgVWNnHc1WUSu6Dw"
```

---

## Task 4: The `apply` entry point and the `EdgeBundle` type

**Files:**
- Create: `ZEN/rre-core/src/edge.rs`
- Modify: `ZEN/rre-core/src/lib.rs`, `ZEN/rre-core/src/bundle.rs`

**Interfaces:**
- Consumes: everything from Tasks 1–3.
- Produces:
  - `edge::{EdgeBundle, EdgeSite, EdgeFeature, SCHEMA_VERSION}`
  - `edge::{RequestFacts, BodyKind, ApplyOutcome, FeatureReport}`
  - `apply(bundle: &EdgeBundle, facts: &RequestFacts, kind: BodyKind, body: String, sanitizer: &ammonia::Builder<'static>) -> ApplyOutcome`
  - `html_selector_matches(applicability: &Applicability, html: &str) -> bool`
  - `json_selector_matches(applicability: &Applicability, json: &serde_json::Value) -> bool`

- [ ] **Step 1: Write the failing test**

Create `ZEN/rre-core/tests/apply.rs`:

```rust
use std::collections::HashMap;
use rre_core::edge::{BodyKind, EdgeBundle, RequestFacts};

fn bundle_from(json: &str) -> EdgeBundle {
    serde_json::from_str(json).expect("fixture bundle parses")
}

fn facts() -> RequestFacts {
    RequestFacts {
        headers: http::HeaderMap::new(),
        path: "/proxy/global/v2/content/2-1-1742531".into(),
        cookies: HashMap::new(),
        site: Some("intrafish-com".into()),
        identity: rre_core::identity::Identity::default(),
    }
}

/// One json feature, a canvas of start -> expression(json_set) -> end, applied
/// to an article body. Proves the whole path: parse bundle, compile canvas,
/// evaluate, fold the action over the body.
#[test]
fn apply_runs_a_json_feature_and_modifies_the_body() {
    let bundle = bundle_from(include_str!("fixtures/json_set_bundle.json"));
    let out = rre_core::apply(
        &bundle,
        &facts(),
        BodyKind::Json,
        r#"{"id":"a1","body":[1,2,3,4,5]}"#.to_string(),
        &rre_core::default_sanitizer(),
    );
    assert!(out.changed, "the json_set action must change the body");
    let v: serde_json::Value = serde_json::from_str(&out.body).unwrap();
    assert_eq!(v["access"], serde_json::json!("granted"));
    assert_eq!(out.features.len(), 1);
    assert!(out.features[0].matched);
}

/// A bundle whose feature is html-kind must not run against a JSON body.
#[test]
fn apply_skips_features_of_the_other_kind() {
    let bundle = bundle_from(include_str!("fixtures/html_inject_bundle.json"));
    let out = rre_core::apply(
        &bundle,
        &facts(),
        BodyKind::Json,
        r#"{"id":"a1"}"#.to_string(),
        &rre_core::default_sanitizer(),
    );
    assert!(!out.changed);
    assert!(out.features.is_empty(), "other-kind features are not reported");
}

/// A body that is not the JSON the feature expects must come back untouched
/// rather than panicking — the hard requirement for running in Wasm.
#[test]
fn apply_never_panics_on_a_malformed_body() {
    let bundle = bundle_from(include_str!("fixtures/json_set_bundle.json"));
    for body in ["", "{", "null", "[]", "\u{feff}not json at all"] {
        let out = rre_core::apply(
            &bundle,
            &facts(),
            BodyKind::Json,
            body.to_string(),
            &rre_core::default_sanitizer(),
        );
        assert_eq!(out.body, body, "malformed body must pass through unchanged");
    }
}
```

Create `ZEN/rre-core/tests/fixtures/json_set_bundle.json` — a complete bundle with one `json` feature whose canvas is `start → expression(json_set $.access = "granted") → end`, `applicability` both-null, empty `outcomes`, `resolved_components` and `saved_outcomes`. Create `html_inject_bundle.json` the same way but `"type": "html"` with an `html_injection` action.

- [ ] **Step 2: Run and watch it fail**

```bash
cd /Users/tahmid/IdeaProjects/zen/rre-core
PATH="$HOME/.cargo/bin:$PATH" cargo test --test apply
```

Expected: FAIL to compile — `rre_core::edge` and `rre_core::apply` do not exist.

- [ ] **Step 3: Write `edge.rs`**

```rust
//! The edge bundle: one self-contained document holding everything a host
//! needs to evaluate a site's published rules with no I/O, plus `apply`, the
//! single per-feature loop both the proxy and the Fastly edge run.

use std::collections::HashMap;
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::applier::json_apply::{
    self, ResolvedComponentMap, ResolvedSavedOutcomeMap,
};
use crate::bundle::{
    ActiveOutcome, Applicability, ResolvedComponent, ResolvedSavedOutcome, VersionSelector,
};
use crate::context::EvaluationContextParts;
use crate::graph::RuleGraph;
use crate::identity::Identity;
use crate::processors::ProcessorRegistry;

/// Bumped on any incompatible change to the bundle format. A host rejects a
/// bundle it does not recognise rather than guessing.
pub const SCHEMA_VERSION: u32 = 1;

#[derive(Deserialize, Serialize, Clone, Debug)]
pub struct EdgeBundle {
    pub schema_version: u32,
    pub site: EdgeSite,
    pub environment: String,
    pub generated_at: String,
    pub features: Vec<EdgeFeature>,
}

#[derive(Deserialize, Serialize, Clone, Debug)]
pub struct EdgeSite {
    pub slug: String,
    pub source_host: String,
}

#[derive(Deserialize, Serialize, Clone, Debug)]
pub struct EdgeFeature {
    pub id: String,
    /// `"html"` or `"json"`; selects which responses this feature runs against.
    pub r#type: String,
    pub execution_order: i32,
    pub version_number: i32,
    pub applicability: Applicability,
    pub rule_graph: RuleGraph,
    #[serde(default)]
    pub outcomes: Vec<ActiveOutcome>,
    /// Every Component template the canvas can reach, keyed
    /// `"<uuid>|<default|N>"` (see `VersionSelector::as_query`).
    #[serde(default)]
    pub resolved_components: HashMap<String, ResolvedComponent>,
    /// Every Outcomes-Library outcome the canvas can reach, keyed by uuid.
    #[serde(default)]
    pub saved_outcomes: HashMap<Uuid, ResolvedSavedOutcome>,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum BodyKind { Html, Json }

impl BodyKind {
    /// The bundle's `type` discriminator for this body kind.
    pub fn as_str(self) -> &'static str {
        match self { BodyKind::Html => "html", BodyKind::Json => "json" }
    }
}

/// Everything about the request a rule can branch on. Built by the host,
/// because the cookie/header NAMES that carry identity are deployment config.
#[derive(Clone, Debug)]
pub struct RequestFacts {
    pub headers: http::HeaderMap,
    pub path: String,
    pub cookies: HashMap<String, String>,
    pub site: Option<String>,
    pub identity: Identity,
}

#[derive(Clone, Debug)]
pub struct FeatureReport {
    pub feature_id: String,
    pub version_number: i32,
    pub matched: bool,
    pub changed: bool,
    pub skipped_reason: Option<&'static str>,
}

#[derive(Clone, Debug)]
pub struct ApplyOutcome {
    pub body: String,
    pub changed: bool,
    pub features: Vec<FeatureReport>,
}
```

Then the loop itself, lifted from `proxy/src/forwarder.rs::apply_features_html` / `apply_features_json` with the I/O removed:

```rust
pub fn apply(
    bundle: &EdgeBundle,
    facts: &RequestFacts,
    kind: BodyKind,
    body: String,
    sanitizer: &ammonia::Builder<'static>,
) -> ApplyOutcome {
    let registry = Arc::new(ProcessorRegistry::default());
    let mut current = body;
    let mut changed = false;
    let mut reports = Vec::new();

    for feature in bundle.features.iter().filter(|f| f.r#type == kind.as_str()) {
        let canvas = &feature.rule_graph.canvas;

        if !applicability_matches(&feature.applicability, kind, &current) {
            reports.push(report(feature, false, false, Some("applicability")));
            continue;
        }

        let needs_meta_tags = kind == BodyKind::Html && canvas_has_meta_tags(canvas);
        let body_for_ctx = if kind == BodyKind::Json || needs_meta_tags {
            current.clone()
        } else {
            String::new()
        };
        let mut ctx = EvaluationContextParts::from_request(
            &facts.headers,
            &facts.path,
            &facts.cookies,
            body_for_ctx,
            kind == BodyKind::Json,
            facts.identity.clone(),
        )
        .with_site(facts.site.clone());
        ctx.needs_meta_tags = needs_meta_tags;

        let content = crate::compile(canvas);
        let actions = crate::evaluate_sync(content, canvas, ctx, registry.clone());
        if actions.is_empty() {
            reports.push(report(feature, false, false, Some("no_match")));
            continue;
        }

        let components = component_map(feature);
        let saved = saved_outcome_map(feature);
        let mut feature_changed = false;
        for ma in &actions {
            match kind {
                BodyKind::Html => {
                    let (next, did) = json_apply::apply_action_html(
                        current, &ma.action, &feature.outcomes, &components, &saved, sanitizer,
                    );
                    current = next;
                    feature_changed |= did;
                }
                BodyKind::Json => {
                    let mut value = match serde_json::from_str::<serde_json::Value>(&current) {
                        Ok(v) => v,
                        Err(_) => break, // malformed body: leave it exactly as received
                    };
                    let did = json_apply::apply_action_json(
                        &mut value, &ma.action, &feature.outcomes, &components, &saved, sanitizer,
                    );
                    if did {
                        if let Ok(s) = serde_json::to_string(&value) {
                            current = s;
                            feature_changed = true;
                        }
                    }
                }
            }
        }
        changed |= feature_changed;
        reports.push(report(feature, true, feature_changed, None));
    }

    ApplyOutcome { body: current, changed, features: reports }
}
```

Add the private helpers in the same file: `report(...)`, `canvas_has_meta_tags(canvas)` (moved from the forwarder), `applicability_matches(applicability, kind, body)` dispatching to `html_selector_matches` (moved verbatim from `proxy/src/forwarder.rs::html_selector_matches`, which uses `scraper`) or `json_selector_matches` (moved from the forwarder's JSON equivalent, which uses `serde_json_path`), and:

```rust
/// Turn the bundle's flat tables into the maps the applier already takes. The
/// proxy fills these from its caches; at the edge they come pre-resolved.
fn component_map(feature: &EdgeFeature) -> ResolvedComponentMap {
    feature
        .resolved_components
        .iter()
        .filter_map(|(key, rc)| parse_component_key(key).map(|k| (k, Arc::new(rc.clone()))))
        .collect()
}

fn saved_outcome_map(feature: &EdgeFeature) -> ResolvedSavedOutcomeMap {
    feature.saved_outcomes.iter().map(|(id, so)| (*id, Arc::new(so.clone()))).collect()
}

/// `"<uuid>|default"` / `"<uuid>|7"` -> the applier's map key. An unparseable
/// key is dropped, not fatal: the action falls back to its own skip path.
fn parse_component_key(key: &str) -> Option<(Uuid, VersionSelector)> {
    let (id, version) = key.split_once('|')?;
    let id = Uuid::parse_str(id).ok()?;
    let selector = if version == "default" {
        VersionSelector::Default
    } else {
        VersionSelector::Number(version.parse().ok()?)
    };
    Some((id, selector))
}
```

Export from `lib.rs`: `pub mod edge;` and `pub use edge::apply;`.

- [ ] **Step 4: Run the tests**

```bash
PATH="$HOME/.cargo/bin:$PATH" cargo test --test apply
PATH="$HOME/.cargo/bin:$PATH" cargo test
PATH="$HOME/.cargo/bin:$PATH" cargo build --target wasm32-wasip1
```

Expected: all three `apply` tests PASS, the whole suite PASSES, wasm builds.

**Note on `VersionSelector`:** confirm its real variant names in `bundle.rs` before writing `parse_component_key` and match them; the names above are the expected shape, not a guess to paste blindly. Likewise confirm `apply_action_json`'s real signature (it takes `&mut Value` and returns `bool`) and adapt the loop if it differs.

- [ ] **Step 5: Commit**

```bash
git add -A rre-core
git commit -m "feat(rre-core): add EdgeBundle + apply(), the shared feature loop

Claude-Session: https://claude.ai/code/session_01BwARVnYgVWNnHc1WUSu6Dw"
```

---

## Task 5: Rewire the proxy onto `rre-core`

**Files:**
- Modify: `ZEN/proxy/Cargo.toml`, `ZEN/proxy/src/domain/mod.rs`, `ZEN/proxy/src/lib.rs`, `ZEN/proxy/src/forwarder.rs`, `ZEN/proxy/src/eval.rs`, `ZEN/proxy/src/full_journey.rs`, `ZEN/proxy/src/state.rs`, `ZEN/proxy/src/infra/{backend_client.rs,saved_outcome_cache.rs,compiled_cache.rs,component_cache.rs}`
- Create: `ZEN/proxy/src/domain/evaluator.rs` (thin async wrapper)

**Interfaces:**
- Consumes: the whole `rre_core` public API from Tasks 1–4.
- Produces: no new public API. The proxy's behaviour, routes and response headers are unchanged; this is a pure refactor whose success criterion is the existing test suite.

- [ ] **Step 1: Add the dependency and delete the moved modules**

```toml
# ZEN/proxy/Cargo.toml — replace the direct zen deps with the shared crate.
rre-core = { path = "../rre-core" }
zen-engine = { path = "../core/engine" }
zen-expression = { path = "../core/expression" }
```

Keep `zen-engine`/`zen-expression` only if the proxy still names those types directly after the rewrite; drop them if it does not. Remove `scraper`, `ammonia`, `mustache`, `lol_html`, `serde_json_path` and `regex` from `proxy/Cargo.toml` ONLY if no proxy file still uses them after this task — check with `grep -rn "scraper::\|ammonia::\|mustache::\|lol_html::\|serde_json_path::\|regex::" proxy/src`.

- [ ] **Step 2: Re-export from `domain/mod.rs` so call sites keep their paths**

```rust
//! Proxy-side domain layer. The rule model, evaluator and applier now live in
//! the shared `rre-core` crate so the proxy and the Fastly edge service run
//! identical code; these re-exports keep the proxy's own import paths stable.

pub mod features_matched;

pub use rre_core::{adapter, applier, bundle, context, graph, identity, processors, translator};
pub use rre_core::{compile, EvalTrace, MatchedAction, TraceStep};

pub mod evaluator;
```

- [ ] **Step 3: Write the thin async evaluator wrapper**

`proxy/src/domain/evaluator.rs` keeps the proxy's threading model (zen's `Variable` and `scraper::Html` are `!Send`, so evaluation must not cross an await point) and delegates the work:

```rust
//! Proxy-side async wrapper around `rre_core::evaluate_sync`.
//!
//! The evaluation itself is synchronous and lives in `rre-core`. What stays
//! here is the proxy's threading model: `spawn_blocking` keeps a multi-ms
//! evaluation off the async workers, and the `!Send` zen `Variable` /
//! `scraper::Html` are built and dropped INSIDE the closure.

use std::sync::Arc;

use rre_core::context::EvaluationContextParts;
use rre_core::graph::CanvasGraph;
use rre_core::processors::ProcessorRegistry;
use rre_core::MatchedAction;

use crate::infra::compiled_cache::CompiledCache;

pub struct GraphEvaluator<'a> {
    pub registry: Arc<ProcessorRegistry>,
    pub compiled: &'a CompiledCache,
}

impl<'a> GraphEvaluator<'a> {
    pub fn new(registry: Arc<ProcessorRegistry>, compiled: &'a CompiledCache) -> Self {
        Self { registry, compiled }
    }

    pub async fn evaluate(
        &self,
        canvas: &CanvasGraph,
        ctx: EvaluationContextParts,
        feature_id: &str,
        version_number: i32,
    ) -> Vec<MatchedAction> {
        let content = self
            .compiled
            .get_or_compile(feature_id, version_number, || rre_core::compile(canvas));
        let registry = self.registry.clone();
        let canvas = canvas.clone();
        tokio::task::spawn_blocking(move || {
            rre_core::evaluate_sync((*content).clone(), &canvas, ctx, registry)
        })
        .await
        .unwrap_or_else(|e| {
            tracing::error!(error = %e, "eval_task_panic");
            Vec::new()
        })
    }
}
```

Keep `evaluate_with_trace` as a second method delegating to `rre_core::evaluator::evaluate_with_trace` inside the same `spawn_blocking`, preserving the `/__rre/eval` contract.

- [ ] **Step 4: Point the caches and clients at the moved types**

In `infra/backend_client.rs` and `infra/saved_outcome_cache.rs`, delete the moved struct definitions and add `pub use rre_core::bundle::{...};` re-exports so the rest of the proxy's imports resolve unchanged. `CompiledCache` keeps its moka cache but its closure now returns `rre_core::compile(canvas)`.

In `state.rs`, the sanitizer field becomes `rre_core::load_sanitizer(&settings.sanitizer_config_path).unwrap_or_else(|e| { tracing::warn!(error = %e, "sanitizer config unreadable; using embedded allow-list"); rre_core::default_sanitizer() })`.

- [ ] **Step 5: Run the proxy's full suite**

```bash
cd /Users/tahmid/IdeaProjects/zen/proxy
PATH="$HOME/.cargo/bin:$PATH" cargo fmt
PATH="$HOME/.cargo/bin:$PATH" cargo clippy --all-targets -- -D warnings
PATH="$HOME/.cargo/bin:$PATH" cargo test
```

Expected: PASS with the same test count as before the extraction (minus the tests that physically moved into `rre-core`). Any behavioural difference is a bug in this task, not an intended change.

- [ ] **Step 6: Commit**

```bash
cd /Users/tahmid/IdeaProjects/zen
git add -A proxy rre-core
git commit -m "refactor(proxy): consume rre-core for eval and apply

Claude-Session: https://claude.ai/code/session_01BwARVnYgVWNnHc1WUSu6Dw"
```

---

## Task 6: Prove proxy/edge parity with golden fixtures

**Files:**
- Create: `ZEN/rre-core/tests/golden.rs`, `ZEN/rre-core/tests/golden/<case>/…`
- Modify: `ZEN/proxy/src/forwarder.rs` (route its per-feature loop through `rre_core::apply`)

**Interfaces:**
- Consumes: `rre_core::apply` from Task 4.
- Produces: `ZEN/rre-core/tests/golden/` as a directory contract — the Fastly repo runs the same fixtures in Task 13, so its layout is frozen here: each case is a directory containing `bundle.json`, `facts.json`, `input.json` or `input.html`, and `expected.json` or `expected.html`.

- [ ] **Step 1: Write the golden runner**

```rust
//! Golden fixtures: the parity contract between the proxy and the Fastly edge.
//! Both hosts run THIS directory through `rre_core::apply`; a case that passes
//! in one and fails in the other is a parity bug, which is the whole reason
//! the evaluator lives in a shared crate.

use std::collections::HashMap;
use std::path::Path;

use rre_core::edge::{BodyKind, EdgeBundle, RequestFacts};

#[derive(serde::Deserialize)]
struct FactsFile {
    #[serde(default)]
    headers: HashMap<String, String>,
    path: String,
    #[serde(default)]
    cookies: HashMap<String, String>,
    #[serde(default)]
    site: Option<String>,
    #[serde(default)]
    logged_in: bool,
    #[serde(default)]
    products: Vec<String>,
}

fn run_case(dir: &Path) {
    let bundle: EdgeBundle =
        serde_json::from_str(&std::fs::read_to_string(dir.join("bundle.json")).unwrap()).unwrap();
    let f: FactsFile =
        serde_json::from_str(&std::fs::read_to_string(dir.join("facts.json")).unwrap()).unwrap();

    let mut headers = http::HeaderMap::new();
    for (k, v) in &f.headers {
        headers.insert(
            http::HeaderName::from_bytes(k.as_bytes()).unwrap(),
            http::HeaderValue::from_str(v).unwrap(),
        );
    }
    let facts = RequestFacts {
        headers,
        path: f.path,
        cookies: f.cookies,
        site: f.site,
        identity: rre_core::identity::Identity {
            logged_in: f.logged_in,
            products: f.products.into_iter().collect(),
        },
    };

    let (kind, input, expected) = if dir.join("input.html").exists() {
        (BodyKind::Html, dir.join("input.html"), dir.join("expected.html"))
    } else {
        (BodyKind::Json, dir.join("input.json"), dir.join("expected.json"))
    };

    let out = rre_core::apply(
        &bundle,
        &facts,
        kind,
        std::fs::read_to_string(&input).unwrap(),
        &rre_core::default_sanitizer(),
    );
    let want = std::fs::read_to_string(&expected).unwrap();

    if kind == BodyKind::Json {
        let got: serde_json::Value = serde_json::from_str(&out.body).unwrap();
        let want: serde_json::Value = serde_json::from_str(&want).unwrap();
        assert_eq!(got, want, "golden mismatch in {}", dir.display());
    } else {
        assert_eq!(out.body.trim(), want.trim(), "golden mismatch in {}", dir.display());
    }
}

#[test]
fn golden_cases_all_pass() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/golden");
    let mut ran = 0;
    for entry in std::fs::read_dir(&root).expect("tests/golden exists") {
        let dir = entry.unwrap().path();
        if dir.is_dir() {
            run_case(&dir);
            ran += 1;
        }
    }
    assert!(ran >= 6, "expected at least 6 golden cases, ran {ran}");
}
```

- [ ] **Step 2: Run it and watch it fail**

```bash
cd /Users/tahmid/IdeaProjects/zen/rre-core
PATH="$HOME/.cargo/bin:$PATH" cargo test --test golden
```

Expected: FAIL — `tests/golden` does not exist.

- [ ] **Step 3: Author the six cases**

Create one directory each under `rre-core/tests/golden/`:

| Case | Exercises |
|---|---|
| `json_set_and_remove` | two expression nodes folding over one JSON body in trace order |
| `json_trim_body` | `trim_json` cutting an article `body` array to a teaser |
| `html_injection_append` | `html_injection` at a CSS selector with `placement_mode: append` |
| `html_truncate_with_fade` | `content_truncation` with the fade-out `<style>` injected once |
| `identity_has_product` | a `has_product` decision routing Yes for `facts.products = ["ifcofa"]` and No without it (two cases: `identity_has_product` and `identity_no_product`) |
| `applicability_miss` | a `json_selector` that does not match, so the body is returned byte-identical |

Each `bundle.json` is a full `EdgeBundle` with `schema_version: 1`. For the two identity cases the same bundle appears in both directories with different `facts.json` — that is intentional duplication so a case is self-contained.

- [ ] **Step 4: Run until green**

```bash
PATH="$HOME/.cargo/bin:$PATH" cargo test --test golden
```

Expected: PASS, `ran >= 6`.

- [ ] **Step 5: Route the proxy's loop through `rre_core::apply`**

In `forwarder.rs`, replace the bodies of `apply_features_html` and `apply_features_json` so that, after the async work they must keep (fetching each feature's active version, resolving component and saved-outcome refs through their caches), they build an in-memory `EdgeBundle` from the fetched `ActiveVersionRead`s and call `rre_core::apply` once. Keep the proxy's per-node timing, metrics and telemetry by reading `ApplyOutcome::features`.

If the per-node timing granularity cannot be preserved through `apply` (the proxy times each expression node individually, `apply` reports per feature), keep the proxy's own loop and instead add a test asserting the two produce identical bodies for every golden case. Record which route was taken in the commit message. Do not silently drop the timing telemetry — it is what `window.rre` and the Prometheus histograms report.

- [ ] **Step 6: Full gate**

```bash
cd /Users/tahmid/IdeaProjects/zen
PATH="$HOME/.cargo/bin:$PATH" make rre-core-check
cd proxy && PATH="$HOME/.cargo/bin:$PATH" cargo test
```

Expected: both PASS.

- [ ] **Step 7: Commit**

```bash
git add -A rre-core proxy
git commit -m "test(rre-core): add golden fixtures as the parity contract

Claude-Session: https://claude.ai/code/session_01BwARVnYgVWNnHc1WUSu6Dw"
```

---

## Task 7: Backend edge-bundle service

**Files:**
- Create: `ZEN/backend/src/services/edge_bundle_service.rs`, `ZEN/backend/src/schemas/edge_bundle.rs`
- Modify: `ZEN/backend/src/services/mod.rs`, `ZEN/backend/src/schemas/mod.rs`

**Interfaces:**
- Consumes: existing `version_service` active-version loading, `component_template_service` resolve, `feature_service` listing, `site_service` lookup, `saved_outcome` repository.
- Produces: `edge_bundle_service::build(pool, site_slug, env) -> Result<EdgeBundle, AppError>` and `schemas::edge_bundle::{EdgeBundle, EdgeSite, EdgeFeature}` deserialising to exactly the same JSON as `rre_core::edge::EdgeBundle`.

- [ ] **Step 1: Write the failing service test**

Add to `ZEN/backend/src/services/edge_bundle_service.rs` (the backend's tests use a live Postgres via `DOCKER_HOST`; follow the pattern in `version_service.rs`'s tests):

```rust
#[cfg(test)]
mod tests {
    use super::*;

    /// The bundle must list features in (type, execution_order) order and carry
    /// each one's live version. Ordering is the contract the edge relies on: it
    /// runs `features` in array order and does not re-sort.
    #[sqlx::test]
    async fn build_orders_features_and_embeds_live_versions(pool: PgPool) {
        seed_site(&pool, "intrafish-com", "test.intrafish.com").await;
        seed_published_feature(&pool, "b-html", "html", 2).await;
        seed_published_feature(&pool, "a-html", "html", 1).await;
        seed_published_feature(&pool, "c-json", "json", 1).await;

        let bundle = build(&pool, "intrafish-com", Env::Live).await.unwrap();

        assert_eq!(bundle.schema_version, 1);
        assert_eq!(bundle.site.source_host, "test.intrafish.com");
        let ids: Vec<&str> = bundle.features.iter().map(|f| f.id.as_str()).collect();
        assert_eq!(ids, vec!["a-html", "b-html", "c-json"]);
    }

    /// A feature with no published version for the environment is omitted
    /// entirely rather than exported with an empty canvas.
    #[sqlx::test]
    async fn build_omits_unpublished_features(pool: PgPool) {
        seed_site(&pool, "intrafish-com", "test.intrafish.com").await;
        seed_draft_feature(&pool, "draft-only", "json", 1).await;

        let bundle = build(&pool, "intrafish-com", Env::Live).await.unwrap();
        assert!(bundle.features.is_empty());
    }

    /// Every component_ref the canvas can reach must be resolved into
    /// `resolved_components`, because the edge cannot fetch one.
    #[sqlx::test]
    async fn build_resolves_component_refs_inline(pool: PgPool) {
        seed_site(&pool, "intrafish-com", "test.intrafish.com").await;
        let cid = seed_component_template(&pool, "paywall-box", "<div>{{title}}</div>").await;
        seed_feature_with_component_ref(&pool, "paywall", "html", 1, cid).await;

        let bundle = build(&pool, "intrafish-com", Env::Live).await.unwrap();
        let f = &bundle.features[0];
        let key = format!("{cid}|default");
        assert!(f.resolved_components.contains_key(&key), "keys: {:?}", f.resolved_components.keys());
        assert!(f.resolved_components[&key].html_body.contains("{{title}}"));
    }

    /// An unknown site is a 404, not an empty bundle — an operator exporting a
    /// typo'd slug must find out at export time, not at the edge.
    #[sqlx::test]
    async fn build_rejects_an_unknown_site(pool: PgPool) {
        let err = build(&pool, "nope", Env::Live).await.unwrap_err();
        assert!(matches!(err, AppError::NotFound { .. }), "got {err:?}");
    }
}
```

- [ ] **Step 2: Run and watch it fail**

```bash
cd /Users/tahmid/IdeaProjects/zen/backend
DOCKER_HOST="unix://$HOME/.colima/default/docker.sock" PATH="$HOME/.cargo/bin:$PATH" cargo test edge_bundle
```

Expected: FAIL to compile — `build` does not exist. (If `DOCKER_HOST` differs on this machine, use whatever the other backend tests need; the value above matches this repo's documented setup.)

- [ ] **Step 3: Implement the schema and service**

`schemas/edge_bundle.rs` mirrors `rre_core::edge::EdgeBundle` field for field, deriving `Serialize`, `Deserialize` and `ToSchema`. It reuses the backend's existing `RuleGraph`, `Applicability` and active-outcome schemas rather than redefining them.

`services/edge_bundle_service.rs`:

```rust
//! Builds the edge bundle: everything a host needs to evaluate one site's
//! published rules with no further I/O. Component templates and saved outcomes
//! are resolved INLINE here, because the consumer (a Fastly Compute service)
//! cannot call back.

pub async fn build(pool: &PgPool, site_slug: &str, env: Env) -> Result<EdgeBundle, AppError> {
    let site = site_service::get(pool, site_slug).await?;      // 404s on miss
    let features = feature_service::list_ordered(pool).await?; // (type, execution_order)

    let mut out = Vec::new();
    for feature in features {
        let Some(av) = version_service::active_version(pool, &feature.id, env).await? else {
            continue;
        };
        let (resolved_components, saved_outcomes) =
            resolve_refs(pool, &av).await?;
        out.push(EdgeFeature {
            id: feature.id,
            r#type: feature.r#type.as_str().to_string(),
            execution_order: feature.execution_order,
            version_number: av.version_number,
            applicability: av.applicability,
            rule_graph: av.rule_graph,
            outcomes: av.outcomes,
            resolved_components,
            saved_outcomes,
        });
    }

    Ok(EdgeBundle {
        schema_version: 1,
        site: EdgeSite { slug: site.slug, source_host: site.source_host },
        environment: env.as_str().to_string(),
        generated_at: OffsetDateTime::now_utc().format(&Rfc3339)?,
        features: out,
    })
}
```

`resolve_refs` walks EVERY expression node in the canvas plus every component of every outcome — statically, not per-request — collecting `component_ref` keys (`"<uuid>|default"` / `"<uuid>|N"`) and `apply_saved_outcome` ids, then resolves each exactly once. Reuse the proxy's `applier::component_ref::config_ref` and `json_apply::component_ref` helpers via `rre_core` so the two agree on what counts as a reference; add `rre-core = { path = "../rre-core" }` to `backend/Cargo.toml` for that.

- [ ] **Step 4: Run until green**

```bash
DOCKER_HOST="unix://$HOME/.colima/default/docker.sock" PATH="$HOME/.cargo/bin:$PATH" cargo test edge_bundle
```

Expected: all four tests PASS.

- [ ] **Step 5: Commit**

```bash
cd /Users/tahmid/IdeaProjects/zen
git add -A backend
git commit -m "feat(backend): build edge bundles with refs resolved inline

Claude-Session: https://claude.ai/code/session_01BwARVnYgVWNnHc1WUSu6Dw"
```

---

## Task 8: Export route and CLI

**Files:**
- Modify: `ZEN/backend/src/api/v1/sites.rs`, `ZEN/backend/src/openapi.rs`, `ZEN/CONTRACTS.md`
- Create: `ZEN/backend/src/bin/export_bundle.rs`

**Interfaces:**
- Consumes: `edge_bundle_service::build` from Task 7.
- Produces: `GET /api/v1/sites/{slug}/edge-bundle?env=live|staging` returning the bundle JSON, and `cargo run --bin export_bundle -- --site <slug> --env live` writing the same JSON to stdout.

- [ ] **Step 1: Write the failing route test**

Follow the existing route-test pattern in `api/v1/sites.rs`:

```rust
/// The route is what the export script calls; it must return the bundle
/// verbatim with a JSON content type.
#[sqlx::test]
async fn get_edge_bundle_returns_the_bundle(pool: PgPool) {
    let app = test_app(pool.clone()).await;
    seed_site(&pool, "intrafish-com", "test.intrafish.com").await;

    let res = app
        .oneshot(Request::get("/api/v1/sites/intrafish-com/edge-bundle?env=live").body(Body::empty()).unwrap())
        .await
        .unwrap();

    assert_eq!(res.status(), StatusCode::OK);
    let body: serde_json::Value = json_body(res).await;
    assert_eq!(body["schema_version"], 1);
    assert_eq!(body["site"]["source_host"], "test.intrafish.com");
}

#[sqlx::test]
async fn get_edge_bundle_rejects_a_bad_env(pool: PgPool) {
    let app = test_app(pool).await;
    let res = app
        .oneshot(Request::get("/api/v1/sites/x/edge-bundle?env=prod").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::BAD_REQUEST);
}
```

- [ ] **Step 2: Run and watch it fail**

```bash
cd /Users/tahmid/IdeaProjects/zen/backend
DOCKER_HOST="unix://$HOME/.colima/default/docker.sock" PATH="$HOME/.cargo/bin:$PATH" cargo test get_edge_bundle
```

Expected: FAIL — 404, the route does not exist.

- [ ] **Step 3: Add the route, the CLI and the docs**

Route in `api/v1/sites.rs`, registered next to the other site routes, delegating to `edge_bundle_service::build` and mapping its `AppError` the way every other handler does. Add it to `openapi.rs`.

`backend/src/bin/export_bundle.rs`:

```rust
//! Offline edge-bundle export. Same service the HTTP route uses, for when the
//! backend's port is not reachable from where the export runs.
//!
//!   cargo run --bin export_bundle -- --site intrafish-com --env live > bundle.json

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut site = None;
    let mut env = "live".to_string();
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--site" => site = args.next(),
            "--env" => env = args.next().unwrap_or_else(|| "live".into()),
            other => return Err(format!("unknown argument: {other}").into()),
        }
    }
    let site = site.ok_or("--site is required")?;
    let env = Env::parse(&env).ok_or("--env must be live or staging")?;

    let settings = rre_backend::config::Settings::load()?;
    let pool = rre_backend::db::connect(&settings).await?;
    let bundle = rre_backend::services::edge_bundle_service::build(&pool, &site, env).await?;
    println!("{}", serde_json::to_string_pretty(&bundle)?);
    Ok(())
}
```

Add the route and the bundle schema to `CONTRACTS.md` in the same commit: the route line in §7's route table, the `EdgeBundle` / `EdgeSite` / `EdgeFeature` structs beside the other schema mirrors, and a sentence stating the `(type, execution_order)` sort is part of the contract.

- [ ] **Step 4: Run the tests and the CLI**

```bash
DOCKER_HOST="unix://$HOME/.colima/default/docker.sock" PATH="$HOME/.cargo/bin:$PATH" cargo test
PATH="$HOME/.cargo/bin:$PATH" cargo run --bin export_bundle -- --site demo-site --env live | head -20
```

Expected: tests PASS; the CLI prints a bundle (run `make up` or `cargo run --bin seed_demo` first if no site is seeded).

- [ ] **Step 5: Commit**

```bash
cd /Users/tahmid/IdeaProjects/zen
git add -A backend CONTRACTS.md
git commit -m "feat(backend): expose edge-bundle export via route and CLI

Claude-Session: https://claude.ai/code/session_01BwARVnYgVWNnHc1WUSu6Dw"
```

---

## Task 9: Fastly — move the Zephr flow behind a flow switch

**Repo: `FASTLY`.** Branch `feat/rre-edge-rules` off `development/test`.

**Files:**
- Create: `FASTLY/compute/intrafish-edge/src/flows/mod.rs`, `FASTLY/compute/intrafish-edge/src/flows/zephr/mod.rs`
- Move: `src/{attributes.rs,paywall_gate.rs,publication.rs,rewrite.rs,html_rewrite.rs,zephr.rs}` → `src/flows/zephr/`
- Modify: `src/main.rs`, `src/routing.rs`

**Interfaces:**
- Produces:
  - `flows::Flow` (`Zephr`, `RreExport`, `RreKv`) with `Flow::as_str()`
  - `flows::select(req: &Request) -> Flow`
  - `flows::FLOW_HEADER = "x-use-custom-rre"`, `flows::FLOW_COOKIE = "x_use_custom_rre"`
  - `flows::cookies(req: &Request) -> HashMap<String, String>`
  - `flows::zephr::apply(res, is_get, cfg, ctx, public_host, auth_template) -> Result<Response, Error>` — today's `html_rewrite::maybe_truncate_teaser` + `paywall_gate::apply`, moved.

- [ ] **Step 1: Write the failing flow-selection tests**

Create `src/flows/mod.rs` with tests first (this module is free of `fastly::` types apart from `select`, following the repo's existing host-testable pattern):

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flow_from_value_maps_every_documented_value() {
        assert_eq!(flow_from_value("true"), Some(Flow::RreExport));
        assert_eq!(flow_from_value("1"), Some(Flow::RreExport));
        assert_eq!(flow_from_value("export"), Some(Flow::RreExport));
        assert_eq!(flow_from_value("kv"), Some(Flow::RreKv));
    }

    #[test]
    fn flow_from_value_is_case_and_whitespace_insensitive() {
        assert_eq!(flow_from_value("  TRUE  "), Some(Flow::RreExport));
        assert_eq!(flow_from_value("Kv"), Some(Flow::RreKv));
    }

    /// An unknown value is the default flow, never an error: a typo in an A/B
    /// harness must serve the reader the current production path.
    #[test]
    fn flow_from_value_falls_back_to_zephr() {
        assert_eq!(flow_from_value("false"), None);
        assert_eq!(flow_from_value(""), None);
        assert_eq!(flow_from_value("rre"), None);
    }

    #[test]
    fn parse_cookies_reads_the_switch_cookie() {
        let c = parse_cookies("a=1; x_use_custom_rre=kv; b=2");
        assert_eq!(c.get("x_use_custom_rre").map(String::as_str), Some("kv"));
        assert_eq!(c.get("a").map(String::as_str), Some("1"));
    }

    #[test]
    fn parse_cookies_tolerates_junk() {
        let c = parse_cookies("; =; noequals; x_use_custom_rre=true ;;");
        assert_eq!(c.get("x_use_custom_rre").map(String::as_str), Some("true"));
    }

    #[test]
    fn select_from_parts_prefers_the_header_over_the_cookie() {
        assert_eq!(select_from_parts(Some("kv"), Some("true")), Flow::RreKv);
        assert_eq!(select_from_parts(None, Some("true")), Flow::RreExport);
        assert_eq!(select_from_parts(None, None), Flow::Zephr);
    }
}
```

- [ ] **Step 2: Run and watch it fail**

```bash
cd /Users/tahmid/IdeaProjects/dngroup-fastly/compute/intrafish-edge
PATH="$HOME/.cargo/bin:$PATH" cargo test flows::
```

Expected: FAIL to compile.

- [ ] **Step 3: Implement `flows/mod.rs`**

```rust
//! Which rules engine handles this request.
//!
//! Three flows live side by side so the RRE edge engine can be A/B tested
//! against the current Zephr path on the same host, per request. `zephr` is
//! the default and is byte-identical to what shipped before this module
//! existed; the two RRE flows differ only in where their bundle comes from.

use std::collections::HashMap;

use fastly::http::header;
use fastly::Request;

pub mod rre_common;
pub mod rre_export;
pub mod rre_kv;
pub mod zephr;

/// Client-settable switch. Deliberately client-controlled: choosing the engine
/// for your OWN response is the point. It never affects origin routing, the
/// cache key or the auth proxy, and it is stripped before the Varnish fetch.
pub const FLOW_HEADER: &str = "x-use-custom-rre";
pub const FLOW_COOKIE: &str = "x_use_custom_rre";

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Flow { Zephr, RreExport, RreKv }

impl Flow {
    pub fn as_str(self) -> &'static str {
        match self {
            Flow::Zephr => "zephr",
            Flow::RreExport => "rre_export",
            Flow::RreKv => "rre_kv",
        }
    }
}

pub fn select(req: &Request) -> Flow {
    let header = req.get_header_str(FLOW_HEADER).map(str::to_owned);
    let cookie = req
        .get_header_str(header::COOKIE)
        .map(parse_cookies)
        .and_then(|c| c.get(FLOW_COOKIE).cloned());
    select_from_parts(header.as_deref(), cookie.as_deref())
}

fn select_from_parts(header: Option<&str>, cookie: Option<&str>) -> Flow {
    header
        .and_then(flow_from_value)
        .or_else(|| cookie.and_then(flow_from_value))
        .unwrap_or(Flow::Zephr)
}

/// `None` means "not a recognised value" and the caller falls back to Zephr.
fn flow_from_value(raw: &str) -> Option<Flow> {
    match raw.trim().to_ascii_lowercase().as_str() {
        "true" | "1" | "export" => Some(Flow::RreExport),
        "kv" => Some(Flow::RreKv),
        _ => None,
    }
}

/// Minimal `Cookie` header parse. No dependency: split on `;`, then on the
/// FIRST `=` so a value containing `=` survives. Malformed pairs are dropped.
pub fn parse_cookies(raw: &str) -> HashMap<String, String> {
    raw.split(';')
        .filter_map(|pair| {
            let (name, value) = pair.split_once('=')?;
            let name = name.trim();
            if name.is_empty() { return None; }
            Some((name.to_owned(), value.trim().to_owned()))
        })
        .collect()
}
```

- [ ] **Step 4: Move the Zephr files verbatim**

```bash
cd /Users/tahmid/IdeaProjects/dngroup-fastly/compute/intrafish-edge
mkdir -p src/flows/zephr
for f in attributes paywall_gate publication rewrite html_rewrite zephr; do
  git mv src/$f.rs src/flows/zephr/$f.rs
done
```

Fix the `use crate::{...}` paths inside them to `use crate::flows::zephr::{...}` (they cross-reference each other and `routing`, `headers`, `cache_fetch`). Write `src/flows/zephr/mod.rs`:

```rust
//! The Zephr flow: today's behaviour, moved verbatim behind the flow switch.
//! Nothing in this directory changed when the RRE flows were added, and
//! nothing in it should change to serve them.

pub mod attributes;
pub mod html_rewrite;
pub mod paywall_gate;
pub mod publication;
pub mod rewrite;
pub mod zephr;

use fastly::{Error, Request, Response};

/// Steps 7 and 8 of the pre-flow pipeline: unconditional teaser truncation on
/// HTML, then the paywall gate on article JSON.
pub fn apply(
    mut res: Response,
    is_get: bool,
    cfg: Option<&publication::PublicationConfig>,
    ctx: &attributes::RequestContext,
    public_host: &str,
    auth_template: Option<Request>,
) -> Result<Response, Error> {
    html_rewrite::maybe_truncate_teaser(&mut res);
    paywall_gate::apply(res, is_get, cfg, ctx, public_host, auth_template)
}
```

- [ ] **Step 5: Wire `routing.rs` to select and dispatch**

In `route_inner`, after `try_zephr_auth_proxy` and before the `auth_template` clone: `let flow = flows::select(&req);` then `req.remove_header(flows::FLOW_HEADER);`. Replace the current steps 7–8 with a `match flow` whose `Flow::Zephr` arm calls `flows::zephr::apply(...)` with exactly today's arguments and whose other two arms are `todo!()` for now (Tasks 10–12 fill them). Set `x-rre-flow` on the response before returning.

- [ ] **Step 6: Run the full suite**

```bash
PATH="$HOME/.cargo/bin:$PATH" cargo test
PATH="$HOME/.cargo/bin:$PATH" cargo build --target wasm32-wasip1
```

Expected: every pre-existing Zephr test still passes from its new location, plus the six new flow tests. The `todo!()` arms are unreachable because nothing selects them yet — but they will panic if reached, so do NOT deploy from this commit.

- [ ] **Step 7: Commit**

```bash
git add -A src
git commit -m "refactor(compute): move zephr behind a per-request flow switch

Claude-Session: https://claude.ai/code/session_01BwARVnYgVWNnHc1WUSu6Dw"
```

---

## Task 10: Vendor `rre-core` and build the shared RRE flow

**Repo: `FASTLY`.**

**Files:**
- Create: `FASTLY/compute/intrafish-edge/vendor/rre-core/` (synced copy), `FASTLY/compute/intrafish-edge/vendor/RRE_CORE_SOURCE`
- Create: `src/flows/rre_common/{mod.rs,facts.rs,headers.rs}`
- Modify: `compute/intrafish-edge/Cargo.toml`

**Interfaces:**
- Consumes: `rre_core::{apply, default_sanitizer, edge::*, identity::*}` from Tasks 1–4.
- Produces:
  - `rre_common::facts::build(req: &Request, site: &str) -> rre_core::edge::RequestFacts`
  - `rre_common::IDENTITY: rre_core::identity::IdentitySettings`
  - `rre_common::apply_with_bundle(res, is_get, bundle, facts, flow) -> Result<Response, Error>`
  - `rre_common::KEEP_PAYWALL_PROBE: bool`

- [ ] **Step 1: Vendor the crate**

The two repos have unrelated remotes (`Tahmid12Khan/cdn-rule-canvas` and `nhst/dngroup-fastly`), so a git dependency would need cross-org credentials in the deploy workflow. Vendor instead — the sync skill in Task 13 keeps it fresh and stamps its provenance.

```bash
cd /Users/tahmid/IdeaProjects/dngroup-fastly/compute/intrafish-edge
mkdir -p vendor
rsync -a --delete \
  --exclude target --exclude tests \
  /Users/tahmid/IdeaProjects/zen/rre-core/ vendor/rre-core/
mkdir -p vendor/rre-core/../
( cd /Users/tahmid/IdeaProjects/zen && git rev-parse HEAD ) > vendor/RRE_CORE_SOURCE
```

`rre-core` depends on `../core/engine` and `../core/expression` by path, so those must be vendored too:

```bash
rsync -a --delete --exclude target \
  /Users/tahmid/IdeaProjects/zen/core/ vendor/core/
```

and the vendored `vendor/rre-core/Cargo.toml` keeps its `path = "../core/engine"` references working, since `vendor/core/` sits beside `vendor/rre-core/`.

Add to `compute/intrafish-edge/Cargo.toml`:

```toml
# Vendored from the RRE repo; see vendor/RRE_CORE_SOURCE for the source commit
# and run the `sync-rules-to-fastly` skill to refresh. Vendored rather than a
# git dep because the two repos live in different GitHub orgs.
rre-core = { path = "vendor/rre-core" }
```

- [ ] **Step 2: Write the failing facts test**

In `src/flows/rre_common/facts.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    /// Identity is the whole personalisation contract at the edge: a cookie the
    /// site sets, with a header fallback for testing with curl.
    #[test]
    fn identity_reads_the_products_cookie() {
        let cookies = super::super::parse_cookies("rre_user=u1; rre_products=IFCOFA,IFNOFA");
        let id = rre_core::identity::resolve(&http::HeaderMap::new(), &cookies, &IDENTITY);
        assert!(id.logged_in);
        assert!(id.products.contains("ifcofa"), "labels are lowercased: {:?}", id.products);
        assert!(id.products.contains("ifnofa"));
    }

    #[test]
    fn identity_defaults_to_logged_out() {
        let id = rre_core::identity::resolve(
            &http::HeaderMap::new(),
            &std::collections::HashMap::new(),
            &IDENTITY,
        );
        assert!(!id.logged_in);
        assert!(id.products.is_empty());
    }

    /// The switch header must never reach a rule: it is transport, not a fact
    /// about the reader, and leaving it in would let a rule branch on it.
    #[test]
    fn switch_header_is_excluded_from_facts() {
        let mut headers = http::HeaderMap::new();
        headers.insert("x-use-custom-rre", http::HeaderValue::from_static("true"));
        headers.insert("user-agent", http::HeaderValue::from_static("curl/8"));
        let out = strip_switch_header(headers);
        assert!(!out.contains_key("x-use-custom-rre"));
        assert!(out.contains_key("user-agent"));
    }
}
```

- [ ] **Step 3: Run and watch it fail**

```bash
PATH="$HOME/.cargo/bin:$PATH" cargo test rre_common
```

Expected: FAIL to compile.

- [ ] **Step 4: Implement `rre_common`**

`facts.rs` builds an `http::HeaderMap` from the Fastly `Request`, strips the switch header, parses cookies with `flows::parse_cookies`, resolves identity, and returns `RequestFacts`. `IDENTITY` is:

```rust
/// The cookie and header names that carry visitor identity. These are the
/// contract between whatever knows the reader (the site's login) and the rules:
/// state it in a cookie and the edge branches on it with no network call.
pub const IDENTITY: rre_core::identity::IdentitySettings = /* rre_user / rre_products / x-rre-user / x-rre-products */;
```

(If `IdentitySettings` holds `String` fields it cannot be a `const`; use a `fn identity_settings() -> IdentitySettings` instead and name the four values as `&str` consts.)

`mod.rs` holds `KEEP_PAYWALL_PROBE`, the content-type gate, the call into `rre_core::apply`, the `x-rre-*` headers and the log line, per the Fastly spec's "rre_common" section. `headers.rs` holds the header names and the setter.

- [ ] **Step 5: Run until green, including the wasm build**

```bash
PATH="$HOME/.cargo/bin:$PATH" cargo test
PATH="$HOME/.cargo/bin:$PATH" cargo build --target wasm32-wasip1
```

Expected: PASS. The wasm build is the real proof that `rre-core` runs in Compute.

- [ ] **Step 6: Commit**

```bash
git add -A vendor src Cargo.toml Cargo.lock
git commit -m "feat(compute): vendor rre-core and add the shared RRE flow

Claude-Session: https://claude.ai/code/session_01BwARVnYgVWNnHc1WUSu6Dw"
```

---

## Task 11: The `rre_export` flow and the exported bundle

**Repo: `FASTLY`.**

**Files:**
- Create: `src/flows/rre_export/mod.rs`, `rules/README.md`, `rules/test.intrafish.com.live.json`, `scripts/export-rules.sh`
- Modify: `src/routing.rs`

**Interfaces:**
- Consumes: `rre_common::apply_with_bundle`, `rre_core::edge::EdgeBundle`.
- Produces: `rre_export::load(host: &str) -> Result<EdgeBundle, LoadError>` and `rre_export::apply(...)`.

- [ ] **Step 1: Write the failing bundle test**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    /// Every committed bundle must parse. `include_str!` only proves the file
    /// exists; this proves it is a bundle this binary understands, so a bad
    /// export fails `cargo test` instead of failing open at the edge.
    #[test]
    fn every_committed_bundle_parses() {
        for (host, raw) in BUNDLES {
            let bundle: rre_core::edge::EdgeBundle =
                serde_json::from_str(raw).unwrap_or_else(|e| panic!("{host}: {e}"));
            assert_eq!(bundle.schema_version, rre_core::edge::SCHEMA_VERSION, "{host}");
            assert_eq!(&bundle.site.source_host, host, "filename must match site.source_host");
        }
    }

    #[test]
    fn load_returns_the_bundle_for_a_known_host() {
        assert!(load("test.intrafish.com").is_ok());
    }

    /// An unknown host is an error, which the caller turns into fail-open
    /// pass-through — the same shape as a host with no Zephr publication row.
    #[test]
    fn load_rejects_an_unknown_host() {
        assert!(load("www.example.com").is_err());
    }
}
```

- [ ] **Step 2: Run and watch it fail**

```bash
PATH="$HOME/.cargo/bin:$PATH" cargo test rre_export
```

Expected: FAIL — no `rules/` file and no `load`.

- [ ] **Step 3: Write the export script and produce a real bundle**

`scripts/export-rules.sh`:

```bash
#!/usr/bin/env bash
# Export a site's published RRE rules into rules/<host>.<env>.json.
#
# The edge NEVER calls RRE: this runs on an operator's machine, the result is
# committed, and `include_str!` bakes it into the Wasm package at build time.
#
#   ./scripts/export-rules.sh test.intrafish.com live http://localhost:8000
set -euo pipefail

HOST="${1:?usage: export-rules.sh <source_host> [env] [backend_base_url]}"
ENV="${2:-live}"
BACKEND="${3:-http://localhost:8000}"

SLUG="$(curl -fsS "$BACKEND/api/v1/sites?page_size=100" \
  | jq -r --arg h "$HOST" '.items[] | select(.source_host == $h) | .slug')"
[ -n "$SLUG" ] || { echo "no Site in RRE with source_host=$HOST" >&2; exit 1; }

OUT="$(dirname "$0")/../rules/$HOST.$ENV.json"
curl -fsS "$BACKEND/api/v1/sites/$SLUG/edge-bundle?env=$ENV" | jq . > "$OUT"
echo "wrote $OUT ($(jq '.features | length' "$OUT") features)"
```

Then run the RRE stack and export for real:

```bash
cd /Users/tahmid/IdeaProjects/zen && make dev --seed   # or: docker compose up + cargo run
cd /Users/tahmid/IdeaProjects/dngroup-fastly/compute/intrafish-edge
chmod +x scripts/export-rules.sh
./scripts/export-rules.sh test.intrafish.com live http://localhost:8000
```

If no Site with `source_host = test.intrafish.com` exists in RRE yet, create one through the dashboard (Sites page) with at least one published feature, then re-run. A bundle with zero features is valid but proves nothing — the committed fixture must contain at least one published feature.

- [ ] **Step 4: Implement `load` and the module**

```rust
//! The `rre_export` flow: rules committed into this repo and compiled into the
//! Wasm package. No network call, no KV, no dependency on the RRE backend
//! being reachable — the bundle is bytes in the binary.

/// Committed bundles, keyed by the site's public host. Add a host by exporting
/// it (scripts/export-rules.sh) and adding a line here.
const BUNDLES: &[(&str, &str)] = &[(
    "test.intrafish.com",
    include_str!("../../../rules/test.intrafish.com.live.json"),
)];
```

`load(host)` finds the entry, parses it, and returns a `LoadError` for an unknown host or a parse failure. `apply(...)` calls `load` then `rre_common::apply_with_bundle`, and on a `LoadError` returns the untouched response with `x-rre-status: error` plus a log line.

Fill the `Flow::RreExport` arm in `routing.rs`.

- [ ] **Step 5: Run until green**

```bash
PATH="$HOME/.cargo/bin:$PATH" cargo test
PATH="$HOME/.cargo/bin:$PATH" cargo build --target wasm32-wasip1
```

Expected: PASS.

- [ ] **Step 6: Write `rules/README.md`**

State: never hand-edit these files; regenerate with `scripts/export-rules.sh`; the filename must equal `site.source_host` plus the environment; a new host needs a line in `BUNDLES`; and the HTML-truncation caveat from the spec — the RRE flows do not unconditionally truncate `#dn-content-ssr`, so a bundle used on a real-reader host needs an html feature that does.

- [ ] **Step 7: Commit**

```bash
git add -A src rules scripts Cargo.toml
git commit -m "feat(compute): add rre_export flow with a committed bundle

Claude-Session: https://claude.ai/code/session_01BwARVnYgVWNnHc1WUSu6Dw"
```

---

## Task 12: The `rre_kv` flow

**Repo: `FASTLY`.**

**Files:**
- Create: `src/flows/rre_kv/mod.rs`
- Modify: `src/routing.rs`, `fastly.toml`

**Interfaces:**
- Consumes: `rre_common::apply_with_bundle`.
- Produces: `rre_kv::{STORE_NAME, key_for(host, env), load(host), apply(...)}`.

- [ ] **Step 1: Write the failing key test**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    /// The key layout is the contract with whatever publishes to KV later
    /// (the RRE backend's outbox). Freeze it now so the publisher can be
    /// written against it.
    #[test]
    fn key_for_is_host_slash_env() {
        assert_eq!(key_for("test.intrafish.com", "live"), "test.intrafish.com/live");
    }
}
```

- [ ] **Step 2: Run and watch it fail**

```bash
PATH="$HOME/.cargo/bin:$PATH" cargo test rre_kv
```

Expected: FAIL to compile.

- [ ] **Step 3: Implement the flow**

```rust
//! The `rre_kv` flow: the same bundle, read from a Fastly KV Store at request
//! time instead of being compiled in. A KV read is served from the local POP,
//! so this is still no network hop in the sense that matters — but it lets
//! rules be republished without a Compute deploy.
//!
//! NO KV Store is provisioned yet (that is a Terraform change, step 5). Until
//! one is linked, `load` fails and the flow falls open to the untouched
//! upstream response with `x-rre-status: error`. That is deliberate: the code
//! path is complete and selectable so it can be verified the day the store
//! exists, without another Compute release.

pub const STORE_NAME: &str = "rre_rules";
pub const ENVIRONMENT: &str = "live";

pub fn key_for(host: &str, env: &str) -> String {
    format!("{host}/{env}")
}
```

`load(host)` opens the store, looks up `key_for(host, ENVIRONMENT)`, and parses. Every failure — store not linked, key absent, bad JSON, wrong `schema_version` — is a `LoadError`. `apply(...)` mirrors `rre_export::apply`. Fill the `Flow::RreKv` arm in `routing.rs` and delete the last `todo!()`.

Add the local-dev store to `fastly.toml` so `fastly compute serve` can exercise the path:

```toml
[local_server.kv_stores]
rre_rules = [{ key = "test.intrafish.com/live", path = "rules/test.intrafish.com.live.json" }]
```

- [ ] **Step 4: Run until green**

```bash
PATH="$HOME/.cargo/bin:$PATH" cargo test
PATH="$HOME/.cargo/bin:$PATH" cargo build --target wasm32-wasip1
```

Expected: PASS, and NO `todo!()` remains in `routing.rs` (`grep -rn 'todo!' src` returns nothing).

- [ ] **Step 5: Commit**

```bash
git add -A src fastly.toml
git commit -m "feat(compute): add rre_kv flow, unprovisioned but selectable

Claude-Session: https://claude.ai/code/session_01BwARVnYgVWNnHc1WUSu6Dw"
```

---

## Task 13: The `sync-rules-to-fastly` skill

**Repo: `ZEN`.**

**Files:**
- Create: `ZEN/.claude/skills/sync-rules-to-fastly/SKILL.md`
- Create: `ZEN/scripts/sync-rules-to-fastly.sh`

**Interfaces:**
- Consumes: the export route/CLI from Task 8 and the vendor layout from Task 10.
- Produces: a skill invoked as `/sync-rules-to-fastly [host] [env]` that refreshes both the exported bundles and the vendored `rre-core`, then verifies the Fastly crate still builds.

- [ ] **Step 1: Write the script**

`ZEN/scripts/sync-rules-to-fastly.sh`:

```bash
#!/usr/bin/env bash
# Sync published RRE rules (and the shared rre-core crate) into the Fastly repo.
#
# Two things travel together and MUST stay in step:
#   1. rules/<host>.<env>.json  — the exported bundle
#   2. vendor/rre-core + vendor/core — the code that evaluates it
# A bundle exported by a newer backend than the vendored crate understands is
# exactly the skew this script exists to prevent, which is why it re-runs the
# Fastly crate's tests at the end.
set -euo pipefail

ZEN="${ZEN_DIR:-$HOME/IdeaProjects/zen}"
FASTLY="${FASTLY_DIR:-$HOME/IdeaProjects/dngroup-fastly}"
EDGE="$FASTLY/compute/intrafish-edge"
HOST="${1:-test.intrafish.com}"
ENV="${2:-live}"
BACKEND="${RRE_BACKEND:-http://localhost:8000}"
export PATH="$HOME/.cargo/bin:$PATH"

[ -d "$EDGE" ] || { echo "Fastly edge crate not found at $EDGE" >&2; exit 1; }

echo "==> exporting $HOST ($ENV) from $BACKEND"
SLUG="$(curl -fsS "$BACKEND/api/v1/sites?page_size=100" \
  | jq -r --arg h "$HOST" '.items[] | select(.source_host == $h) | .slug')"
[ -n "$SLUG" ] || { echo "no Site with source_host=$HOST in RRE" >&2; exit 1; }
curl -fsS "$BACKEND/api/v1/sites/$SLUG/edge-bundle?env=$ENV" \
  | jq . > "$EDGE/rules/$HOST.$ENV.json"

echo "==> syncing rre-core + core"
rsync -a --delete --exclude target --exclude tests "$ZEN/rre-core/" "$EDGE/vendor/rre-core/"
rsync -a --delete --exclude target "$ZEN/core/" "$EDGE/vendor/core/"
( cd "$ZEN" && git rev-parse HEAD ) > "$EDGE/vendor/RRE_CORE_SOURCE"

echo "==> verifying the Fastly crate"
cd "$EDGE"
cargo test
cargo build --target wasm32-wasip1

echo
echo "Synced $HOST ($ENV), $(jq '.features | length' "rules/$HOST.$ENV.json") features."
echo "rre-core at $(cat vendor/RRE_CORE_SOURCE)"
echo "Review with: git -C $FASTLY diff --stat"
```

- [ ] **Step 2: Write the skill**

`ZEN/.claude/skills/sync-rules-to-fastly/SKILL.md`:

```markdown
---
name: sync-rules-to-fastly
description: Export published RRE rules from this app into the Fastly Compute repo and refresh the vendored rre-core crate. Use when rules have been published and need to reach the edge, when the user says "sync rules to fastly", "push rules to the edge", "export the bundle", "update the edge rules", or after changing anything under rre-core/. Not for deploying — it only updates the Fastly working tree.
---

# Sync rules to Fastly

Rules authored here reach the Fastly edge as a committed JSON bundle plus a
vendored copy of `rre-core`, the crate that evaluates it. Both are refreshed
together, because a bundle and an evaluator that disagree fail at the edge.

## Preconditions

Check each before running; stop and say which one failed rather than guessing.

1. The RRE backend is reachable: `curl -fsS http://localhost:8000/health`.
   If not, start it (`make dev` in the zen repo) or set `RRE_BACKEND`.
2. The site exists in RRE with `source_host` equal to the Fastly host, and has
   at least one PUBLISHED feature. A bundle with zero features is valid JSON
   and silently does nothing at the edge.
3. The Fastly repo is clean enough to review a diff: `git -C ~/IdeaProjects/dngroup-fastly status --short`.

## Run it

```bash
./scripts/sync-rules-to-fastly.sh [host] [env]
```

Defaults: `test.intrafish.com`, `live`. Override the backend with
`RRE_BACKEND=…`, and the repo locations with `ZEN_DIR` / `FASTLY_DIR`.

The script exports the bundle, rsyncs `rre-core` and `core` into
`compute/intrafish-edge/vendor/`, stamps the source commit into
`vendor/RRE_CORE_SOURCE`, then runs the Fastly crate's tests and its
`wasm32-wasip1` build. It stops at the first failure.

## After it runs

1. Read the diff: `git -C ~/IdeaProjects/dngroup-fastly diff --stat`, then look
   at `rules/*.json` specifically. A bundle that lost features, or whose
   `features` array reordered unexpectedly, means someone unpublished or
   changed `execution_order` — confirm that was intended before committing.
2. Report the feature count and the source commit to the user.
3. Commit in the Fastly repo with a conventional message naming the host, the
   environment and the feature count. Do NOT push or deploy unless asked —
   pushing to `master` there applies to production immediately.

## What this does not do

It does not publish rules (do that in the dashboard first), does not provision
or write to a Fastly KV Store, and does not deploy the Compute service. The
bundle only reaches readers after `deploy-compute.yml` runs.
```

- [ ] **Step 3: Run the skill end to end**

```bash
chmod +x /Users/tahmid/IdeaProjects/zen/scripts/sync-rules-to-fastly.sh
cd /Users/tahmid/IdeaProjects/zen && ./scripts/sync-rules-to-fastly.sh test.intrafish.com live
```

Expected: exports, syncs, and both Fastly gates pass. Verify `vendor/RRE_CORE_SOURCE` matches `git -C ~/IdeaProjects/zen rev-parse HEAD`.

- [ ] **Step 4: Commit both repos**

```bash
cd /Users/tahmid/IdeaProjects/zen
git add -A .claude scripts
git commit -m "feat(skill): add sync-rules-to-fastly

Claude-Session: https://claude.ai/code/session_01BwARVnYgVWNnHc1WUSu6Dw"

cd /Users/tahmid/IdeaProjects/dngroup-fastly
git add -A compute/intrafish-edge
git commit -m "chore(compute): sync rre-core and the intrafish bundle

Claude-Session: https://claude.ai/code/session_01BwARVnYgVWNnHc1WUSu6Dw"
```

---

## Task 14: End-to-end verification

**Files:** none created; this task only runs things and records the result.

- [ ] **Step 1: Full gate in `ZEN`**

```bash
cd /Users/tahmid/IdeaProjects/zen
PATH="$HOME/.cargo/bin:$PATH" make check
```

Expected: `rre-core-check`, `backend-check`, `proxy-check` and `frontend-check` all pass. Record the test counts.

- [ ] **Step 2: Full gate in `FASTLY`**

```bash
cd /Users/tahmid/IdeaProjects/dngroup-fastly/compute/intrafish-edge
PATH="$HOME/.cargo/bin:$PATH" cargo fmt --check
PATH="$HOME/.cargo/bin:$PATH" cargo clippy --all-targets -- -D warnings
PATH="$HOME/.cargo/bin:$PATH" cargo test
PATH="$HOME/.cargo/bin:$PATH" cargo build --target wasm32-wasip1
```

Expected: all pass.

- [ ] **Step 3: Run the Compute service locally and exercise all three flows**

```bash
cd /Users/tahmid/IdeaProjects/dngroup-fastly/compute/intrafish-edge
fastly compute serve --skip-verification &
sleep 5
BASE=http://127.0.0.1:7676
ART=/proxy/global/v2/content/2-1-1742531

# default flow
curl -si -H 'fastly-ff: 1' -H 'x-hostname: test.intrafish.com' "$BASE$ART" | grep -i 'x-rre-flow'
# expect: x-rre-flow: zephr

# export flow, header switch
curl -si -H 'fastly-ff: 1' -H 'x-hostname: test.intrafish.com' \
     -H 'x-use-custom-rre: true' "$BASE$ART" | grep -iE 'x-rre-'
# expect: x-rre-flow: rre_export, x-rre-status: ok, x-rre-bundle present

# export flow, cookie switch
curl -si -H 'fastly-ff: 1' -H 'x-hostname: test.intrafish.com' \
     -H 'cookie: x_use_custom_rre=true' "$BASE$ART" | grep -i 'x-rre-flow'
# expect: rre_export

# kv flow against the local_server store
curl -si -H 'fastly-ff: 1' -H 'x-hostname: test.intrafish.com' \
     -H 'x-use-custom-rre: kv' "$BASE$ART" | grep -iE 'x-rre-'
# expect: x-rre-flow: rre_kv, and x-rre-status: ok locally (the local_server
# store is populated) — it will be `error` in production until the store exists

# identity changes the decision
curl -si -H 'fastly-ff: 1' -H 'x-hostname: test.intrafish.com' \
     -H 'x-use-custom-rre: true' -H 'x-rre-user: u1' -H 'x-rre-products: IFCOFA' \
     "$BASE$ART" | grep -i 'x-rre-identity'
# expect: logged_in=true products=1
```

If the upstream (Varnish) is unreachable locally, point `fastly compute serve` at a local fixture backend or accept a 502 and assert only on the `x-rre-flow` header, which is set regardless. Note in the report which of these ran against a real upstream.

- [ ] **Step 4: Record the outcome**

Write down, for the final report: the test counts per crate, which curl checks passed, and anything that could not be verified locally with the reason.

---

## Self-Review Notes

**Spec coverage.** Every section of both specs maps to a task: `rre-core` extraction (1–5), golden parity (6), export service/route/CLI (7–8), flow switch and the Zephr move (9), `rre_common` and identity (10), `rre_export` plus the bundle and script (11), `rre_kv` (12), the sync skill (13), verification (14).

**Deliberately deferred, matching the specs' non-goals:** the Terraform `fastly_kvstore` + `resource_link`, the publish-time KV outbox, replacing `/auth/user/paywall`, and API auth. `KEEP_PAYWALL_PROBE` ships as `true` so the RRE flows still embed `article.paywall`.

**Known risks carried into implementation:**
- Task 2 verifies `futures::executor::block_on` against zen's tokio dependency and names the fallback.
- Task 6 Step 5 may find the proxy's per-node timing incompatible with a per-feature `apply` report; the fallback keeps the proxy loop and proves equivalence by test instead.
- Task 4 flags that `VersionSelector`'s variant names and `apply_action_json`'s signature must be read from the code, not pasted from this plan.
- Task 10 vendors `rre-core` and `core` rather than using a git dependency, because the repos are in different GitHub orgs. The sync skill is what keeps the copy honest.
