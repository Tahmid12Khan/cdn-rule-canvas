# Task 06 — Features List Page (Frontend)

## Goal
Build the Features list screen (§4.2) and the global app shell (top nav per §4.1). Wire to the real backend with TanStack Query. No version interaction yet — clicking a feature navigates to a stub Version List page (filled in Task 08).

## Dependencies
Task 03, 05.

## Acceptance Criteria
- Top navigation bar matches §4.1: sections **Journey, Products (active), Identity, B2B, Delivery**; tenant pill ("nhst-testing"); right-side help/settings/profile icons. Dark bar, teal accent.
- Route `/products/features` lists features with breadcrumb `Products / Features`.
- Each row is a card linking to `/products/features/{type}/{id}`.
- Empty state: "No features yet" + `+ Add A New Feature` button (creates via modal POST).
- Loading skeleton + error banner with retry.
- All API state goes through TanStack Query.
- Vitest + RTL tests for empty, populated, error, and create-modal happy path.

## Implementation Steps
1. Create `src/app/layout.tsx` global shell with `<TopNav />`, `<Breadcrumbs />` slot, `<main>`.
2. `src/components/nav/TopNav.tsx` — exact section list + tenant chip + icon trio.
3. `src/components/nav/Breadcrumbs.tsx` — derives from `usePathname`.
4. `src/app/products/features/page.tsx` — server component fetches initial list via `fetch(API)` then hydrates Query cache; OR client component with `useQuery`. Pick client for simplicity now.
5. `src/lib/api/features.ts` — `listFeatures`, `createFeature`, schemas via `zod`.
6. `src/components/features/FeatureCard.tsx`, `FeatureCreateModal.tsx`, `EmptyState.tsx`.
7. `src/app/products/features/[type]/[slug]/page.tsx` — placeholder ("Version list — Task 08").
8. Tests with MSW handlers for `/api/v1/features`.

## Files
- `frontend/src/app/layout.tsx`
- `frontend/src/components/nav/TopNav.tsx`
- `frontend/src/components/nav/Breadcrumbs.tsx`
- `frontend/src/app/products/features/page.tsx`
- `frontend/src/app/products/features/[type]/[slug]/page.tsx`
- `frontend/src/components/features/FeatureCard.tsx`
- `frontend/src/components/features/FeatureCreateModal.tsx`
- `frontend/src/lib/api/features.ts`
- `frontend/src/test/mocks/handlers.ts` (extend)
- `frontend/src/components/features/__tests__/*.test.tsx`

## Tests
- `FeaturesPage.test.tsx`: loading skeleton, populated grid, retry on error.
- `FeatureCreateModal.test.tsx`: form validation (slug pattern), success closes modal and refetches.

## Verify
1. Empty DB → empty state visible.
2. Create "DN Article" via modal → row appears immediately (optimistic or refetch).
3. Click row → lands on placeholder version page.
4. Top nav layout matches reference (visual diff).

## Done When
PR merged with screenshots: empty state, populated grid, create modal.
