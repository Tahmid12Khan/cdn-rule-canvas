# Task 10 — Rule Graph Schema & Validation

## Goal
Formalize the rule graph JSON schema (§3.5) covering Decision Nodes (with MetaTags + DeviceType as the MVP processors), Outcome terminals, and edges. Add server-side validation that the graph is well-formed before persisting in `version.rule_graph`.

> **Note:** This task owns the **canvas `rule_graph` serde schema + validation only**. The exact same persisted JSON is later consumed at runtime by the proxy's **JDM translator** (Task 18), which converts `CanvasGraph` → `zen_engine::model::DecisionContent` and evaluates it with `zen_engine::DecisionEngine`. Keep the schema stable and well-documented so the translator can rely on it; do **not** add proxy/evaluation concerns here.

## Dependencies
Task 07, 09.

## Acceptance Criteria
- serde models in `src/schemas/rule_graph.rs`:
  - `CanvasGraph { nodes: Vec<Node>, edges: Vec<Edge>, root_node_id: Option<Uuid> }`.
  - `Node` is an internally-tagged enum by `kind` (`#[serde(tag = "kind")]`):
    - `DecisionNode { id, kind="decision", processor: ProcessorConfig, position: {x,y} }`
    - `OutcomeNode { id, kind="outcome", outcome_id: Uuid, position }`
  - `ProcessorConfig` is an internally-tagged enum by `type` (`#[serde(tag = "type")]`):
    - `MetaTagsProcessor { type="meta_tags", tag_name: String, operator: "contains"|"equals"|"exists", value: Option<String> }`
    - `DeviceTypeProcessor { type="device_type", operator: "equals"|"contains", value: "mobile"|"desktop"|"tablet" }`
  - `Edge { id, source_node_id, target_node_id, branch: "yes"|"no" }`
  - `RuleGraph { anonymous: CanvasGraph, registered: CanvasGraph, customer: CanvasGraph }`
- Validation rules (raise `GraphValidationError`):
  1. All edge endpoints exist in nodes.
  2. Decision nodes have at most one outgoing edge per branch label.
  3. No cycles (DFS check).
  4. Outcome nodes have zero outgoing edges.
  5. All referenced `outcome_id` values exist on the version.
  6. `root_node_id` (if set) is in nodes.
- `PATCH /api/v1/features/{fid}/versions/{vnum}` accepts `rule_graph` and runs validation. Invalid → 422 with structured error list.
- Helper `service::validate_graph(version, graph)` is unit tested with explicit failure cases.

## Implementation Steps
1. Schema models as serde internally-tagged enums (`#[serde(tag = "kind")]` for `Node`, `#[serde(tag = "type")]` for `ProcessorConfig`). Untagged variants rejected by serde at deserialize time.
2. `src/services/rule_graph_service.rs::validate(version_id, graph)` — fetches outcomes once, runs all checks.
3. Replace the loose `serde_json::Value` graph type from Task 07 with the typed `RuleGraph`; provide migration of existing rows if needed (empty graphs only at this point).
4. Tests cover: valid empty graph, missing outcome ref, duplicate yes-branch, cycle, unknown processor type → all rejected.

## Files
- `backend/src/schemas/rule_graph.rs`
- `backend/src/services/rule_graph_service.rs`
- `backend/src/services/version_service.rs` (call validator on update)
- `backend/tests/rule_graph_validation.rs` (unit)
- `backend/tests/version_patch_graph.rs` (integration)

## Tests
- 10+ unit cases for `validate(...)` covering each rule.
- API: 422 with `[{loc, msg, rule_id}]` payload on invalid graphs.

## Verify
1. PATCH a version with a hand-crafted valid graph (1 decision → 2 outcome leaves) → 200.
2. Re-PATCH with a cycle → 422 with `rule_id="cycle"`.
3. `cargo test` green.

## Done When
PR merged. Graph schema doc auto-generated to `docs/rule_graph_schema.md` via a `schemars` JSON-schema export (small binary in `src/bin/`). The proxy team (Task 18) confirms the published schema is sufficient input for the JDM translator (every processor `type` and edge `branch` value maps to a translatable JDM construct).
