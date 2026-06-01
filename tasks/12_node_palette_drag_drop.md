# Task 12 — Node Palette & Drag-Drop

> **Note:** Build fresh React components with TailwindCSS per the design sections; `playground/static/flow.html` is a REFERENCE ONLY for React Flow usage — do NOT copy or embed that monolithic HTML file.

## Goal
Build the horizontal scrollable category palette (§4.5) and wire drag-and-drop so users can drop `MetaTags` and `DeviceType` decision nodes plus user-defined Outcome nodes onto the canvas. Connecting nodes with edges is included.

## Dependencies
Task 11.

## Acceptance Criteria
- Palette strip above the canvas with tabs matching §4.5 table order: Session, User, Content, Decision Data, Access, Sub Rules, Split Tests, Integrations, Outcomes, Advanced, Bypass, Gift Tokens, Campaign Tokens, Custom Segments — a 🔍 search tab leads.
- Only **Content → Meta Tags**, **Session → Device Type**, and **Outcomes → \<each outcome from version\>** are draggable for MVP; the rest render as disabled chips with "Coming soon" tooltip.
- Drag a chip onto the canvas → React Flow node appears at drop coordinates. Newly dropped decision nodes start with default processor config (validated to require user edit before save).
- Connecting two nodes by dragging from a source handle to a target handle creates an edge. Decision nodes have **two** source handles labeled `YES` and `NO` (oval pill labels per §4.5).
- Outcome nodes are terminals (target handle only).
- Edit mode required for any of this; in view mode everything is locked.
- Palette search filters chips across categories by name (debounced).
- Tests for palette filtering, drag-drop reducer, edge creation.

## Implementation Steps
1. `src/components/canvas/palette/NodePalette.tsx` — tabs + chip lists.
2. `src/components/canvas/palette/NodeChip.tsx` — drag source using HTML5 DnD (set `dataTransfer` with node-type payload).
3. Canvas drop handler in `RuleBuilderCanvas`: `onDrop` reads payload, computes RF coords via `screenToFlowPosition`, dispatches `addNode` to store.
4. Custom edge component `LabeledEdge.tsx` rendering the YES/NO oval label.
5. `DecisionNode` exposes two source `Handle`s with `id="yes"` / `id="no"`.
6. Store mutations: `addNode`, `addEdge`, `removeNode`, `removeEdge`, `updateNodePosition`.
7. Disabled chip pattern + tooltips via `radix-ui/react-tooltip` (or `@radix-ui/react-popover`).
8. Tests: store reducer unit tests + RTL test for drag-drop using `react-dnd-test-backend` OR direct dispatch (HTML5 DnD in jsdom is unreliable — prefer dispatching the underlying store action).

## Files
- `frontend/src/components/canvas/palette/NodePalette.tsx`
- `frontend/src/components/canvas/palette/NodeChip.tsx`
- `frontend/src/components/canvas/edges/LabeledEdge.tsx`
- `frontend/src/state/ruleBuilderStore.ts` (extend)
- `frontend/src/lib/canvas/nodeTemplates.ts` (chip definitions + which are enabled)
- `frontend/src/components/canvas/__tests__/store.test.ts`
- `frontend/src/components/canvas/__tests__/NodePalette.test.tsx`

## Tests
- Store: `addNode` increments; `addEdge` rejects edge from outcome (no source handle).
- Palette: search "device" shows only Device Type chip; disabled chips have `aria-disabled="true"`.

## Verify
1. Open version page, hit Edit, drag "Meta Tags" → diamond appears on canvas.
2. Drag "Device Type" → second diamond.
3. Drag outcome "Show Content" → black rectangle.
4. Connect Meta Tags → Device Type → Show Content with YES branches; YES label visible.
5. Switch canvas to Registered → empty; switch back → graph still there.

## Done When
PR merged with screen recording (GIF) of the drag-drop + connect sequence in description.
