# Task 11 — Rule Builder Canvas Scaffold (Frontend)

> **Note:** Build fresh React components with TailwindCSS per the design sections; `playground/static/flow.html` is a REFERENCE ONLY for React Flow usage — do NOT copy or embed that monolithic HTML file.

## Goal
Render the Version Detail page (§4.4 header + §4.5 canvas) using React Flow. The canvas is read-only by default with dotted-grid background, zoom controls, full-screen button, template-library placeholder, and the Anonymous/Registered/Customer slider. No drag-drop yet — that lands in Task 12.

## Dependencies
Task 08, 10.

## Acceptance Criteria
- Route `/products/features/{type}/{slug}/{vnum}` renders:
  - Header: "Version {N}" + status pills (LIVE/STAGING/PREV/DRAFT chips).
  - Inline editable description with pencil icon (PATCH on blur/Enter, optimistic).
  - Metadata card: Last Updated By / On.
  - "Rules Builder" section with **Edit** (toggles canvas to editable) and **Analytics** (disabled — Coming soon).
  - Canvas user-type slider with three filled dots; clicking switches which graph is shown.
  - Canvas itself: React Flow with dotted-grid background (`<Background variant="dots" />`), zoom in/out controls bottom-right, full-screen toggle bottom-right, "Template Library" dashed button bottom-right (no-op for MVP).
  - Loads `version.rule_graph[selectedCanvas]` and renders existing nodes/edges in read-only mode (`nodesDraggable={false}` until Edit pressed).
  - Custom React Flow node components for `DecisionNode` (blue diamond via CSS rotation) and `OutcomeNode` (black rectangle).
- Switching canvas tabs preserves edits via Zustand store keyed by canvas.
- Edit mode is local-only here — Save lives in Task 14.
- Vitest tests for the page wrapper + Zustand store.

## Implementation Steps
1. Install `reactflow`, `zustand`.
2. `src/components/canvas/RuleBuilderCanvas.tsx` — `<ReactFlow>` with custom node types.
3. `src/components/canvas/nodes/DecisionNode.tsx` — diamond via `transform: rotate(45deg)` on outer + counter-rotate on label.
4. `src/components/canvas/nodes/OutcomeNode.tsx`.
5. `src/components/canvas/CanvasSlider.tsx` — 3-dot pill toggle.
6. `src/state/ruleBuilderStore.ts` — Zustand store: `{ canvases: Record<CanvasKey, CanvasGraph>, selected, isEditing, setCanvas, setSelected, toggleEdit }`.
7. `src/app/products/features/[type]/[slug]/[vnum]/page.tsx` server component fetches version + outcomes.
8. `src/components/version/VersionHeader.tsx`, `DescriptionEditable.tsx`, `LastUpdatedCard.tsx`.

## Files
- All under `frontend/src/components/canvas/*`, `frontend/src/components/version/*`, `frontend/src/state/*`, the page file, plus tests.

## Tests
- Store: switching canvas keeps both unchanged; `toggleEdit` flips flag.
- Page: pills reflect status; description PATCH on blur calls API once.
- Canvas: with a fixture graph (1 decision + 1 outcome + edge), renders correct counts.

## Verify
1. Open `/products/features/html/dn-article/1` → page laid out per §4.4.
2. Switching Anonymous → Registered → Customer changes the displayed (empty) canvas.
3. Edit pencil opens inline description editor, PATCH succeeds.
4. Zoom controls and full-screen visibly work.

## Done When
PR merged with screenshot showing all three canvases empty + a fixture-loaded graph in one of them.
