# Spec: Canvas v2 — journey, validation, node redesign, JSON rules & applicability

Master spec for a cross-layer feature. Authoritative deltas to `CONTRACTS.md` (update it as part of the work). Layers: **config (manifest)**, **backend**, **proxy**, **frontend**.

> Identifier rule (unchanged): manifest `kind` == rule_graph processor `type` == proxy `CanvasProcessor::kind()` == JDM CustomNode kind. All snake_case. One mismatch breaks eval.

---

## 0. User requirements → where they land

| # | Requirement | Layer |
|---|---|---|
| 1 | Journey shows start→end: dagre auto-layout **and** always-on START→outcome path highlight | frontend |
| 2 | Validate no cycles + always-an-outcome — **client and server** | backend + frontend |
| 3 | On error, show **full server response** in an expand panel (first ~1000 words, then `…`) | frontend |
| 4 | Node: name on top **in front** of node; all input field values shown **inside**, one per line, **centered** (bigger diamond) | frontend |
| 5 | Edge connects to the **closest side** of the destination from the source (floating edges) | frontend |
| 6 | Rename "Article URL" → **"URL"** | config (label only) |
| 7 | **JSON expression** decision (`$.path` + operator + value); only available for JSON features | config + proxy + frontend |
| 8 | HTML/JSON rules must NOT apply to all: apply HTML only when content is HTML **and** a CSS selector matches; apply JSON only when content is JSON **and** a JSONPath matches. Configurable at the **version** level | backend (config storage) + proxy (gate) + frontend (UI) |
| — | JSON rule effect = **full JSON body mutation** (remove path / set field / replace value) | backend (component types) + proxy (JSON applier) + frontend (forms) |

---

## 1. Config / manifest — `backend/config/node_types.json`

### 1.1 Rename (req 6)
`article_url` node: change `"label": "Article URL"` → `"label": "URL"`. **Keep `kind: "article_url"`** (changing kind breaks the proxy processor registration and existing graphs). Update `summary` to `"Matches against the request URL (path)."`.

### 1.2 New `applies_to` field on every node-type spec
Add an optional `applies_to` discriminator to gate palette availability by feature type:
- values: `"all"` (default when omitted) | `"html"` | `"json"`.
- `meta_tags` → `"html"` (reads response HTML meta tags).
- `device_type`, `article_url` → `"all"` (request-based; work for any feature).
- New `json_expression` → `"json"`.

Backend `NodeTypeSpec` (schemas/node_type.rs) gains `#[serde(default)] applies_to: AppliesTo` where `AppliesTo` = `All|Html|Json` (serde rename_all snake_case, `#[default] All`). The raw-JSON endpoint already serves verbatim, so the field flows to the frontend automatically. Frontend filters palette chips by the current feature's type (`all` always shown).

### 1.3 New node type `json_expression` (req 7)
```json
{
  "kind": "json_expression",
  "label": "JSON Expression",
  "category": "json",
  "applies_to": "json",
  "summary": "Matches a JSONPath in the response body against a value.",
  "fields": [
    { "name": "json_path", "label": "JSON path", "control": "text", "required": true, "default": "",
      "placeholder": "$.type", "required_message": "Enter a JSONPath (e.g. $.type)" },
    { "name": "operator", "label": "Operator", "control": "select", "required": true, "default": "contains",
      "options": [
        { "value": "equals", "label": "equals" },
        { "value": "contains", "label": "contains" },
        { "value": "starts_with", "label": "starts with" },
        { "value": "ends_with", "label": "ends with" },
        { "value": "is_one_of", "label": "is one of (comma-separated)" },
        { "value": "exists", "label": "exists" }
      ] },
    { "name": "value", "label": "Value", "control": "text", "required": false,
      "required_unless": { "field": "operator", "value": "exists" }, "default": "",
      "placeholder": "e.g. premium", "required_message": "Enter a value, or switch operator to 'exists'" }
  ],
  "output": { "branches": [ { "id": "yes", "label": "Yes" }, { "id": "no", "label": "No" } ] }
}
```
Add a non-coming_soon category `{ "id": "json", "label": "JSON" }` to `categories`.

No backend Rust changes for this node beyond `applies_to` — manifest-driven validation handles its fields. Proxy needs one `CanvasProcessor` (WF2).

---

## 2. Backend

### 2.1 Reachability validation rule `outcome_reachable` (req 2)
Add to `rule_graph_service::validate_canvas`. Definition (mirrors the accepted "diamond" semantics — partial branches are OK, dead-ends are NOT):

> Every node **reachable from the canvas root** must be able to **reach at least one Outcome node**.

Algorithm:
1. Determine `root`: `graph.root_node_id` if `Some`; else the unique node with **no incoming edges** (over resolvable edges). If neither yields a single root, **skip** this rule (do not emit) — anchoring requires a defined start; other rules (`root_in_nodes`) cover misconfig.
2. If `nodes` is empty → skip (empty canvas stays valid, test #1).
3. `can_reach_outcome` = reverse-BFS from all Outcome nodes over reversed resolvable edges.
4. `reachable` = forward-BFS from `root` over resolvable edges.
5. For each node in `reachable` **not** in `can_reach_outcome`: push `ValidationDetail { loc: "rule_graph.<canvas>.nodes[idx]", msg: "node '<id>' cannot reach an outcome", rule_id: "outcome_reachable" }`.

This must NOT break existing tests: all tests with `root_node_id: None` skip; `well_formed` (#2), `diamond` (#9), `meta_tags_*` (#15,#18) all have a root that reaches outcomes. Add new unit tests: dead-end decision with root set → fails; decision→decision→outcome chain → passes; outcome-only root → passes.

Update the rule table doc comment + `CONTRACTS.md §6` validation table with the new `outcome_reachable` row.

> The frontend already maps `rule_id` → human reason in `validationMapping.ts:reasonFor`; add an `outcome_reachable` case there (WF3).

### 2.2 Version-level applicability (req 8)
New JSONB column + DTO `Applicability`:
```rust
// schemas/version.rs (or a new schemas/applicability.rs)
#[derive(Deserialize, Serialize, Clone, Debug, Default, PartialEq, ToSchema)]
pub struct Applicability {
    /// CSS selector that must match ≥1 element for an HTML feature's rules to apply.
    /// None/empty = apply whenever the response content-type is HTML.
    #[serde(default)] pub html_selector: Option<String>,
    /// JSONPath that must match for a JSON feature's rules to apply.
    /// None/empty = apply whenever the response content-type is JSON.
    #[serde(default)] pub json_selector: Option<String>,
}
```
- **Migration** `0006_version_applicability.up.sql`:
  `ALTER TABLE rre.versions ADD COLUMN applicability JSONB NOT NULL DEFAULT '{}'::jsonb;`
- `models/version.rs`: add `pub applicability: serde_json::Value`. Add `applicability` to `VERSION_COLUMNS` in version_repository.
- `repositories/version_repository.rs`:
  - `insert`: add an `applicability` param (or insert `'{}'` default and set via update_fields). Simplest: keep insert as-is (DB default `'{}'`), add applicability to `update_fields` as `applicability = COALESCE($N, applicability)`.
  - `VERSION_COLUMNS` const must include `applicability`.
- `schemas/version.rs`:
  - `VersionCreate`: add `#[serde(default)] pub applicability: Option<Applicability>`.
  - `VersionUpdate`: add `#[serde(default)] pub applicability: Option<Applicability>` (editable on DRAFT only, same lock as rule_graph).
  - `VersionRead`: add `pub applicability: Applicability` (parse from the stored Value; default on parse-miss).
- `services/version_service.rs`:
  - `to_read`: parse `applicability` (default `{}` on null/missing).
  - `create_version`: store `body.applicability` if present, else carry forward `source.applicability`, else default.
  - `update`: when `body.applicability` present, require DRAFT (reuse the VERSION_EDIT_LOCKED path) and persist via update_fields.
- **Validation**: light backend check only — `html_selector`/`json_selector` length ≤ 500 and trimmed-nonempty-if-present. Real selector parsing happens in the proxy (resilient). Surface a `VALIDATION_ERROR` on overflow.
- `schemas/active_version.rs`: add `pub applicability: Applicability` to `ActiveVersionRead` so the proxy receives it. `version_service::active_version` must populate it.

### 2.3 JSON outcome component types (full mutation)
Extend `ComponentConfig` (schemas/component.rs) with three internally-tagged variants:
```rust
/// type = "json_remove"  — delete the value(s) at target_path.
JsonRemove { target_path: String },
/// type = "json_set"     — upsert (create or overwrite) the value at target_path.
JsonSet { target_path: String, value: serde_json::Value },
/// type = "json_replace" — overwrite ONLY if target_path already exists.
JsonReplace { target_path: String, value: serde_json::Value },
```
- `type_str()`: add the three arms.
- `validate_domain()`: `target_path` trimmed-nonempty; for set/replace `value` may be any JSON (incl. null). Add a length cap on `target_path` (≤ 500).
- **`target_path` is a simple path** (dot + `[index]`, e.g. `$.user.premium`, `$.items[0].price`) — NOT a filter expression. Document this; the proxy mutator walks the parsed path. (Filter-style JSONPath is only for read/query: applicability + json_expression.)
- **Migration**: extend the CHECK constraint. `0005` defines
  `CHECK (type IN ('html_injection','content_truncation'))`. Add migration `0006_*` (same file as applicability, or `0007`) that drops & recreates `components_type_known` to also allow `'json_remove','json_set','json_replace'`.
- `ActiveComponent.type` is a free string already — no change. Proxy dispatches on it (WF2).

### 2.4 Seed / demo
`bin/seed_demo.rs` seeds `demo-article` (HTML feature). After adding `outcome_reachable`, ensure the seeded graph's every reachable node reaches an outcome and `root_node_id` is set. If the seed graph has a dead-end, fix it. Run `cargo run --bin seed_demo` is not part of CI; just ensure `cargo test` passes and the seed graph would validate (add/adjust a test if practical).

### 2.5 Verify (WF1 done-criteria)
`PATH="$HOME/.cargo/bin:$PATH" cargo build --manifest-path backend/Cargo.toml` and `cargo test --manifest-path backend/Cargo.toml` green. `make backend-check` mirrors CI (fmt + clippy + test). New unit tests for `outcome_reachable`, applicability round-trip in `to_read`, and the new ComponentConfig variants' `validate_domain`.

---

## 3. Proxy (WF2 — FINAL design)

The gate keys off the **response content-type** (not feature type): `text/html` → HTML path + `html_selector` gate; `application/json` → JSON path + `json_selector` gate; neither → pass-through. This needs no feature-type lookup (a `json_expression` node on an HTML response simply sees no `response_json` and returns `No`; `meta_tags` on JSON sees no meta tags → `No`).

### 3.1 `proxy/Cargo.toml`
Add `serde_json_path = "0.7"` (read-only JSONPath query, incl. filter exprs for applicability + json_expression). Mutation uses a hand-rolled simple-path walk (no extra crate). Keep `scraper` for CSS matching.

### 3.2 `infra/backend_client.rs` — mirror `applicability`
Add a proxy `Applicability { #[serde(default)] html_selector: Option<String>, #[serde(default)] json_selector: Option<String> }` and `#[serde(default)] pub applicability: Applicability` on `ActiveVersionRead`. (Backend serializes `version_number, rule_graph, applicability, outcomes`.)

### 3.3 `domain/context.rs` — carry the response JSON
- `EvaluationContext` (shared with the zen adapter — MUST stay `Send + Sync`): add `pub response_json: Option<serde_json::Value>` (`Value` is `Send+Sync`; do NOT hold `scraper::Html`).
- `EvaluationContextParts`: add `pub is_json: bool` (keep `html: String` as the body carrier). `from_request(headers, path, cookies, body, is_json)`. `into_context()`: when `is_json` → `response_json = serde_json::from_str(&body).ok()`, `meta_tags` empty; else extract meta tags as today, `response_json = None`.
- Update the two `from_request` call sites (forwarder HTML path + eval.rs test path).

### 3.4 `domain/processors/json_expression.rs` (new) — register in `default_registry`
- `kind() = "json_expression"`.
- Read `config.json_path` (str, len-cap ≤1000), `config.operator` (str), `config.value` (str, optional).
- `let Some(body) = ctx.response_json.as_ref() else { return Ok(No) }`.
- Parse path via `serde_json_path::JsonPath::parse(json_path)` (parse error → `ProcessorError::Config`, fail-open); `path.query(body).all()` → `Vec<&Value>`.
- Stringify each matched node (`as_str()` for strings, else `to_string()`), then apply operator:
  - `exists` → matched set non-empty.
  - `equals` → any node-string == value.
  - `contains` / `starts_with` / `ends_with` → any node-string matches accordingly.
  - `is_one_of` → value is comma-separated; any node-string ∈ the trimmed set.
- `Branch::Yes` on hit else `Branch::No`. (Mirror `article_url.rs` structure/limits.)

### 3.5 `domain/applier/` — JSON mutation
- New `applier/json_path.rs`: parse a SIMPLE path (`$` root optional, `.key`, `[index]`) into `Vec<Seg::{Key(String),Index(usize)}>`. Reject filter/wildcard syntax (those are query-only).
- New `applier/json_apply.rs` (orchestrator parallel to `orchestrator.rs`): `apply_outcome_json(body: Value, outcome, ) -> Result<JsonModificationResult{ json, applied }, ApplyError>`. Order components by `(placement_rank, order_index)` (same as HTML). Dispatch on `component.r#type`:
  - `json_remove { target_path }` → navigate to parent, remove the key/index if present.
  - `json_set { target_path, value }` → upsert; create intermediate objects for missing `Key` segments (missing `Index` on a non-array → skip, fail-open).
  - `json_replace { target_path, value }` → overwrite ONLY if the path already resolves.
  - `html_injection` / `content_truncation` / unknown → no-op for JSON (warn + skip; fail-open).
- Per-component failure is caught and skipped (same fail-open contract as the HTML orchestrator). `applied` = any component changed the body.

### 3.6 `forwarder.rs` — JSON branch (req 8 + JSON mutation)
After `decode_for_modify` (works for any text body):
1. Determine kind: `is_html()` (existing) vs new `is_json()` (`content_type` contains `application/json`). Neither → existing skip (`into_response("skipped")`).
2. **HTML path** (existing + applicability gate): if `av.applicability.html_selector` is set+non-empty, parse `body_string` with `scraper` and require ≥1 element match; if zero matches → serve original, `apply_status="skipped"` (do NOT evaluate/apply). Otherwise proceed as today (build ctx `is_json=false`, evaluate, resolve outcome, `orchestrator::apply_outcome`).
3. **JSON path** (new): parse `body_string` as `serde_json::Value` (parse fail → serve original, skipped). If `av.applicability.json_selector` set+non-empty, require `serde_json_path` query to match ≥1 node; else skip. Build ctx `is_json=true`, evaluate the classified canvas, resolve outcome (reuse the `None` / builtin-ShowContent short-circuits → skipped). On a real outcome, `json_apply::apply_outcome_json` → re-serialize to string → `encoding::reencode` → rebuild with `Content-Length` fixed. Keep the `x-rre-apply-status` stamping + tracing parity with the HTML path.
4. Factor the shared eval+outcome-resolve into a helper if it reduces duplication, but keep it readable; never panic; every failure degrades to serving the original body.

### 3.7 `eval.rs` test endpoint (`/__rre/eval`) — support JSON test input
Read the current request DTO. Add an optional response-body input so the frontend can test JSON features: accept an optional `body`/`content_kind` (or `response_json`) field; when JSON, build the `EvaluationContext` with `response_json` populated so `json_expression` nodes evaluate. Keep the existing trace response shape (the frontend path highlight depends on it). Document the new request field for WF3.

### 3.8 graph.rs / translator
`json_expression` is a generic open-config processor — **no `graph.rs` or `translator.rs` struct change** (the `ProcessorRef` open map + registry dispatch already handle any kind). Verify the translator emits a CustomNode for it (it should, generically).

### 3.9 Verify
`make proxy-check` (fmt + clippy -D warnings + `cargo test`). Add unit tests: json_expression operators (equals/contains/starts_with/ends_with/is_one_of/exists, incl. no-body → No, bad path → fail-open); applicability CSS match + JSONPath match (hit/miss); json_path parser (key/index/reject-filter); json mutation remove/set(upsert+intermediate)/replace(exists-only). A wiremock forwarder test for the JSON branch if feasible. Latency smoke unchanged.

## 4. Frontend (WF3 — FINAL design)

Feature `type` (`"html"`/`"json"`) is the route param already passed to `RuleBuilderClient` (`frontend/src/components/version/RuleBuilderClient.tsx:50`) and to the outcome editor page — thread it to the palette, TestPanel, applicability form, and component modal. Split into **3a (logic/state/api)** then **3b (UI)** — 3b depends on 3a's types/store.

### 4a. Logic / state / api
- **`lib/api/nodeTypes.ts`**: add `applies_to: z.enum(["all","html","json"]).optional()` to `NodeTypeSpec`.
- **`lib/api/client.ts`**: capture the raw error body. In `request()`, read the response **text once** (`const text = await res.text()`), parse the envelope from `text` (not a second `res.json()`), and pass the raw `text` to `ApiError`. Add `public rawBody?: string` to `ApiError`. Do the same in `lib/api/evalTest.ts:postEvalTest` (so proxy errors also carry rawBody).
- **`lib/api/ruleGraph.ts`** (or wherever): add an `Applicability` zod = `{ html_selector: z.string().nullish(), json_selector: z.string().nullish() }`.
- **`lib/api/canvasVersions.ts`**: add `applicability: Applicability.optional()` (default `{}`) to `VersionRead`; add `applicability: Applicability.optional()` to `VersionUpdate` and `VersionCreate`. Add a helper `patchApplicability(fid, vnum, applicability)` → `updateVersion(fid, vnum, { applicability })`. (`createVersionFromGraph` should also forward applicability when provided — optional for MVP.)
- **`lib/canvas/graphValidation.ts`** (NEW, pure, no React): mirror the server rules the client should pre-flight:
  - `findCycle(nodes, edges)`: DFS three-colour over real edges (exclude the start node + its edge) → returns the set of node ids on a cycle (or null).
  - `findUnreachableOutcomeNodes(nodes, edges, rootNodeId)`: reverse-BFS from outcome nodes; forward-BFS from root (use the start node's target as root, == `computeRootNodeId`); return reachable nodes that can't reach an outcome. Mirror §2.1 exactly (skip when no single root / empty).
  - `validateCanvasGraph(canvas)` → `{ nodeErrors: Record<id,string>, problems: string[] }` combining both, with messages matching server copy ("forms a cycle", "cannot reach an outcome").
  - `validateAllCanvases(canvases)` → aggregate per-canvas; used by SaveBar pre-flight.
- **`lib/canvas/validationMapping.ts`**: add a `reasonFor` case for `rule_id === "outcome_reachable"` → `"have no path to an outcome (every branch must end at an outcome)"`.
- **`lib/canvas/layout.ts`** (NEW, pure): `autoLayout(nodes, edges, opts?)` using `@dagrejs/dagre` (add dep) — rank top-to-bottom (TB), include the start node as the rank-0 source, return nodes with new `position`. Keep node sizes in sync with the redesigned node dimensions.
- **`lib/schemas/components.ts`**: add `JsonRemoveConfig {type:"json_remove", target_path:string.min(1)}`, `JsonSetConfig {type:"json_set", target_path, value: z.unknown()}`, `JsonReplaceConfig {type:"json_replace", target_path, value: z.unknown()}` to the discriminated union; extend `ComponentType` enum; extend `defaultConfigFor` (target_path "", value null). `target_path` ≤500, message hints "$.user.premium".
- **`state/ruleBuilderStore.ts`**: add
  - `setNodePositions(k, Map<id,{x,y}>)` (bulk position set for layout; flips dirty).
  - keep `testHighlight` as the bright/test highlight; add a derived **journey path** (compute in the node/edge components or a selector): the set of node/edge ids on any START→outcome path (forward-reachable ∩ can-reach-outcome). 3b uses it for always-on dim styling; testHighlight overrides brighter.

### 4b. UI
- **`nodes/DecisionNode.tsx`** (req 4): grow diamond to ~`h-28 w-28`; title span gets `relative z-20` (and the diamond `z-0`) so the name renders **in front** (fixes "name behind node"); inside the counter-rotated label, render a centered vertical stack — title line + one `fieldDisplayValue` per manifest field, each on its own line, centered, truncated. Keep the hover tooltip. Keep error ring + onPath ring.
- **Floating edges** (req 5): in `edges/LabeledEdge.tsx`, read the **target** node geometry via `useStore`/`useInternalNode(target)` and compute the closest point on the target node's border to the source point (`sourceX,sourceY`), choosing `targetPosition` by side; pass the computed `targetX/targetY/targetPosition` to `getBezierPath`. The edge then meets the **closest side** of the destination. Keep the YES/NO source handles (right/bottom) for branch semantics and the YES/NO pill + delete button. For connecting, set `connectionMode={ConnectionMode.Loose}` on `<ReactFlow>` and give DecisionNode/OutcomeNode a target handle that is connectable from any side (or keep the single target handle — the post-connect render floats to the closest side regardless). Do NOT regress `START_NODE_ID` non-deletable edge handling.
- **`nodes/OutcomeNode.tsx` / `StartNode.tsx`**: z-index parity with the new DecisionNode; ensure a target handle exists for floating attach.
- **Auto-layout + always-on highlight** (req 1): in `RuleBuilderCanvas.tsx` add a "Tidy layout" control (calls `autoLayout` → `setNodePositions`) and run it once after seed/empty when nodes lack a layout; always-on journey highlight — style journey-path nodes/edges as the baseline (subtle), and when `testHighlight` is active brighten the traversed path + dim the rest (existing behavior). Reuse the `rre-edge-dash` keyframe for the test path.
- **`ui/ErrorBanner.tsx`** (req 3): add an optional `rawResponse?: string` prop; when present render a collapsible accordion ("Show full server response") that displays the raw body **truncated to the first ~1000 words then `…`**, expandable to full (a `<details>` or button-toggled `<pre>` with max-height + scroll). Wire SaveBar + TestPanel to pass `err.rawBody` (from `ApiError`) into the banner.
- **`SaveBar.tsx`** (req 2): add a **pre-flight client validation gate** before `save.mutate()` / `saveAsNew.mutate()`: run `validateAllCanvases(store.canvases)`; if problems, `setNodeErrors(...)` + `setError(buildClientValidationUserError(...))` and DO NOT call the mutation. On server 422, also pass `err.rawBody` to the banner.
- **`TestPanel.tsx` + `evalTest.ts`** (req 7 JSON test): for JSON features, add a JSON response-body textarea; send it per WF2's `evalRequestDelta` (the proxy eval request gained an optional response-body/content-kind field). Keep the existing device/meta/path inputs for HTML. Pass `err.rawBody` to the banner.
- **Palette filter** (req 7): `lib/canvas/nodeTemplates.ts buildPalette` + `palette/NodePalette.tsx` accept `featureType: "html"|"json"`; filter chips so a spec with `applies_to==="html"` shows only for HTML features, `"json"` only for JSON, `"all"`/undefined always. `json_expression` thus appears only for JSON features. Thread `featureType={type}` from `RuleBuilderClient`. (`GenericNodeForm` already renders `json_path`+operator+value from the manifest — the `$.type` placeholder is in the manifest; optionally add a small "JSONPath" hint.)
- **Applicability UI** (req 8): new `components/version/ApplicabilityForm.tsx` — for HTML features a "CSS selector" input (placeholder `#article, .article`), for JSON features a "JSONPath" input (placeholder `$.books[?(@.title contains 'Century')]`); empty = "always apply". Save via `patchApplicability` (DRAFT only; read-only otherwise). Seed from `version.applicability`. Place it in `RuleBuilderClient` near the Rules Builder header.
- **JSON component forms** (JSON mutation): new `JsonMutationForm.tsx` (or one per type) rendering `target_path` (+ a JSON `value` editor for set/replace). In `ComponentConfigModal.tsx`, choose the `TYPE_TABS` by feature type (HTML → html_injection/content_truncation; JSON → json_remove/json_set/json_replace) — thread `featureType` from the callers (`AddComponentDrawer` / `OutcomeEditorPage`, which have the `type` route param). Update `componentBadges.ts` if it switches on type.

### 4c. Verify
`cd frontend && npm run lint && npm run typecheck && npm run test`. Update/extend vitest specs for: `graphValidation` (cycle + reachability), node render (field values inside, title z-index), palette filtering by `applies_to`, ErrorBanner truncation, ComponentConfig JSON schemas. Keep Playwright e2e green if it covers the canvas.

---

## 5. Contract bookkeeping
Update `CONTRACTS.md`: §5 (VersionCreate/Update/Read + Applicability, ActiveVersionRead, ComponentConfig new variants), §6 (validation table: `outcome_reachable`; manifest `applies_to`), and note the `json_expression` node + version `applicability` column. Bump any contract version marker the file uses.
