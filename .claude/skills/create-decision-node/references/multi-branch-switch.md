# Multi-branch switch decision nodes (3+ outcomes)

Today the whole stack assumes a decision yields exactly **yes** or **no**. The
`Branch` enum is `{ Yes, No }` in two places, the translator emits exactly two
`SwitchStatement`s per decision, and the frontend draws two edge handles. To
route a single decision into 3+ branches (e.g. `low` / `mid` / `high`), you must
change every one of those. This is a coordinated, cross-cutting change — do NOT
attempt it as a copy-paste of Part A.

Decide first **whether you actually need it.** Two binary decisions chained
(`is high? → yes; else is mid? → yes; else low`) cover most cases with zero
schema churn. Only go multi-branch when the fan-out is genuinely N-way and
chaining would bloat the canvas.

## Two viable designs

**Design 1 — keep `Branch` binary, add more decision nodes.** Recommended.
Model the N-way choice as a small tree of yes/no decisions. No engine, schema, or
frontend changes. This is why Part A is "99% of cases".

**Design 2 — generalize `Branch` to a named label.** Required only for a true
single-node N-way switch. The touch points:

### Proxy

- `proxy/src/domain/processors/mod.rs`
  - Replace `enum Branch { Yes, No }` with an open label, e.g.
    `pub struct Branch(pub String)` or a fixed `enum` of your labels. Update
    `as_str()` / `into_variable()` so the output Variable is still
    `{ "branch": "<label>" }` (the SwitchNode reads `branch == '<label>'`).
  - `ProcessorOutcome.branch` now carries the label your processor chose.
- `proxy/src/domain/graph.rs`
  - The edge `Branch` enum (`#[serde rename_all snake_case] { Yes, No }`) must
    gain the new labels — or also become a string. **Keep it identical to the
    backend mirror.**
- `proxy/src/domain/translator.rs`
  - `to_decision_content` hard-codes exactly two statements
    (`"{id}:yes"` / `"{id}:no"` with conditions `branch == 'yes'` / `'no'`).
    Emit **one `SwitchStatement` per possible label**, with statement id
    `"{id}:<label>"` and condition `branch == '<label>'`. The canvas-edge loop's
    `source_handle` (`"{src}:{branch}"`) must match these ids exactly — that
    coupling is the whole routing contract (CONTRACTS §8 "Branch routing").
  - Decide the label set: either fixed per processor kind, or derived from the
    processor config. Whatever you choose, the translator must enumerate the
    same labels the processor can emit, or a branch silently drops to no edge.

### Backend

- `backend/src/schemas/rule_graph.rs` — mirror the edge `Branch` change (with
  `ToSchema`).
- `rule_graph_service::validate` — the `branch_unique` rule ("at most one
  outgoing edge per branch") iterates over branch values. Extend it to the new
  label set so a decision can have one edge per label without false 422s.

### Frontend

- `frontend/src/lib/canvas/types.ts` — `Branch = "yes" | "no"` and `RFEdge`'s
  `sourceHandle` must include the new labels.
- The decision node component (`components/canvas/nodes/DecisionNode.tsx`) draws
  source handles per branch — add a handle per label.
- The labeled-edge renderer + edge-creation logic must carry the new label.
- Validation/UX: the palette/save path assumes ≤2 outgoing edges per decision;
  audit anything that hard-codes yes/no.

## Verification

Beyond `make {proxy,backend,frontend}-check`, add a **translator test**
(`proxy/tests/translator.rs`) asserting the JDM output has one `SwitchStatement`
per label with the correct `branch == '<label>'` condition and that a canvas
edge on `<label>` produces a `DecisionEdge` whose `source_handle` is
`"{id}:<label>"`. The routing breaks silently if statement ids and edge handles
drift — the test is the guardrail.
