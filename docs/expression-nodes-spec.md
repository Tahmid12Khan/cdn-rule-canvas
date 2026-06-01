# Spec: Expression-node flow + Transformation Journey

**Status:** FROZEN contract for this change. Every implementing agent reads this
file first and treats the code blocks here as byte-authoritative (the backend
`rule_graph.rs` and proxy `graph.rs` mirrors must be identical minus `ToSchema`).
This extends `CONTRACTS.md` §6/§8 — update `CONTRACTS.md` to match when done.

## 0. Why

Today a canvas selects ONE `outcome_id`; the proxy then applies that outcome's
DB components to the body. We are inverting this into an **in-graph action
pipeline**: `START → (decisions route) → expression/action nodes (mutate body,
pass through) → END`. zen still routes; the proxy applies the matched path's
actions in order via the existing `json_apply` primitives.

Five deliverables:
1. New node taxonomy: real persisted **Start**, **Expression** (action), **End**; **Outcome removed**.
2. Two new JSON action node types: **`trim_json`**, **`add_attribute`**.
3. **`apply_outcome`** expression action = the migration target for every old Outcome node (keeps `rre.outcomes` alive; HTML + JSON).
4. New validation: exactly one Start + ≥1 End per non-empty canvas; every node reachable from Start must reach an End.
5. **Transformation Journey**: a node-by-node stepper in the Test panel showing the body after each node, navigable by ←/→ keys and big clickable arrows.

---

## 1. Node taxonomy — `backend/src/schemas/rule_graph.rs` (+ proxy `graph.rs` mirror, drop `ToSchema`)

Replace the `Node` enum. `ProcessorConfig` (proxy: `ProcessorRef`) is REUSED for the
expression `action` (same `{ "type": "<kind>", <fields…> }` open shape).

```rust
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Node {
    /// Entry marker. Exactly one per non-empty canvas. No incoming edges; one outgoing edge.
    Start { id: String, position: Position },                                 // kind = "start"
    /// Branches yes/no via a processor. UNCHANGED.
    Decision { id: String, processor: ProcessorConfig, position: Position },  // kind = "decision"
    /// Performs one body action and passes through. Exactly one outgoing edge.
    Expression { id: String, action: ProcessorConfig, position: Position },   // kind = "expression"
    /// Terminal. Stops the flow. Zero outgoing edges. ≥1 per non-empty canvas.
    End { id: String, position: Position },                                   // kind = "end"
}
```

`Node::id()` covers all four. Add helpers: `is_start`, `is_end`, `is_expression`,
`is_decision`. **Remove** `Outcome` and `is_outcome`.

`Edge`/`Branch`/`Position` UNCHANGED. `branch` is only meaningful when the edge
source is a `Decision`; for `Start`/`Expression` sources there is exactly one
outgoing edge and the wire value is `"yes"` by convention (ignored by translator
and evaluator). Edges never originate from `End`.

---

## 2. Manifest — `backend/src/schemas/node_type.rs` + `backend/config/node_types.json`

### Schema additions (`node_type.rs`)
- Add to `NodeTypeSpec`: `#[serde(default)] pub node_kind: NodeKind` where
  `enum NodeKind { #[default] Decision, Expression }` (serde snake_case). Drives
  which RF node type the frontend creates on drop, and which validation path applies.
- Add `Control::OutcomeSelect` (serde `outcome_select`) — a dynamic dropdown of
  the version's outcomes (options are NOT in the manifest; supplied by the client/validator).

### New node types (`node_types.json`, appended to `node_types`)

```jsonc
{
  "kind": "trim_json", "label": "Trim JSON", "category": "json",
  "applies_to": "json", "node_kind": "expression",
  "summary": "Trims a JSON array at a path to at most N items (min(actual, length)).",
  "fields": [
    { "name": "json_path", "label": "JSON path", "control": "text", "required": true,
      "default": "", "placeholder": "$.body", "required_message": "Enter the JSONPath to the array" },
    { "name": "length", "label": "Max length", "control": "number", "required": true,
      "default": 0, "placeholder": "0", "required_message": "Enter the maximum array length" }
  ],
  "output": { "branches": [ { "id": "out", "label": "Next" } ] }
},
{
  "kind": "add_attribute", "label": "Add Attribute", "category": "json",
  "applies_to": "json", "node_kind": "expression",
  "summary": "Sets (upserts) a value at a JSON path, creating missing parents; replaces if present.",
  "fields": [
    { "name": "json_path", "label": "JSON path", "control": "text", "required": true,
      "default": "", "placeholder": "$.paywall_show", "required_message": "Enter the JSONPath to set" },
    { "name": "value", "label": "Value", "control": "text", "required": true,
      "default": "", "placeholder": "<html>…</html>", "required_message": "Enter the value to set" }
  ],
  "output": { "branches": [ { "id": "out", "label": "Next" } ] }
},
{
  "kind": "apply_outcome", "label": "Apply Outcome", "category": "content",
  "applies_to": "all", "node_kind": "expression",
  "summary": "Applies a saved outcome's components to the body, then continues.",
  "fields": [
    { "name": "outcome_id", "label": "Outcome", "control": "outcome_select", "required": true,
      "default": "", "required_message": "Pick an outcome to apply" }
  ],
  "output": { "branches": [ { "id": "out", "label": "Next" } ] }
}
```

`json_path`/length use the **JSONPath** dialect already in
`proxy/src/domain/processors/json_expression.rs` (`serde_json_path`), capped at 1000 chars.

Update the `committed_manifest_loads_and_is_verbatim` test's expected kind list to
include `trim_json`, `add_attribute`, `apply_outcome`.

---

## 3. Validation — `backend/src/services/rule_graph_service.rs` (+ frontend `graphValidation.ts` mirror)

Remove `outcome_terminal`, `outcome_branch_forbidden`, `outcome_ref_exists`,
`outcome_reachable`. Keep `edge_endpoint_exists`, `branch_unique`, `no_cycles`,
`root_in_nodes`, `processor_kind_known`, `processor_field_required`,
`processor_field_option` (the processor checks now also run on **expression**
nodes' `action` for manifest kinds). Add:

| `rule_id` | Rule |
|---|---|
| `start_present` | A non-empty canvas has exactly ONE `start` node (0 → error "missing start"; ≥2 → error). |
| `end_present` | A non-empty canvas has ≥1 `end` node. |
| `start_no_incoming` | No edge targets a `start` node. |
| `start_single_out` | A `start` node has exactly one outgoing edge. |
| `expression_single_out` | An `expression` node has exactly one outgoing edge. |
| `end_terminal` | An `end` node has zero outgoing edges. (replaces `outcome_terminal`) |
| `edge_source_kind` | Edges originate only from `start`/`decision`/`expression`, never `end`. (replaces `outcome_branch_forbidden`) |
| `all_paths_reach_end` | Every node reachable from the Start node can reach an `end` node (dead-ends invalid). Anchored at the Start node. (replaces `outcome_reachable`) |
| `apply_outcome_ref_exists` | An `expression` node whose `action.type == "apply_outcome"` has `action.outcome_id` (or `action.fields["outcome_id"]`) present in `valid_outcome_ids`. (replaces `outcome_ref_exists`) |

- **Root** for `all_paths_reach_end` is now the unique `start` node (no more
  no-incoming heuristic). Empty canvas (zero nodes) stays valid (no start/end required).
- `processor_field_option` skips `outcome_select` controls (dynamic options); the
  `apply_outcome_ref_exists` rule covers that field instead. `processor_field_required`
  still applies to `outcome_select` (must be non-empty).
- The validator’s `valid_outcome_ids` arg stays. Rewrite the unit tests: replace
  the `outcome(...)` helper with `start(...)`/`expression_apply(...)`/`end(...)`
  and update every test graph to the `start → … → end` shape. Keep coverage ≥ the
  current 24 tests.

Frontend `graphValidation.ts`: mirror the structural subset that runs client-side
today (cycles, reachability) but retargeted: root = the start node; reachability =
"can reach an `end`"; plus missing-start / missing-end / dead-end messages. Update
`CYCLE_MESSAGE`/reachability copy and the `validationMapping.ts` server-reason
substrings to stay byte-aligned.

---

## 4. Proxy runtime — `proxy/src/domain/{graph,translator,evaluator}.rs`, `applier/json_apply.rs`, `forwarder.rs`

**Routing stays in zen; mutation moves to the forwarder via `json_apply`.**

### Translator (`to_decision_content`)
- **Root = the `start` node.** Wire `input → entry(successor_of_start)`. (Start
  itself emits no JDM node; its single edge’s target is the real entry.)
- `Decision` → CustomNode + SwitchNode (UNCHANGED).
- `Expression { id, action }` → one zen `ExpressionNode` `<id>__expr` that emits a
  trace marker, e.g. `{ "exprNode": "'<id>'" }` (a string literal). It must connect
  forward to its single successor.
- `End { id }` → zen `OutputNode` `<id>__out` (terminal).
- Edge wiring by **source kind**: Decision source → `<src>__switch` + `source_handle
  = "<src>:yes|no"` (unchanged). Start/Expression source → from the source’s exit
  JDM id (`input` for start’s edge is handled by the root wiring; expression exits
  at `<src>__expr`) with `source_handle = None`.

### Evaluator (`GraphEvaluator::evaluate`) — return ORDERED actions, not one outcome
Change the hot-path return from `Option<Uuid>` to an ordered list of matched
expression actions:

```rust
pub struct MatchedAction { pub node_id: String, pub action: serde_json::Value } // action = {type, …fields}
// evaluate(...) -> Vec<MatchedAction>   // empty = no-op / fail-open
```

Run zen **with `trace: true`** (user-approved; small overhead), map the JDM trace
back to canvas nodes (reuse the `__expr` suffix logic already in
`evaluate_with_trace`), keep only `expression` nodes in `order` order, and resolve
each to its `action` config from the canvas. Fail-open → empty Vec.

### Applier (`json_apply.rs`) — add the new ops
- `trim_json(root, json_path, length)`: query the array at `json_path`; truncate to
  `min(actual_len, length)`. Non-array / missing path → no-op (fail-open). Reuse the
  `serde_json_path` parse + a mutable navigation (mirror `set_path`’s walk). `length < 0`
  treated as 0.
- `add_attribute(root, json_path, value)`: = existing `set_path(create=true)` semantics
  (upsert + parent creation). Accept a JSONPath like `$.paywall_show`; map dotted/`$`
  segments to the internal `Seg` path (reuse `json_path::parse`).
- Add a single entry point the forwarder calls per matched action:
  `apply_action_json(&mut Value, action: &Value) -> bool` dispatching on `action["type"]`
  (`trim_json`, `add_attribute`, `apply_outcome` → look up the outcome by id and run the
  existing `apply_outcome_json` components). Returns whether the body changed.
- HTML path: an analogous `apply_action_html` — only `apply_outcome` is meaningful
  (delegates to `orchestrator::apply_outcome`); `trim_json`/`add_attribute` are JSON-only
  no-ops (warn + skip).

### Forwarder (`forwarder.rs`)
Replace `let (outcome_id, …) = evaluate(...); let outcome = …find_outcome…; apply_outcome[_json]`
with: get the ordered `Vec<MatchedAction>`, fold each over the body in order
(`apply_action_json` for JSON, `apply_action_html` for HTML). `applied = any action
changed the body`. Keep the same logging fields (`eval_ms`, `transform_ms`,
`apply_status`); `apply_status = "ok"` iff applied, else `"skipped"`. Keep fail-open
on every error. `find_outcome` / `ActiveVersionRead.outcomes` stay (used by `apply_outcome`).

---

## 5. Transformation Journey — eval test endpoint + frontend stepper

### Backend/proxy: extend the `/__rre/eval` test response (additive)
`evaluate_with_trace` already returns ordered steps. Add a `journey` array where each
entry is the body **after** that node’s action is applied (decisions/start/end don’t
mutate, so their `body_after` equals the running body):

```jsonc
// added to the eval-test response JSON (alongside traversed_node_ids, matched_*)
"journey": [
  { "index": 0, "node_id": "start",  "kind": "start",      "label": "Start",         "branch": null,  "body_after": <json|string> },
  { "index": 1, "node_id": "d_api",  "kind": "decision",   "label": "JSON Expression","branch": true,  "body_after": <…unchanged…> },
  { "index": 2, "node_id": "t_body", "kind": "expression", "label": "Trim JSON",      "branch": null,  "body_after": <…trimmed…> },
  { "index": 3, "node_id": "a_pw",   "kind": "expression", "label": "Add Attribute",  "branch": null,  "body_after": <…with paywall_show…> },
  { "index": 4, "node_id": "end",    "kind": "end",        "label": "END",            "branch": null,  "body_after": <…final…> }
]
```
Compute by replaying the matched path: start with the input body; at each expression
node apply its action (reuse §4 appliers) and snapshot. `body_after` is a JSON value for
JSON features, a string for HTML. `label` comes from the manifest (`kind → label`) or the
node kind. Keep this on the **test** path only (the hot proxy path stays trace-free aside
from the routing trace already added).

### Frontend `TestPanel.tsx` + `lib/api/evalTest.ts`
- Extend the `evalTest` client type with the `journey` array.
- Add a **"Transformation Journey"** view that appears after a successful run:
  - A stepper showing the current step’s `label` + `branch` and a pretty-printed
    `body_after` (JSON.stringify(…, null, 2) in a `<pre>` for JSON; raw for HTML).
  - **Big** clickable Prev/Next arrow buttons (≥40px touch target, e.g. `h-10 w-10`,
    `aria-label` "Previous node"/"Next node"), disabled at the ends.
  - **Keyboard**: ArrowRight/ArrowLeft advance/retreat when the journey view is focused
    (attach a keydown handler on the journey container with `tabIndex={0}`; don’t hijack
    arrows globally — only when the container or its children have focus). Clamp at
    `[0, journey.length-1]`.
  - Step indicator "Step k of N" + the node id. Also drive the existing canvas
    `setTestHighlight` so the current journey node glows as you step (optional but nice:
    set the highlight to the current step’s node).
  - Name in code/UI: **Transformation Journey** (component `TransformationJourney`).
- Add a Vitest test: given a stubbed journey, ← / → and the buttons move the index and
  render the right `body_after`, clamped at both ends.

---

## 6. Migration — `backend/migrations/0007_expression_nodes.{up,down}.sql`

Transform every `versions.rule_graph` JSONB. We are on dev; keep it pragmatic but
**idempotent** and reversible. For EACH canvas (`anonymous`/`registered`/`customer`) of
each row:
1. **Outcome → Expression**: every node `{kind:"outcome", id, outcome_id, position}` →
   `{kind:"expression", id, action:{type:"apply_outcome", outcome_id:<uuid>}, position}`.
2. **Add END**: if the canvas has ≥1 node and no `end` node, append
   `{kind:"end", id:"end", position:{x,y}}` (place below). Add one edge from EACH
   former-outcome (now expression) node → `end` with `branch:"yes"`
   (`id:"e_end_<exprId>"`).
3. **Add START**: if ≥1 node and no `start` node, find the old root (the unique node
   with no incoming edge among the pre-migration nodes); append
   `{kind:"start", id:"start", position:{…}}` and one edge `start → oldRoot`
   (`branch:"yes"`, `id:"e_start"`). If the old root is ambiguous, anchor START at the
   first node.
4. Leave `root_node_id` as-is (or set to `"start"`).

SQL via a `DO $$ … $$` PL/pgSQL block + `jsonb` builders is acceptable; if pure SQL
proves unwieldy, ship a Rust one-shot binary `backend/src/bin/migrate_rule_graphs.rs`
(loads rows, transforms with the new `Node` serde, writes back) and document running it
— but a real SQL migration in `0007` is strongly preferred so `sqlx::migrate!` applies it
on backend restart. `down.sql`: best-effort reverse (expression(apply_outcome) → outcome,
drop the injected start/end nodes + their edges). `rre.outcomes`/`components` tables are
**NOT** dropped.

---

## 7. Seed — `backend/src/bin/seed_demo.rs`

Add feature **`dn-json-article`** (type `json`) with one published version whose
**anonymous** canvas implements:

```
start ──> json_expression{ json_path:"$.api", operator:"equals", value:"dn-article" }
              ├─ yes ─> trim_json{ json_path:"$.body", length:0 }
              │            └─> add_attribute{ json_path:"$.paywall_show", value:"<html>paywall_showed</html>" }
              │                   └─> end
              └─ no  ─> end
```

i.e. when `$.api == "dn-article"`: empty `$.body` and set
`$.paywall_show = "<html>paywall_showed</html>"`; otherwise pass through to END. Seed
must be idempotent (upsert by feature key) like the existing `dn-article` seed. If the
user’s running DB already has a `dn-json-article` feature row, the seed should update its
version’s rule_graph rather than duplicate.

---

## 8. Per-lane file checklist

**Backend (lane A):** `schemas/rule_graph.rs` (enum), `schemas/node_type.rs`
(`node_kind`, `outcome_select`), `config/node_types.json` (3 node types),
`services/rule_graph_service.rs` (rules + tests), `migrations/0007_*`,
`bin/seed_demo.rs`, `CONTRACTS.md` §6 + node-type manifest + validation table.
Gate: `make backend-check`.

**Proxy (lane B):** `domain/graph.rs` (mirror enum — identical minus ToSchema),
`domain/translator.rs` (start root, expression→ExpressionNode, end→OutputNode, edge
wiring by source kind), `domain/evaluator.rs` (`Vec<MatchedAction>` + journey body
snapshots), `domain/applier/json_apply.rs` (`trim_json`, `add_attribute`,
`apply_action_json`), an `apply_action_html`, `forwarder.rs` (fold actions),
`eval.rs`/eval handler (journey field), proxy tests (new `trim_json`/`add_attribute`
apply tests + translator/forwarder). Gate: `make proxy-check`.

**Frontend core (lane C):** `lib/canvas/types.ts` (node data + RFNode union),
`lib/canvas/{serialize,deserialize}.ts` (persist start/expression/end; stop stripping
start), `lib/canvas/nodeTemplates.ts` (Trim JSON + Add Attribute + Apply Outcome chips;
auto-inject one start + one end on init), `lib/canvas/manifest.ts` (node_kind →
RF type; outcome_select control), `components/canvas/nodes/{StartNode,ExpressionNode,
EndNode}.tsx` (+ remove/repurpose `OutcomeNode`), `config/{NodeConfigDrawer,
GenericNodeForm}.tsx` (expression form; outcome_select dropdown), `graphValidation.ts`
+ `validationMapping.ts`, store (`ruleBuilderStore`: start/end injection, non-deletable
start+end). Gate: `make frontend-check`.

**Frontend journey (lane D, after C):** `components/canvas/TransformationJourney.tsx`,
wire into `TestPanel.tsx`, extend `lib/api/evalTest.ts`, Vitest. Gate: `make frontend-check`.

## 9. Cross-layer invariants (must hold)
- `backend rule_graph.rs Node` ≡ `proxy graph.rs Node` (byte-identical minus `ToSchema`/doc).
- manifest `kind` == rule_graph `type` == (for decisions) proxy `CanvasProcessor::kind()`.
- Wire JSON for a node is `{ "kind": "...", "id": "...", "position": {...}, <"processor"|"action">: { "type": "...", … } }`.
- Empty canvas → valid. Non-empty canvas → needs exactly one start, ≥1 end, all reachable nodes reach an end.
- Fail-open everywhere in the proxy: any eval/apply error serves the original body.
</content>
</invoke>
