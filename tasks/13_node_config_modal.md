# Task 13 — Decision Node Configuration Modal

## Goal
Open a side panel (or modal) when a decision node is double-clicked, letting the user configure its processor. Cover **Meta Tags** and **Device Type**. Resolves open question §9.1 for MVP.

## Dependencies
Task 12.

## Acceptance Criteria
- Double-click (or click "configure" icon on hover) opens a right-side drawer.
- Drawer form is per-processor:
  - **Meta Tags:** `Tag name` (text, required), `Operator` (select: `contains`, `equals`, `exists`), `Value` (text — hidden when operator is `exists`).
  - **Device Type:** `Operator` (select: `equals`, `contains`), `Value` (select: `mobile`/`desktop`/`tablet`).
- Live validation. Save button disabled until valid. Reflected on the diamond as a sub-label (e.g., `CONTAINS / true`).
- "Delete node" button in drawer header removes node + all connected edges from the store.
- Drawer remains open across canvas pan/zoom; pressing Esc or clicking outside cancels with discard-confirm if dirty.
- Vitest tests cover form rendering per processor, validation transitions, save callback.

## Implementation Steps
1. `src/components/canvas/config/NodeConfigDrawer.tsx` — Radix `Dialog` with side anchoring or a custom flex panel.
2. `src/components/canvas/config/MetaTagsForm.tsx`, `DeviceTypeForm.tsx`.
3. Discriminated rendering on `node.data.processor.type`.
4. Validation via `zod` schemas matching backend serde DTOs in Task 10.
5. Store action `updateNodeProcessor(id, processor)`.
6. Node visual update: `DecisionNode` renders `processor.operator` + `processor.value` above the diamond.

## Files
- `frontend/src/components/canvas/config/NodeConfigDrawer.tsx`
- `frontend/src/components/canvas/config/MetaTagsForm.tsx`
- `frontend/src/components/canvas/config/DeviceTypeForm.tsx`
- `frontend/src/lib/canvas/processorSchemas.ts`
- `frontend/src/state/ruleBuilderStore.ts` (extend)
- `frontend/src/components/canvas/nodes/DecisionNode.tsx` (sub-label)
- `frontend/src/components/canvas/config/__tests__/*.test.tsx`

## Tests
- MetaTagsForm: switching operator to `exists` hides Value input; saving with empty tag is blocked.
- DeviceTypeForm: only allowed enum values selectable.
- Drawer: discard-confirm fires on Esc when dirty.

## Verify
1. Drop Meta Tags node → double-click → drawer.
2. Set `paywall` / `contains` / `true` → Save → diamond shows `CONTAINS / true`.
3. Try empty save → button disabled.
4. Delete node from drawer → diamond and edges gone.

## Done When
PR merged with screenshots of both config forms.
