# Task 18 — Proxy Graph Evaluator (zen DecisionEngine) + Processors

## Goal
Inside the proxy runtime, fetch the active version's rule graph for the requested feature, classify the user into a canvas (Anonymous/Registered/Customer), **evaluate the canvas graph with the embedded `zen_engine::DecisionEngine`** against request + response context, and identify which Outcome (if any) should be applied. Implement `MetaTags` and `DeviceType` processors.

We do **not** hand-roll a graph walker. Instead we:
1. **Translate** the canvas `rule_graph` (the JSON schema from Task 10) into a `zen_engine::model::DecisionContent` (JDM) — a "graph → JDM translator".
2. **Evaluate** that `DecisionContent` with `zen_engine::DecisionEngine`. Zen owns the traversal, edge/branch routing, cycle/max-depth protection, and produces the terminal output.
3. **Plug processors** (`metaTags`, `deviceType`, and all FUTURE ones) into zen as **custom nodes**: each processor implements a local `CanvasProcessor` trait, is registered in a `ProcessorRegistry`, and a single `CustomNodeAdapter` implementation dispatches zen's custom-node callbacks to the registry.

Adding a new decision processor is therefore: **implement the `CanvasProcessor` trait + one `registry.register(...)` line.** No changes to the evaluator, adapter, or translator.

## Dependencies
Task 10, 17.

## Acceptance Criteria
- Proxy reads the **feature mapping** from a config file `proxy/config/feature_map.yaml` mapping `host + path glob → feature_id` (one entry for MVP: `localhost:9000` + `/article*` → `dn-article`), parsed via `serde_yaml`.
- For each request the proxy:
  1. Resolves the matching feature (else passes through untouched).
  2. Calls backend `GET /api/v1/features/{id}/active-version?env=live` (new endpoint) which returns the LIVE version's `rule_graph` + outcomes. Result cached in-process via `moka` (LRU + TTL 30s) keyed by `(feature_id, env, version_number)`.
  3. Classifies user canvas using a stub `UserClassifier` (reads a cookie `rre_user_type=anonymous|registered|customer`, default `anonymous` — real auth deferred).
  4. Forwards the request to upstream (Task 17).
  5. Loads the response body if `Content-Type` starts with `text/html`. Other types pass through.
  6. Runs `GraphEvaluator::evaluate(canvas_graph, ctx)` to find the terminal outcome id — internally this **translates the canvas graph to JDM and delegates traversal to `zen_engine::DecisionEngine`** (it is an orchestrator over zen, NOT a hand-written walker).
- **`GraphEvaluator` (orchestrator over zen, not a walker):**
  - `translator::to_decision_content(canvas_graph) -> zen_engine::model::DecisionContent`:
    - Canvas **DecisionNode** (carrying a processor `metaTags` / `deviceType`) → a JDM **`CustomNode`** whose `content.kind` = the processor key (`"metaTags"` / `"deviceType"`) and `content.config` = the processor's serialized config (`serde_json::Value`).
    - Canvas **OutcomeNode** → a JDM node that emits `{ "outcomeId": "<uuid>" }` as the decision output (model as an `OutputNode` fed by an expression/edge, or the simplest JDM construct that surfaces the outcome id in `result`).
    - An **InputNode** seeds the evaluation context (request headers/path/cookies/device + parsed-HTML lookups) so processors can read it via `request.get_field(...)`.
    - Canvas `Edge.branch` (`"yes"`/`"no"`) → JDM `DecisionEdge.source_handle` so zen routes correctly; YES/NO routing is expressed with JDM `SwitchNode`/edge conditions or by the processor returning a branch key consumed by the edge condition. **Zen evaluates the branch** — the proxy never re-implements branch selection.
    - Exact JDM field names/serde renames per `core/types/src/decision/mod.rs` (`DecisionNode { id, name, kind: DecisionNodeKind }`, `DecisionEdge { id, source_id, target_id, source_handle }`, `CustomNodeContent { kind, config }`).
  - Builds the engine once: `let engine = DecisionEngine::default().with_adapter(Arc::new(adapter));` (adapter from `CustomNodeAdapter` impl below). The compiled `DecisionContent` is cached (see compiled-graph cache below); only `decision.evaluate(input)` runs per request.
  - **Cycle / max-depth protection is provided by zen** via `EvaluationOptions { max_depth, .. }` — do not add a second guard.
  - Returns the terminal `outcome_id` parsed out of the zen `DecisionGraphResponse.result` (the `{ outcomeId }` payload), or `None` if the result carries no outcome (dead-end / empty canvas).
  - **`!Send` constraint:** zen's `Variable` is `!Send`; per Task 17, the translate→evaluate→read-result step runs inside `spawn_blocking` on a current-thread runtime, with only `serde_json::Value` crossing the boundary.
- **Processor plug-in mechanism — `CanvasProcessor` trait + `ProcessorRegistry` + one `CustomNodeAdapter`:**
  - All processors implement a single local trait. Document the exact signature:
    ```rust
    /// One decision processor (e.g. metaTags, deviceType). Pure + sync; CPU-only.
    pub trait CanvasProcessor: Send + Sync {
        /// Stable key used as the JDM CustomNode `content.kind` and the registry key.
        fn kind(&self) -> &'static str;

        /// Evaluate this node against the request/response context.
        /// `config`  = the canvas ProcessorConfig as JSON (zen CustomNode `content.config`).
        /// `ctx`     = EvaluationContext (headers, path, cookies, device, parsed HTML).
        /// Returns the branch outcome zen will route on.
        fn evaluate(&self, config: &serde_json::Value, ctx: &EvaluationContext)
            -> Result<ProcessorOutcome, ProcessorError>;
    }

    /// What a processor decides; the adapter turns this into zen NodeResponse output
    /// (e.g. { "branch": "yes" } / { "branch": "no" }) that edge conditions route on.
    pub struct ProcessorOutcome { pub branch: Branch /* Yes | No */ }
    ```
  - **`ProcessorRegistry`**: `HashMap<&'static str, Arc<dyn CanvasProcessor>>` with `register(p: Arc<dyn CanvasProcessor>)` (keys off `p.kind()`) and `get(kind) -> Option<Arc<dyn CanvasProcessor>>`. Built once at startup; held in `AppState` behind `Arc`.
  - **Single `CustomNodeAdapter` impl** (`CanvasNodeAdapter`) bridges zen → registry. Per `core/engine/src/nodes/custom/adapter.rs`:
    ```rust
    impl CustomNodeAdapter for CanvasNodeAdapter {
        fn handle(&self, request: CustomNodeRequest)
            -> Pin<Box<dyn Future<Output = NodeResult> + '_>> {
            Box::pin(async move {
                let kind = request.node.kind.as_ref();           // = content.kind
                let proc = self.registry.get(kind).ok_or_else(/* typed ProcessorError::UnknownKind -> NodeError */)?;
                let outcome = proc.evaluate(request.node.config.as_ref(), &self.ctx)?;
                Ok(NodeResponse { output: outcome.into_variable(), trace_data: None })
            })
        }
    }
    ```
    - `NodeResult = Result<NodeResponse, NodeError>`; `NodeResponse { output: Variable, trace_data: Option<Variable> }` (per `core/engine/src/nodes/result.rs`). Unknown `kind` → typed `ProcessorError::UnknownKind` mapped to a `NodeError` (never silently default true/false). Missing/invalid config keys → typed `ProcessorConfigError` mapped to `NodeError` (never `unwrap()` panic).
  - **Adding a future processor = implement `CanvasProcessor` + one `registry.register(Arc::new(MyProcessor))` line.** The translator (it already turns any DecisionNode into a `CustomNode` keyed by processor type), the adapter, and the evaluator are untouched.
- **MetaTagsProcessor** (`kind() == "metaTags"`) reads the pre-parsed HTML from `EvaluationContext` (CSS selector `meta[name="{tag_name}"]`, see selector-safety rule below), reads the `content` attribute, returns a `ProcessorOutcome`:
  - `exists` → branch Yes iff tag found.
  - `equals` → branch Yes iff exact-match on `content` attribute.
  - `contains` → branch Yes iff substring match.
  - Untrusted `tag_name` from component config is length-capped + character-whitelisted before being interpolated into the selector. Selector miss → log WARN, fail-open (branch No).
- **DeviceTypeProcessor** (`kind() == "deviceType"`) reads the device classification from `EvaluationContext` (computed once from `User-Agent`: `Mobi|Android|iPhone` → `mobile`, `iPad|Tablet` → `tablet`, else `desktop`) and applies operator/value, returning a `ProcessorOutcome`.
- **zen dependency** is the path dependency wired in Task 17 (`zen-engine`, `zen-expression` via `path`); no new crate version pins here.
- **Compiled-graph cache:** the translated `DecisionContent` (call `.compile()` on it once, per `core/engine/src/model/decision_content.rs`) is cached via `moka` keyed by `(feature_id, version_number)`, size-bounded (≤256). Cache the compiled JDM, not just the raw payload, so per-request work is only `engine.create_decision(content).evaluate(input)`.
- New backend endpoint `GET /api/v1/features/{id}/active-version?env=live` returning the cached payload.
- Proxy logs `outcome_id` per request as `outcome=<id|none>` plus `eval_ms`.
- This task does **not** modify the response yet (that lands in Task 19). It only computes and logs the outcome.
- Unit tests on each processor + the graph→JDM translator + an end-to-end zen evaluation + classifier.

## Implementation Steps
1. Backend: add `src/api/v1/features.rs::active_version(id, env)` returning compact payload `{version_number, rule_graph, outcomes: [{id, title, components: [...]}]}`.
2. Proxy modules:
   ```
   proxy/src/
   ├── domain/
   │   ├── graph.rs            # serde models mirroring Task 10 rule_graph schema (canvas side)
   │   ├── translator.rs       # canvas CanvasGraph -> zen_engine::model::DecisionContent (JDM)
   │   ├── evaluator.rs        # GraphEvaluator: translate -> compile/cache -> DecisionEngine.evaluate -> outcome_id
   │   ├── adapter.rs          # CanvasNodeAdapter: impl zen_engine CustomNodeAdapter -> ProcessorRegistry
   │   ├── processors/
   │   │   ├── mod.rs          # CanvasProcessor trait + ProcessorRegistry + ProcessorOutcome/Branch/ProcessorError
   │   │   ├── meta_tags.rs    # MetaTagsProcessor   (kind() == "metaTags")
   │   │   └── device_type.rs  # DeviceTypeProcessor (kind() == "deviceType")
   │   └── classifier.rs
   ├── infra/
   │   ├── feature_map.rs
   │   ├── backend_client.rs   # cached active-version fetch (raw payload, moka future::Cache + TTL)
   │   └── compiled_cache.rs   # moka cache of compiled DecisionContent keyed by (feature_id, version_number)
   ```
3. `EvaluationContext { request_headers, request_path, request_cookies, device: DeviceType, response_html: scraper::Html }`. Built once per request; processors only read it (no per-request global writes).
4. Caches: raw active-version payload via `moka::future::Cache` (TTL 30s); compiled `DecisionContent` via a size-bounded `moka` cache (≤256) keyed by `(feature_id, version_number)`. Both are concurrency-safe — no external lock.
5. Build `ProcessorRegistry` once at startup (`register(Arc::new(MetaTagsProcessor)); register(Arc::new(DeviceTypeProcessor));`) and hold it in `AppState`. Per request, construct `CanvasNodeAdapter { registry, ctx }`, `DecisionEngine::default().with_adapter(Arc::new(adapter))`, then evaluate the cached compiled decision inside `spawn_blocking` (zen `Variable` is `!Send`).
6. Wire into the `forwarder::forward(...)` pipeline; log `outcome=<id|none> eval_ms=<n>`.

## Files
- Proxy: as above (incl. `domain/translator.rs`, `domain/adapter.rs`, `domain/processors/mod.rs`, `infra/compiled_cache.rs`)
- Backend: `src/api/v1/features.rs` extension, schema `src/schemas/active_version.rs`
- Tests:
  - `proxy/tests/translator.rs`
  - `proxy/tests/evaluator.rs`
  - `proxy/tests/meta_tags_processor.rs`
  - `proxy/tests/device_type_processor.rs`
  - `proxy/tests/classifier.rs`
  - `proxy/tests/feature_map.rs`
  - `backend/tests/active_version_endpoint.rs`

## Tests
- Translator: canvas DecisionNode → JDM `CustomNode` with `content.kind` = processor key + config preserved; OutcomeNode → node emitting `{ outcomeId }`; `Edge.branch` → `DecisionEdge.source_handle`; round-trips through `serde_json::from_value::<DecisionContent>`.
- Evaluator (via zen): 1-decision graph reaches the expected outcome; deep/cyclic canvas is bounded by zen `max_depth` (no panic, no hang); empty/dead-end canvas → `None`.
- Processors via the adapter: unknown `content.kind` → typed `ProcessorError::UnknownKind` surfaced as `NodeError` (not a default branch); missing config key → typed `ProcessorConfigError` (no panic).
- MetaTags: positive + negative + missing tag; oversized/illegal `tag_name` rejected before the selector.
- DeviceType: UA strings for each device + custom operator paths.
- Classifier: cookie absent → anonymous; each cookie value → matching canvas.
- Backend endpoint: returns 200 with shape; 404 if no LIVE version.
- "Add a processor" guard test: a dummy `CanvasProcessor` registered with one `register()` line is reachable end-to-end through translator → adapter → zen without touching evaluator/adapter/translator code.

## Verify
1. Author a graph in the dashboard (Anonymous canvas): `MetaTags(paywall, contains, true)` → YES branch → Outcome "Show Paywall" terminal; NO branch → "Show Content".
2. Publish to LIVE.
3. `curl localhost:9000/article.html` → response unchanged, but `docker logs proxy` shows `outcome=<id of Show Paywall>` (computed by zen `DecisionEngine` over the translated graph).
4. `curl -H "Cookie: rre_user_type=customer" localhost:9000/article.html` → uses customer graph (still empty) → `outcome=none`.
5. `cargo test` green across backend + proxy.

## Done When
PR merged with log snippets demonstrating different outcomes for different request contexts. The PR description shows the translated JDM for the demo graph and confirms evaluation goes through `zen_engine::DecisionEngine` (not a bespoke walker), and that a new processor needs only `impl CanvasProcessor` + one `register()` line.
