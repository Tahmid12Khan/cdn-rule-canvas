# Task 01 — Frontend Scaffold

> **Note:** Build fresh React components with TailwindCSS per the design sections; `playground/static/flow.html` is a REFERENCE ONLY for React Flow usage — do NOT copy or embed that monolithic HTML file.

## Goal
Stand up a Next.js (latest stable, App Router) + TypeScript (latest stable) + Tailwind project on the active Node.js LTS, with linting, formatting, and a unit-test harness. Deliver a styled landing page that visibly confirms the toolchain works.

## Dependencies
None.

## Acceptance Criteria
- Node.js pinned to the active LTS (latest LTS major) via `frontend/.nvmrc` and `package.json` `engines.node`.
- Next.js and TypeScript on their latest stable releases (whatever `create-next-app@latest` installs at scaffold time).
- `cd frontend && npm run dev` serves the landing page at `http://localhost:3000`.
- Landing page renders **"Response Rule Engine"** heading, a teal/green brand accent strip, and a subtitle. Visual baseline established for §8.3 color palette.
- `npm run lint`, `npm run typecheck`, `npm run test`, `npm run build` all pass with zero warnings.
- One Vitest unit test exists and passes (`<BrandHeader />` renders the product name).

## Implementation Steps
1. Use the active Node.js LTS: set `frontend/.nvmrc` to `lts/*` (or the current LTS major), `nvm use`, and add `"engines": { "node": ">=<lts-major>" }` to `package.json`.
2. `npx create-next-app@latest frontend --typescript --tailwind --app --eslint --src-dir --import-alias "@/*"` — pulls the latest stable Next.js + TypeScript.
3. Add devDeps: `vitest`, `@testing-library/react`, `@testing-library/jest-dom`, `@vitejs/plugin-react`, `jsdom`, `prettier`, `prettier-plugin-tailwindcss`.
4. Configure `vitest.config.ts` with `jsdom` env and a `src/test/setup.ts` that imports `@testing-library/jest-dom`.
5. Add scripts to `package.json`: `dev`, `build`, `start`, `lint`, `typecheck` (`tsc --noEmit`), `test`, `test:watch`, `format`.
6. Create design tokens in `tailwind.config.ts`: brand teal (`#0ea5a4` family), action blue (`#2563eb`), status colors (live-green, staging-amber, prev-gray).
7. Create `src/components/BrandHeader.tsx` and use it on `src/app/page.tsx`.
8. Add `src/components/__tests__/BrandHeader.test.tsx`.
9. Commit `.nvmrc`, `.editorconfig`, `.prettierrc`, `.gitignore` additions.

## Files
- `frontend/src/app/layout.tsx`
- `frontend/src/app/page.tsx`
- `frontend/src/components/BrandHeader.tsx`
- `frontend/src/components/__tests__/BrandHeader.test.tsx`
- `frontend/tailwind.config.ts`
- `frontend/vitest.config.ts`
- `frontend/src/test/setup.ts`
- `frontend/.nvmrc`

## Tests
- `BrandHeader.test.tsx`: asserts heading text and brand accent class.

## Verify
1. Open `http://localhost:3000` → see styled landing page.
2. Run `npm run test` → 1 test passing.
3. Run `npm run build` → clean build, no type errors.

## Done When
PR merged to `main` with screenshot of landing page attached.
