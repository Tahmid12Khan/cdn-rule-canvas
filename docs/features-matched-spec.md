# features_matched + journey-richness spec (FROZEN CONTRACT)

Authoritative spec for the next feature batch. Workflow agents build against THIS
file (it survives context compaction). On any conflict, `CONTRACTS.md` wins for
DB/route/rule_graph shapes; this file owns the new runtime metadata + UI.

Verbatim user request is captured below in "Source request" — read it if any
detail here seems underspecified.

---

## 0. Scope (six deliverables)

1. **Match-marker headers** — `x-rre-feature-<feature_id>: true` for every feature
   that MATCHED on a request (proxy runtime).
2. **`features_matched` metadata** injected into the response:
   - JSON: `body.rre.features_matched`
   - HTML: a `<script>` at end of `<body>` setting `window.rre.features_matched`
3. **Empty-canvas default** = `start → end` (node + node + edge), enforced in BOTH
   backend and frontend.
4. **Highlight + collapsible journey** — full START→END highlight shown initially
   with the Transformation Journey COLLAPSED; expanding switches to node-by-node;
   collapsing restores the full-path highlight.
5. **Richer journey steps** — each step shows technical info (node label, inputs,
   result) AND a plain-English "what was done" sentence, plus per-node time.
6. **Per-node timing + expensive_nodes** — total feature time + top-3 slowest
   expression nodes, all formatted `d.dd`.

---

## 1. Locked definitions

- **feature_id**: the slug (`demo-json-article`, `demo-article`). Used as the key in
  headers and in `features_matched` (header/JSON-safe — names have spaces, slugs
  don't). The spec's `{feature_name}` placeholder == feature_id.
- **"matched" feature**: a feature whose evaluated path produced **≥1 expression
  action** (i.e. `outcome_ids` is non-empty). A feature that resolves the route
  but whose decision goes straight to END with no expression node is NOT matched
  → no header, no `features_matched` entry. (demo-article on a JSON response =
  not matched; demo-json-article on that response = matched.)
- **outcome node**: in this spec "outcome" == **expression node** (the action
  nodes: `trim_json`, `add_attribute`, `apply_outcome`, …). Decision / start / end
  nodes are NEVER outcomes.
- **outcome_ids / outcome_labels**: the canvas node ids / display labels of the
  expression nodes the matched path traversed, in order, **last 10** (if the path
  has >10 expression nodes, keep the last 10). Parallel arrays, equal length.
  - label = manifest label for the action kind (`fieldless` fallback: the raw
    kind string, e.g. `trim_json`).

---

## 2. `features_matched` data shape (identical for JSON body + HTML window)

```jsonc
{
  "features_matched": {
    "<feature_id>": {
      "outcome_ids":    ["t_body", "a_pw"],            // last 10 expression node ids, in order
      "outcome_labels": ["Trim JSON", "Add Attribute"],// parallel labels
      "time_took_ms":      "1.21",                         // whole-feature time, ms, string "d.dd"
      "expensive_nodes": [                              // top 3 expression nodes by time, DESC
        { "outcome_id": "a_pw",   "outcome_label": "Add Attribute", "outcome_time_in_ms": "0.80" },
        { "outcome_id": "t_body", "outcome_label": "Trim JSON",     "outcome_time_in_ms": "0.41" }
      ]
    }
  }
}
```

- `time_took_ms` = the feature's total `eval_ms + transform_ms` (sum of per-node apply
  times), formatted `d.dd`.
- `expensive_nodes` = expression nodes sorted by per-node apply time DESC, top 3
  (fewer if the path has <3 expression nodes).
- All times are **strings** formatted `d.dd` (see §3). Strings (not numbers)
  guarantee the trailing-zero format survives JSON (`0.10`, not `0.1`).

### Injection rules

- **JSON** (`apply_features_json`, after the per-feature loop): set
  `body["rre"]["features_matched"] = {…}`. Create the `rre` object if absent;
  merge `features_matched` without clobbering other `rre.*` keys. `rre` is a
  reserved top-level namespace. Inject only when ≥1 feature matched (else no `rre`
  key). The injected `rre` metadata itself does NOT count as a transform for the
  `apply_status` decision — but its presence means a feature matched, so
  `apply_status = "ok"` already holds.
- **HTML** (`apply_features_html`, after the loop): append, immediately before
  `</body>` (or at end of document if no `</body>`), exactly:
  ```html
  <script>window.rre=window.rre||{};window.rre.features_matched=<JSON>;</script>
  ```
  where `<JSON>` is `serde_json::to_string(&features_matched_map)` with `<`
  escaped to `<` (so an embedded `</script>` cannot break out — XSS-safe;
  this is first-party data). This script is **trusted** and MUST bypass the
  component HTML sanitizer (inject as the final step, after sanitized component
  transforms). Inject only when ≥1 feature matched.

---

## 3. Number format `d.dd` (LOCKED)

- Round to 2 decimal places, render with **exactly two** decimals.
  - Rust: `format!("{:.2}", ms_f64)`
  - TS: `ms.toFixed(2)`
- Examples (normalized): `0.10`, `1.20`, `1.21`, `123.00`, `123455.00`, `123.12`.
  (The user's "123 / 123455" examples are treated as `123.00 / 123455.00` — the
  `.toFixed(2)` reading of "only last 2 digits after the floating point".)
- The value carried in `features_matched` JSON and `window.rre` is a STRING.

---

## 4. Per-node timing (proxy + eval)

The forwarder currently folds actions with one `transform_ms` for the whole
feature. Change to **time each expression node's action apply** individually:

- In `apply_features_json` / `apply_features_html`, wrap each `MatchedAction`
  apply in its own `Instant::now()` … `elapsed()`, collecting
  `(node_id, label, time_ms)` per expression node.
- `label` resolution: manifest label for the action kind; fallback to the kind
  string. (Reuse the eval.rs `label()` helper pattern — a node-id→label lookup
  over the canvas.)
- Build the per-feature `features_matched` entry from these.
- Keep the existing `apply_status`/log behavior; per-node timing is additive.

`MatchedAction` already carries `node_id` + `action`. If the label needs the
canvas, the forwarder has `av.canvas(canvas_class)`.

---

## 5. `/__rre/eval` endpoint extension (test panel)

The eval endpoint tests ONE canvas (no feature_id). Extend its response so the
test panel can render timing + the rich journey:

- **`JourneyEntry`** gains `time_ms: String` ("d.dd"). For expression steps it's
  the node's apply time; for start/decision/end it's `"0.00"`.
- **`EvalResponse`** gains `summary`:
  ```jsonc
  "summary": {
    "outcome_ids": [...], "outcome_labels": [...],
    "time_took_ms": "d.dd",
    "expensive_nodes": [{ "outcome_id","outcome_label","outcome_time_in_ms" }]
  }
  ```
  built exactly like a single feature's `features_matched` entry, for the canvas
  under test. (Key by nothing — it's the single canvas.)
- Frontend `evalTest.ts` Zod: add `time_ms` to `JourneyStep` (default `"0.00"`)
  and an optional `summary` object (additive, default null/absent-safe).

---

## 6. Empty-canvas default = start → end (backend + frontend)

Any canvas with **zero nodes** must instead contain a start node, an end node,
and a single edge start→end. Enforce on BOTH sides; mirrors stay byte-identical.

- **Backend**: when a version is created/validated with an empty canvas (any of
  anonymous/registered/customer), auto-populate `{start, end, e_start_end}`
  rather than storing/accepting `{nodes:[],edges:[]}`. Put this in the canvas
  default/normalization path used by version create + validate
  (`rule_graph_service`). Node ids: `start`, `end`; edge id: `e_start_end`,
  `branch: yes`. Positions: start `{x:40,y:160}`, end `{x:940,y:160}`.
- **Frontend**: the empty-canvas factory in `ruleBuilderStore` (today returns
  `{ nodes:[], edges:[], rootNodeId:null }`) and `deserialize` of an empty backend
  canvas must yield the same start + end + edge, `rootNodeId = "start"`. RF node
  types `startNode` / `endNode`.
- Existing seeded/non-empty canvases are untouched.

---

## 7. Highlight + collapsible Transformation Journey (frontend)

Current bug: the run no longer highlights the full START→END path (the proxy's
`traversed_node_ids` omits start/end, and the journey's mount effect narrows the
glow to a single node). Target behavior:

- **On run (TestPanel.onSuccess)**: compute the **full-path** highlight from the
  journey node sequence — `nodeIds` = every `journey[i].node_id` (incl. start +
  end); `edgeIds` = for each consecutive journey pair, the live-canvas edge whose
  `(source,target)` matches. Store it as the "full path" (keep a fallback to
  `traversed_*` when journey is empty).
- **TransformationJourney is COLLAPSIBLE, default COLLAPSED**:
  - **Collapsed** → canvas shows the FULL START→END highlight. The journey body
    is hidden (just a header + "Expand" affordance + maybe step count).
  - **Expanded** → node-by-node stepper (current behavior): big ←/→ + ArrowLeft/
    Right, current node glows on canvas, body-after shown.
  - **Collapse again** → restore the full-path highlight.
- Coordinate the highlight via the store: collapsed sets the full path; expanded
  sets the current step. The full-path set must be available to the journey
  (pass from TestPanel as a prop, or stash on the store alongside `testHighlight`).
- Do not hijack ArrowLeft/Right globally — keys act only when the (expanded)
  journey container is focused (keep current behavior).

---

## 8. Richer journey step info (frontend)

Today a step shows only `trim_json` / `EXPRESSION · t_body` / body. Each step must
show BOTH:

- **Technical**: node label (manifest label, e.g. "Trim JSON"), kind + node_id,
  the **inputs** for this node (the action/processor config fields rendered as
  `Field label = value`, using the manifest field labels — looked up from the
  live canvas node's config + the node-type manifest), and the **result** (the
  body after / a one-line "what changed"). Also the node's `time_ms` (`d.dd ms`).
- **Non-technical**: a plain-English sentence of what was done. Compute via a
  `describeStep(kind, config, fieldLabels)` helper — data-driven from the manifest
  where possible, with concise per-kind phrasing for the expression kinds:
  - `trim_json` → "Trimmed the array at `$.body` to at most 0 items."
  - `add_attribute` → "Set `$.paywall_show` to `<html>paywall_showed</html>`."
  - `apply_outcome` → "Applied the outcome ‘<label>’."
  - decision → "Checked `$.api` equals `demo-article` → matched (yes)."
  - start/end → "Start of the flow." / "End of the flow — this is the final output."

Inputs come from the live canvas node (find by `node_id` in the selected canvas)
+ the node-type manifest (`getNodeTypes`) for field labels. The journey step
already carries `node_id`/`kind`/`label`/`body_after`/`time_ms`.

---

## 9. Per-lane checklist (for the workflow)

| Lane | Files (indicative) | Complexity / model |
|------|--------------------|--------------------|
| **proxy-runtime** | `proxy/src/forwarder.rs` (per-node timing, features_matched assembly, header injection, JSON `rre` inject, HTML `<script>` inject + escape + sanitizer bypass), small helpers; `proxy/src/domain/applier/json_apply.rs` if a raw-append helper is needed | HIGH → opus |
| **eval-endpoint** | `proxy/src/eval.rs` (JourneyEntry.time_ms, EvalResponse.summary) | MED → sonnet/opus |
| **backend-canvas** | `backend/src/services/rule_graph_service.rs` (+ schema if needed) empty→start/end normalization | MED → sonnet |
| **frontend-canvas** | `frontend/src/state/ruleBuilderStore.ts`, `frontend/src/lib/canvas/deserialize.ts` empty→start/end | MED → sonnet |
| **frontend-journey** | `frontend/src/components/canvas/TransformationJourney.tsx` (collapse/expand + highlight coordination + rich step info + describeStep), `frontend/src/components/canvas/TestPanel.tsx` (full-path highlight), `frontend/src/lib/api/evalTest.ts` (time_ms + summary zod) | HIGH → opus |
| **tests** | proxy: header/inject/timing e2e (`proxy/tests/*`); frontend: TransformationJourney + TestPanel vitest; backend: empty-canvas test | per-lane, with each lane |

Mirror rule still applies: backend `rule_graph.rs` ↔ proxy `graph.rs` stay
byte-identical (minus `ToSchema`).

---

## 10. Verification gates (every lane green before done)

- `make proxy-check` (fmt + clippy + cargo test) — incl. new header/inject/timing tests.
- `make backend-check` — incl. empty-canvas normalization test.
- `make frontend-check` (lint + typecheck + vitest) — journey collapse + rich step + full-path highlight tests.
- **Live E2E** (stack already runs natively via `scripts/dev.sh`; proxy upstream
  is `localhost:9001`):
  - JSON: `curl -s -D - http://localhost:9000/proxy/v2/content/2-1-1997318` →
    header `x-rre-feature-demo-json-article: true`; body has
    `rre.features_matched["demo-json-article"]` with outcome_ids `["t_body","a_pw"]`,
    labels, `time_took_ms` `d.dd`, `expensive_nodes` top-3 desc.
  - HTML: `curl -s http://localhost:9000/article.html` → ends with a
    `<script>window.rre.features_matched=…</script>`; `x-rre-feature-demo-article: true`.
  - Test panel (browser, after hard reload): journey collapsed by default with
    full START→END highlight; expand → node-by-node with inputs + plain-English
    description + per-node time; collapse → full highlight again.

---

## v2 — revisions (FROZEN; supersede v1 where they conflict)

Second batch of changes. `expression` is the new name for what v1 called
`outcome`. Mirror rule still applies (backend `rule_graph.rs` ↔ proxy `graph.rs`
byte-identical minus `ToSchema`).

### v2.1 — "matched" now requires the body to ACTUALLY CHANGE

Root cause of the `x-rre-feature-demo-article: true` bug on JSON: demo-article's
`apply_outcome` references an HTML outcome whose components are no-ops on a JSON
body (`json_apply` skips non-JSON component types → `changed = false`), yet v1
marked the feature on "≥1 expression node traversed" regardless. demo-article also
has an EMPTY `json_selector` (= "always apply"), so the selector gate can't stop
it. Fix:

- A feature is marked — `x-rre-feature-<feature_id>: true` header **and** a
  `feature_expressions` entry — ONLY when its canvas actually CHANGED the body
  (`applied == true`), in BOTH `apply_features_json` and `apply_features_html`.
  Gate `matched.push(...)` on `applied`.
- The applicability selector gate (`html_selector` for HTML responses /
  `json_selector` for JSON responses; empty/absent = apply, malformed = fail
  open) is UNCHANGED and still runs first. There is **no** "global" selector in
  the schema — do NOT invent one.
- `build_entry` semantics are unchanged (≥1 traversed → `Some`); the forwarder
  wraps it in `if applied`. The `/__rre/eval` `summary` is NOT applied-gated
  (it's an authoring preview of what the canvas does).

Result: demo-article → marked only on HTML; demo-json-article → marked only on JSON.

### v2.2 — rename + restructure to `feature_expressions`

- Top-level key renamed: `rre.features_matched` → `rre.feature_expressions`
  (JSON body) and `window.rre.features_matched` → `window.rre.feature_expressions`
  (HTML `<script>`). The header stays `x-rre-feature-<feature_id>`.
- Per-feature entry: replace the parallel `outcome_ids` + `outcome_labels`
  arrays with a SINGLE `expressions` array of objects. Rename every "outcome"
  identifier to "expression" (`expensive_nodes` items too).

New shape — identical for JSON body, HTML window, and the eval `summary`:

```jsonc
{
  "feature_expressions": {
    "<feature_id>": {
      "expressions": [                                  // last-10 traversed, in order
        { "expression_id": "t_body", "expression_label": "trim_json",
          "custom_expression_label": "", "expression_time_ms": "0.02" }
      ],
      "time_took_ms": "1.29",                              // d.dd string (unchanged)
      "expensive_nodes": [                              // top-3 by time DESC, SAME object shape
        { "expression_id": "t_body", "expression_label": "trim_json",
          "custom_expression_label": "", "expression_time_ms": "0.02" }
      ]
    }
  }
}
```

- `expression_id` = canvas node id. `expression_label` = the action kind string
  (proxy has no manifest at runtime → fieldless fallback = raw kind, e.g.
  `trim_json`, `apply_outcome`). `expression_time_ms` / `time_took_ms` are still
  `d.dd` STRINGS (§3). `custom_expression_label` is the node's `custom_label`
  (v2.3), `""` when unset.
- `NodeTiming` gains `custom_label` so the forwarder can carry it through.
- `FeatureEntry` becomes `{ expressions: Vec<Expression>, time_took_ms,
  expensive_nodes: Vec<Expression> }`; the old `ExpensiveNode` collapses into the
  one `Expression` struct (it now carries time too).

### v2.3 — custom expression name (new schema field + dual validation)

- `rule_graph` `Node::Expression` gains
  `custom_label: Option<String>` (`#[serde(default, skip_serializing_if = "Option::is_none")]`).
  Backend `rule_graph.rs` and proxy `graph.rs` stay byte-identical (minus
  `ToSchema`). Update `CONTRACTS.md` §6.
- Validation — IDENTICAL rule in BOTH frontend (zod) and backend
  (`rule_graph_service`): an empty/absent value is VALID (default = no custom
  name); a non-empty value MUST be snake_case matching
  `^[a-z0-9]+(_[a-z0-9]+)*$` (lowercase alphanumerics in single-underscore
  segments; no leading/trailing/double underscore, no uppercase, no spaces).
  - Backend: validate each Expression node; on failure push
    `ValidationDetail { rule_id: "expression_custom_label_invalid",
    loc: "<canvas>.nodes[<node_id>].custom_label", message }`. No new crate —
    manual segment check (split on `_`; every segment non-empty and all bytes in
    `a-z`/`0-9`).
  - Frontend: zod refine on the expression node config form; on failure show the
    message inline: **"Custom name must be snake_case (lowercase letters,
    digits, single underscores) or left empty."**
- Proxy reads `custom_label` off the canvas Expression node and emits it as
  `custom_expression_label` ("" when `None`/empty).

### v2.4 — rename UI label "Apply Outcome" → "Apply"

- `backend/config/node_types.json`: the `apply_outcome` node-type `label`
  changes from "Apply Outcome" to **"Apply"**.
- Update any frontend fixture / hard-coded "Apply Outcome" display string.
- DO NOT change the kind `apply_outcome` (serde discriminator) anywhere — label
  only.

### v2 verification (live E2E, after the stack picks up new binaries)

- JSON `curl -s -D - http://localhost:9000/proxy/v2/content/2-1-1997318`:
  - header `x-rre-feature-demo-json-article: true` present; `x-rre-feature-demo-article`
    **ABSENT** (no-op on JSON).
  - body `rre.feature_expressions["demo-json-article"].expressions` =
    `[{expression_id:"t_body",…}, {expression_id:"a_pw",…}]` with
    `custom_expression_label` + `expression_time_ms` (`d.dd`); `expensive_nodes`
    top-3 DESC; no `demo-article` key.
- HTML `curl -s http://localhost:9000/article.html`: `x-rre-feature-demo-article: true`;
  `x-rre-feature-demo-json-article` ABSENT; `<script>window.rre.feature_expressions=…</script>`.
- A custom name with uppercase/space → backend 422 with
  `expression_custom_label_invalid` and the frontend form shows the inline error;
  empty custom name saves fine.

---

## Source request (verbatim, for disambiguation)

> in the headers, add x-rre-feature-{feature_name}: true for all features that it
> matched. for json, add extra body attr - rre.features_matched: {[feature_name]:
> {outcome_ids, outcome_labels}}. for html, append a script tag at the bottom,
> which will do something like - window.rre.features_matched = {[feature_name]:
> {outcome_ids, outcome_labels}}. outcome_ids and outcome_labels are the last 10
> expressions that it went through. it is not the decision node or end node but
> the expression nodes. Also all empty canvas should have start -> end, if not
> already both in frontend and backend. ... also for
> rre.features_matched[feature_name].time_took_ms = {time}ms show that for both json
> and html. This will give an idea of how much time it took for whole feature.
> similarly extend {outcome_ids, outcome_labels} to {outcome_ids, outcome_labels,
> time_took_ms}. also for rre.features_matched[feature_name].expensive_nodes =
> [{outcome_id, outcome_label, outcome_time_in_ms}] for top 3 nodes sorted in desc
> order. for time_in_ms it format should be d.dd, for ex: 0.10, 1.20, 1.21, 123,
> 123455, 123.12 - showing only last 2 digits after floating point. also the
> highlight seems broken. It no longer shows full highlight from start to end. I
> wanted this to be shown initially and transformation journey to be collapsed.
> when it is expanded then it will show node by node. when it is collapsed again,
> it will highlight full journey from start to end. also in current transformation
> journey it does not give much information on what it is being done. in the image
> it just shows trim_json, expression, body. it should provide both technical and
> non tech info. tech info is node name and the inputs that's given for each label
> and the outcome it got. non tech info is what has been done.
