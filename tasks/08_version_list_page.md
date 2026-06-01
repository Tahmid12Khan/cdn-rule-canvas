# Task 08 — Feature Version List Page (Frontend)

> **Note:** Build fresh React components with TailwindCSS per the design sections; `playground/static/flow.html` is a REFERENCE ONLY for React Flow usage — do NOT copy or embed that monolithic HTML file.

## Goal
Build the Feature Version List page (§4.3) — the deployment-status header row, the versions table with status pills, search, pagination, row context menu, and the `+ Add A New Version` action.

## Dependencies
Task 06, 07.

## Acceptance Criteria
- Route `/products/features/{type}/{slug}` renders:
  - Feature name as H1.
  - **Deployment status row:** `STAGING V{n}` (amber) + `LIVE V{n}` (green) pills, populated from `feature.staging_version_id` / `feature.live_version_id`. Hidden if unset.
  - "Versions" section heading with `Access Permissions` (disabled in MVP, tooltip "Coming soon") and `+ Add A New Version` buttons.
  - Search input filters by description / version number (debounced 300ms, server-side query param).
  - Table columns: Version, Description, Created By, Last Updated, Status (pill), Actions (`...`).
  - Status pills: LIVE green, STAGING amber, PREV dark gray, DRAFT outlined.
  - `...` overflow menu: Unpublish (disabled unless LIVE or STAGING), Edit Description (inline dialog), Delete (confirmation dialog; disabled unless DRAFT/PREV).
  - Pagination component "Results X–Y of Z" with prev/next + numbered pages.
- Click version number or description → navigates to `/products/features/{type}/{slug}/{vnum}` (placeholder for Task 11+).
- Comprehensive Vitest tests.

## Implementation Steps
1. `src/app/products/features/[type]/[slug]/page.tsx` — server component fetches feature + first page of versions in parallel.
2. `src/components/versions/StatusPill.tsx` — variant for each status.
3. `src/components/versions/DeploymentStatusRow.tsx`.
4. `src/components/versions/VersionsTable.tsx`, `VersionRow.tsx`, `RowActionsMenu.tsx`.
5. `src/components/versions/SearchInput.tsx` (`useDebouncedValue` hook).
6. `src/components/versions/Pagination.tsx`.
7. `src/components/versions/AddVersionDialog.tsx`, `EditDescriptionDialog.tsx`, `ConfirmDeleteDialog.tsx`.
8. Extend `src/lib/api/versions.ts` with all version endpoints (incl. publish/unpublish — buttons exposed in later tasks).
9. Optimistic mutations + cache invalidation via TanStack Query.

## Files
- `frontend/src/app/products/features/[type]/[slug]/page.tsx`
- `frontend/src/components/versions/*` (as above)
- `frontend/src/lib/api/versions.ts`
- `frontend/src/lib/hooks/useDebouncedValue.ts`
- `frontend/src/components/versions/__tests__/*.test.tsx`

## Tests
- `VersionsTable.test.tsx`: renders rows with correct pills.
- `RowActionsMenu.test.tsx`: items disabled per status.
- `AddVersionDialog.test.tsx`: submits + closes + refetches.
- `Pagination.test.tsx`: page change emits correct query.

## Verify
1. Create feature, create 3 versions, publish per Task 07 verify.
2. Page header shows `STAGING V2` (amber) + `LIVE V1` (green).
3. Search "Testing" filters rows.
4. `...` on v3 (DRAFT) → Delete enabled.
5. `...` on v1 (LIVE) → Unpublish enabled, Delete disabled.

## Done When
PR merged with full-page screenshot matching §4.3 layout.
