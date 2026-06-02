# Version Diff / Compare — Design

**Date:** 2026-06-02
**Scope:** Frontend only (RRE `frontend/`). No backend, proxy, DB, or `CONTRACTS.md` changes.
**Status:** Approved design, ready for implementation plan.

## 1. Goal

From a version's Rule Builder page, let the user pick another version of the same feature and see a
git-diff-style comparison of their rule graphs. The diff:

- Ignores node `position` (x/y) entirely — moving a node is never a change.
- Shows, like a git diff, which nodes were **added**, **removed**, and **modified**, plus **edge
  rewiring** (added/removed wires).
- Renders all three canvases (`anonymous`, `registered`, `customer`) at once — no per-canvas
  navigation.
- Shows a numbered change list; clicking a number scrolls to that canvas and centers the affected
  node/edge.
- Applies git-diff coloring in **both** the canvas and the change list.

## 2. Decisions (locked)

| Decision | Choice |
|---|---|
| Change scope | Added + Removed + Modified nodes **and** edge rewiring (added/removed edges). Position always ignored. |
| Layout | Side change-list (left) + the three canvases stacked vertically (right). |
| Entry point | Full-screen **modal** over the version page (no new route). Read-only. |
| Modified detail | Bullet lists the changed field names **and** old→new values. |
| Edge changes | Shown in the change list (per-canvas sub-group), not canvas-only. |

## 3. Approach

**Dedicated read-only diff renderer.** A pure `diffRuleGraph(old, new)` computes the diff; the modal
renders three React Flow instances from it using new, small, diff-specific node/edge components that
take a `status` and paint git-diff colors. Reuses the manifest label helpers (`nodeTitle`/
`nodeSummary`) for node labels but touches `ruleBuilderStore` **zero** — the live editor session is
never disturbed.

Rejected alternatives:
- *Reuse `DecisionNode`/`ExpressionNode` + the store* — those subscribe to the singleton
  `ruleBuilderStore` (testHighlight, journeyPath, nodeErrors); seeding it for the modal would clobber
  the user's open edits.
- *Text/tree diff, no canvas* — the requirement is explicitly canvas + pan-to-node.

## 4. Diff engine — `lib/canvas/diff.ts` (pure, unit-tested)

```ts
type NodeStatus = "added" | "removed" | "modified" | "unchanged";
type EdgeStatus = "added" | "removed" | "unchanged";

interface FieldChange { field: string; old: unknown; new: unknown }

interface NodeDiff {
  id: string;
  status: NodeStatus;
  node: GraphNode;          // the copy to render: new's for added/modified/unchanged, old's for removed
  changes?: FieldChange[];  // present only when status === "modified"
}
interface EdgeDiff {
  key: string;              // `${source}|${target}|${branch}`
  status: EdgeStatus;
  source: string;
  target: string;
  branch: Branch;
}
interface CanvasDiff { nodes: NodeDiff[]; edges: EdgeDiff[]; changeCount: number }
type RuleGraphDiff = Record<CanvasKey, CanvasDiff>;

function diffRuleGraph(oldRg: RuleGraph, newRg: RuleGraph): RuleGraphDiff;
```

Per canvas:

- **Nodes matched by `id`** (ids are client-generated once and persisted, so they survive cloning and
  "Save as New Version"). For each id in `union(old, new)`:
  - in both, equal after dropping `position` → `unchanged`
  - in both, differ (after dropping `position`) → `modified`, with `changes[]`
  - only in new → `added`
  - only in old → `removed`
- **`changes[]` computation** (modified only): compare `kind`, `custom_label`, and the flattened
  processor/action config (`type` + each config field). Each differing key emits
  `{ field, old, new }` where `field` is the raw config key (e.g. `operator`, `value`, `tag_name`,
  `json_path`, `length`, `outcome_id`), or the literal `"type"` / `"custom_label"` / `"kind"`.
  Field values are emitted raw; the change-list component prettifies the field key to its manifest
  `label` (fallback: the raw key) and stringifies the values.
- **Edges matched by the semantic key `source|target|branch`** (edge ids are not stable across a
  rewire). `added` = key only in new; `removed` = key only in old; otherwise `unchanged`. A branch
  flip on the same wire surfaces as one removed + one added edge.
- `position` is never compared. `changeCount` = count of nodes with status ≠ `unchanged` + edges with
  status ≠ `unchanged`.

This module is pure (no React, no manifest, no store) so it is trivially unit-testable.

## 5. Entry point — `RuleBuilderClient` header

Add a **Compare** button to the existing header action cluster (next to Edit / Make Live / Analytics).
Click opens `CompareDialog`. The button is always available (read-only feature). No route change.

## 6. `CompareDialog`

Full-screen Radix dialog (matches existing dialog patterns in `components/versions/`).

- **Top bar:**
  - Base = the current version A (the page's `vnum`), fixed and labeled.
  - "Compare against" dropdown = every other version of the feature
    (`listVersions(fid, { page_size: 100 })`), each labeled `v{version_number} · {status}`. Default
    selection = the previous version (largest `version_number` < A) when one exists, else unselected
    (empty state prompt).
  - **Swap** button: flips which side is treated as *old* vs *new* (controls green-vs-red meaning).
  - Legend: `+ added` (green) · `− removed` (red) · `~ modified` (amber).
- **Data:** version B fetched via `getVersion(fid, vnumB)` (TanStack Query, key `["version", fid, vnumB]`
  — reuses the editor's cache). B-fetch error → inline `ErrorBanner`; the picker stays usable.
  Selecting A's own vnum is prevented (excluded from the dropdown).
- **Compute:** `diffRuleGraph(old, new)` where old/new are assigned per the swap toggle.
- **Body layout:** left `ChangeList` panel + right column of three stacked `DiffCanvas` sections.
- **Focus coordination:** the dialog holds a `rfInstances` ref map (`Record<CanvasKey,
  ReactFlowInstance | null>`) populated by each `DiffCanvas`'s `onInit`, and a `sectionRefs` map of the
  canvas section DOM nodes. `onFocus(canvasKey, position)` → `sectionRefs[canvasKey].scrollIntoView({
  behavior: "smooth", block: "start" })` then `rfInstances[canvasKey].setCenter(x, y, { zoom: 1.2,
  duration: 400 })`, and sets a transient `focusedNodeId` that pulses a ring on the target node.

## 7. `ChangeList` (left panel)

- Items numbered **globally** 1..N in display order (canvas order: anonymous → registered → customer;
  within a canvas: node changes then edge changes), grouped under a canvas header.
- **Node item:** `n  {+|−|~}  {node label}` colored by status. For `modified`, a sub-line per
  `FieldChange`: `{field label}: {old} → {new}` (e.g. `Operator: contains → equals`). Field label via
  manifest (`spec.fields[].label`), fallback to the raw key; `(none)` for absent values.
- **Edge item:** `n  {+|−}  {sourceLabel} → {targetLabel} ({branch})`.
- Canvas with no changes → muted "No changes" row under its header.
- Clicking any item calls `onFocus(canvasKey, position)` (node items center the node; edge items
  center the midpoint of the two endpoints, falling back to the source node).

## 8. `DiffCanvas` (one per canvas)

- `ReactFlowProvider` + `ReactFlow`, **read-only**: `nodesDraggable={false}`,
  `nodesConnectable={false}`, `deleteKeyCode={null}`, `elementsSelectable` only for click-to-focus.
- Builds RF nodes/edges from the `CanvasDiff` **union**:
  - `unchanged`/`modified`/`added` nodes render at the new version's position; `removed` nodes render
    as ghosts at the old version's position.
  - Same for edges (`added`/`removed`/`unchanged`).
- Registers its instance via `onInit` into the dialog's `rfInstances` map.
- Reuses the existing visual shapes (teal diamond for decision, rounded rect for expression, pill for
  start/end) via new diff components, with status-driven styling:
  - added → green border + tint; removed → red border + tint, dashed, reduced opacity; modified →
    amber border + tint; unchanged → neutral/muted.
  - `focusedNodeId` → transient highlight ring.
- Dot-grid `Background` and `Controls` consistent with `RuleBuilderCanvas`. `fitView` on mount.

## 9. Files

```
lib/canvas/diff.ts                                       (new, pure)
lib/canvas/__tests__/diff.test.ts                        (new, Vitest)
components/canvas/compare/CompareDialog.tsx              (new)
components/canvas/compare/ChangeList.tsx                 (new)
components/canvas/compare/DiffCanvas.tsx                 (new)
components/canvas/compare/DiffEdge.tsx                   (new)
components/canvas/compare/diffNodes/DiffDecisionNode.tsx (new)
components/canvas/compare/diffNodes/DiffExpressionNode.tsx (new)
components/canvas/compare/diffNodes/DiffTerminalNode.tsx (new, start + end)
components/version/RuleBuilderClient.tsx                 (edit: Compare button + dialog mount)
```

No backend/proxy/contract changes. Uses only existing endpoints
(`GET /features/{fid}/versions`, `GET /features/{fid}/versions/{vnum}`).

## 10. Styling tokens

Git-diff palette mapped to existing semantic tokens where available
(`status-live`/emerald = added-green, `danger` = removed-red, amber = modified). Confirm exact token
names against `tailwind.config` during implementation; introduce minimal `diff-*` utility classes only
if no suitable token exists.

## 11. Edge / empty cases

- Identical graphs (ignoring position) → modal shows "No differences" overall; each canvas shows
  "No changes".
- A canvas empty in both versions → "No changes".
- Version B not yet selected → body shows a "Pick a version to compare" prompt.
- Version B fetch fails → `ErrorBanner`, picker stays usable.

## 12. Testing

- **Vitest `diff.test.ts`** (≥80% on `diff.ts`): added-only, removed-only, modified-with-field-changes
  (operator/value/custom_label/type), edge added/removed, branch-flip → remove+add, position-only
  change → `unchanged`, identical graphs → all `unchanged`, empty canvas.
- **Light component test:** `ChangeList` item click invokes `onFocus` with the right
  `(canvasKey, position)`.
- `npm run lint && npm run typecheck && npm run test` green before done.
```
