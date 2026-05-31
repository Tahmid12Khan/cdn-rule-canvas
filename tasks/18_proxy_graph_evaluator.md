# Task 18 — Proxy Graph Evaluator + Processors

## Goal
Inside the proxy runtime, fetch the active version's rule graph for the requested feature, classify the user into a canvas (Anonymous/Registered/Customer), traverse the graph against request + response context, and identify which Outcome (if any) should be applied. Implement `MetaTags` and `DeviceType` processors.

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
  6. Runs `GraphEvaluator::evaluate(canvas_graph, ctx)` to find the terminal outcome id.
- `GraphEvaluator`:
  - Walks from `root_node_id` following edges.
  - For each decision node, calls the matching `Processor` (`MetaTagsProcessor` or `DeviceTypeProcessor`), gets a `bool`, follows `yes` / `no` edge.
  - Stops at first OutcomeNode; returns its `outcome_id` (`None` if traversal dead-ends).
  - Configurable max-depth guard (default 64).
- **MetaTagsProcessor** parses HTML with `scraper` (CSS selector `meta[name="{tag_name}"]`), reads the `content` attribute, returns:
  - `exists` → boolean tag-found.
  - `equals` → exact-match on `content` attribute.
  - `contains` → substring match.
- **DeviceTypeProcessor** parses `User-Agent` header (lightweight: regex for `Mobi|Android|iPhone` → `mobile`, `iPad|Tablet` → `tablet`, else `desktop`) and applies operator/value.
- New backend endpoint `GET /api/v1/features/{id}/active-version?env=live` returning the cached payload.
- Proxy logs `outcome_id` per request as `outcome=<id|none>`.
- This task does **not** modify the response yet (that lands in Task 19). It only computes and logs the outcome.
- Unit tests on each processor + graph traversal + classifier.

## Implementation Steps
1. Backend: add `src/api/v1/features.rs::active_version(id, env)` returning compact payload `{version_number, rule_graph, outcomes: [{id, title, components: [...]}]}`.
2. Proxy modules:
   ```
   proxy/src/
   ├── domain/
   │   ├── graph.rs            # serde models mirroring rule_graph schema
   │   ├── evaluator.rs        # GraphEvaluator
   │   ├── processors/
   │   │   ├── mod.rs          # Processor trait
   │   │   ├── meta_tags.rs
   │   │   └── device_type.rs
   │   └── classifier.rs
   ├── infra/
   │   ├── feature_map.rs
   │   └── backend_client.rs   # cached active-version fetch
   ```
3. `EvaluationContext { request_headers, request_path, request_cookies, response_html: scraper::Html }`.
4. Async LRU+TTL cache via `moka::future::Cache` (built-in TTL + concurrency — no external lock needed).
5. Wire into the `forwarder::forward(...)` pipeline.

## Files
- Proxy: as above
- Backend: `src/api/v1/features.rs` extension, schema `src/schemas/active_version.rs`
- Tests:
  - `proxy/tests/evaluator.rs`
  - `proxy/tests/meta_tags_processor.rs`
  - `proxy/tests/device_type_processor.rs`
  - `proxy/tests/classifier.rs`
  - `proxy/tests/feature_map.rs`
  - `backend/tests/active_version_endpoint.rs`

## Tests
- Evaluator: 1-node graph reaches outcome; cycle aborts (depth guard); dead-end returns `None`.
- MetaTags: positive + negative + missing tag.
- DeviceType: UA strings for each device + custom operator paths.
- Classifier: cookie absent → anonymous; each cookie value → matching canvas.
- Backend endpoint: returns 200 with shape; 404 if no LIVE version.

## Verify
1. Author a graph in the dashboard (Anonymous canvas): `MetaTags(paywall, contains, true)` → YES branch → Outcome "Show Paywall" terminal; NO branch → "Show Content".
2. Publish to LIVE.
3. `curl localhost:9000/article.html` → response unchanged, but `docker logs proxy` shows `outcome=<id of Show Paywall>`.
4. `curl -H "Cookie: rre_user_type=customer" localhost:9000/article.html` → uses customer graph (still empty) → `outcome=none`.
5. `cargo test` green across backend + proxy.

## Done When
PR merged with log snippets demonstrating different outcomes for different request contexts.
