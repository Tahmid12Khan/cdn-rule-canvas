# Task 03 — Wire Frontend ↔ Backend

## Goal
Connect the Next.js app to the Axum backend through a typed API client. Display backend health status on the landing page so the connection is visibly verifiable.

## Dependencies
Tasks 01, 02.

## Acceptance Criteria
- Backend enables CORS for `http://localhost:3000`.
- Frontend has a typed API client (`src/lib/api/client.ts`) using `fetch` + `zod` (or generated types) with a single `apiBase` env var.
- Landing page shows a "Backend Status" card: green dot + version when reachable, red dot + error message when not.
- TanStack Query is installed and wraps the app with `<QueryClientProvider>`.
- Unit tests cover both states (success/error) using MSW (Mock Service Worker).

## Implementation Steps
1. **Backend:**
   - Add a `tower-http` `CorsLayer` with `.allow_origin(settings.frontend_origin)` mounted on the router.
   - Add `FRONTEND_ORIGIN=http://localhost:3000` to `.env.example`.
2. **Frontend:**
   - Install `@tanstack/react-query`, `zod`, `msw` (dev).
   - Create `src/lib/api/client.ts` with `apiGet<T>(path, schema)` helper.
   - Create `src/lib/api/health.ts` with `HealthSchema` and `fetchHealth()`.
   - Create `src/components/providers/QueryProvider.tsx` and wrap in `src/app/layout.tsx`.
   - Create `src/components/BackendStatusCard.tsx` using `useQuery`.
   - Add MSW handlers in `src/test/mocks/handlers.ts` and setup in `vitest.config.ts`.
   - Add `NEXT_PUBLIC_API_BASE=http://localhost:8000` to `.env.local.example`.
3. Render `<BackendStatusCard />` on `src/app/page.tsx`.

## Files
- `backend/src/lib.rs` (CORS layer)
- `backend/src/config.rs` (frontend_origin)
- `frontend/src/lib/api/client.ts`
- `frontend/src/lib/api/health.ts`
- `frontend/src/components/BackendStatusCard.tsx`
- `frontend/src/components/providers/QueryProvider.tsx`
- `frontend/src/test/mocks/handlers.ts`
- `frontend/src/components/__tests__/BackendStatusCard.test.tsx`

## Tests
- Frontend: `BackendStatusCard.test.tsx` — renders loading → connected (MSW success), then error path (MSW 500).
- Backend: `tests/cors.rs::preflight_options_health_returns_allow_origin` — preflight `OPTIONS /health` returns expected `Access-Control-Allow-Origin`.

## Verify
1. Both servers running: landing page shows green "Connected · v0.1.0".
2. Stop backend: card flips to red "Backend unreachable".
3. `npm run test` and `cargo test` both green.

## Done When
PR merged with two screenshots (connected + disconnected).
