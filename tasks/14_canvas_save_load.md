# Task 14 — Persist Rule Graph (Save / Save as New Version)

> **Note:** Build fresh React components with TailwindCSS per the design sections; `playground/static/flow.html` is a REFERENCE ONLY for React Flow usage — do NOT copy or embed that monolithic HTML file.

## Goal
Connect the in-memory rule graph from Tasks 11–13 to the backend. Implement both **Save** (PATCH current DRAFT) and **Save as New Version** (POST new version). Edits blocked unless version is `DRAFT`.

## Dependencies
Task 10, 13.

## Acceptance Criteria
- Edit mode is permitted only when `version.status === "DRAFT"`. Non-DRAFT versions render canvas read-only with a banner: "Read-only · Save as New Version to edit".
- Toolbar adds **Save** (visible in edit mode for DRAFT) and **Save as New Version** (always visible when not editing OR for non-DRAFT versions).
- Save:
  - Serializes all three canvases (Anonymous/Registered/Customer) from Zustand to the typed `RuleGraph` payload.
  - PATCH `/api/v1/features/{fid}/versions/{vnum}` with `{ rule_graph }`.
  - On 422 validation error from backend (Task 10), render inline per-node markers + a toast listing failing rules.
- Save as New Version:
  - Prompts for a description string.
  - POST `/api/v1/features/{fid}/versions` (cloned from current LIVE or current state depending on user choice — pick "current canvas state" for MVP).
  - On success, navigates to the new version page.
- Unsaved-changes guard: `beforeunload` and Next.js `useRouter` block navigation when dirty (Radix confirm dialog).
- Optimistic UI: Save toggles the toolbar to "Saving…" then "Saved · {timestamp}".
- E2E test (Playwright) covers: drop nodes → configure → connect → save → reload page → graph persists.

## Implementation Steps
1. `src/lib/canvas/serialize.ts` — convert RF nodes/edges into `RuleGraph` shape from Task 10.
2. `src/lib/canvas/deserialize.ts` — inverse, with `position` from stored coords.
3. `src/components/canvas/SaveBar.tsx` — Save / Save as New Version buttons + status text.
4. Mutations in `src/lib/api/versions.ts`: `patchRuleGraph`, `createVersionFromGraph`.
5. Dirty-tracking selector in store (`isDirty`).
6. Validation error rendering: store has `nodeErrors: Record<NodeId, string>`; nodes get red border when present.
7. Playwright spec under `frontend/e2e/canvas-roundtrip.spec.ts` using a seeded backend (start docker compose stack in CI script).

## Files
- `frontend/src/lib/canvas/serialize.ts`, `deserialize.ts`
- `frontend/src/components/canvas/SaveBar.tsx`
- `frontend/src/lib/api/versions.ts` (extend)
- `frontend/src/state/ruleBuilderStore.ts` (dirty tracking, errors)
- `frontend/e2e/canvas-roundtrip.spec.ts`
- `frontend/playwright.config.ts`
- Tests for serialize/deserialize unit-level.

## Tests
- Unit: serialize then deserialize is identity on a 3-node, 2-edge fixture across all canvases.
- Unit: validation error from API maps to `nodeErrors` keyed by node id.
- Playwright: full round-trip described above.

## Verify
1. Build a graph, click Save → toolbar reads "Saved · just now".
2. Refresh the page → same graph reappears.
3. Try saving an invalid graph (cycle) → red borders + toast.
4. Publish via API → reopen page → canvas locked + banner shown.
5. Save as New Version → routed to new version page with same graph.

## Done When
PR merged. Phase 3 complete — visible deliverable is a working rule-authoring loop with persistence.
