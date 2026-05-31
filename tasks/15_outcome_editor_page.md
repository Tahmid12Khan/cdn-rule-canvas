# Task 15 — Edit Outcome Page

## Goal
Build the Edit Outcome screen (§4.6) — title/description, components list with reordering, content-control behaviors, placement-specific component shortcuts, and Cancel/Save footer.

## Dependencies
Task 09, 14.

## Acceptance Criteria
- Route `/products/features/{type}/{slug}/{vnum}/transformation/{outcome-id}` accessible from outcome row "Edit" button in the Version Detail page's Outcome section.
- Breadcrumb: `Products / Features / {feature} / Version {N} / Edit Outcome`.
- Page title "Edit an Outcome".
- **Details section:** Title (required, max 100), Description (optional, max 500). `Preview` button top-right opens a placeholder modal ("Preview coming soon").
- **Components section:** Layout/List view toggle (List view is the only one implemented; Layout is disabled with tooltip). Vertically ordered list with drag handle, name/label, action buttons. Drag-drop reorder via `@dnd-kit/sortable`.
- **Placement Specific Components section:** Two buttons `+ Sticky Footer`, `+ Pop-Up` which create components with `placement="sticky_footer"` / `popup"` and open the config modal (Task 16).
- `+ Add A Component Or Form` dashed button opens a component-type picker drawer (MVP types: `html_injection`, `content_truncation`).
- Footer: Cancel returns to version page (discard-confirm if dirty), Save submits all changes via batched PATCH/POST/DELETE per dirty component.
- Outcome editor list section also lives inline on the version detail page (§4.4 "Outcome section") with `+ Add A New Outcome` and per-row `Clone / Edit / Delete` — implement that as a thin reuse of the same list component.
- Tests cover reorder, add, delete, validation, dirty-discard.

## Implementation Steps
1. Install `@dnd-kit/core`, `@dnd-kit/sortable`.
2. `src/app/products/features/[type]/[slug]/[vnum]/transformation/[outcomeId]/page.tsx`.
3. `src/components/outcome/OutcomeEditorPage.tsx` orchestrates state.
4. `src/components/outcome/ComponentRow.tsx`, `SortableComponentList.tsx`.
5. `src/components/outcome/AddComponentDrawer.tsx` — pick component type.
6. `src/components/outcome/PlacementButtons.tsx`.
7. Inline outcome list on version page: `src/components/version/OutcomeListSection.tsx`.
8. API client extensions for batch reorder.
9. Tests: list reorder updates store; add/delete debounced batch; cancel with dirty triggers confirm.

## Files
- Pages + components above
- `frontend/src/lib/api/outcomes.ts`, `components.ts`
- `frontend/src/components/outcome/__tests__/*.test.tsx`
- `frontend/src/components/version/__tests__/OutcomeListSection.test.tsx`

## Tests
- Reorder produces correct `order_index` payload.
- Title validation: empty triggers error and disables Save.
- Cancel with dirty shows confirm; OK navigates away.
- Builtin "Show Content" outcome list row hides Delete + Clone (per Task 09 invariants).

## Verify
1. Click Edit on outcome "DN Regwall 1.0" → editor page renders.
2. Reorder two components → Save → reload → order persists.
3. Click `+ Sticky Footer` → new row appended with sticky-footer badge → config modal opens (Task 16).
4. Try empty title → Save disabled.

## Done When
PR merged with full-page screenshot matching §4.6.
