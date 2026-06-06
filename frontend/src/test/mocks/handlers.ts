import { http, HttpResponse } from "msw";

import { NODE_TYPES_FIXTURE } from "@/test/fixtures/nodeTypes";

const API_BASE = process.env.NEXT_PUBLIC_API_BASE ?? "http://localhost:8000";

// Base MSW handlers. Domain (LEAF) test files add their own handlers via
// `server.use(...)` per test; these are the shared defaults.
export const handlers = [
  http.get(`${API_BASE}/health`, () =>
    HttpResponse.json({ status: "ok", version: "0.1.0", git_commit: "dev" }),
  ),
  // Node-type manifest (backend-driven node metadata). Served from a fixture so
  // tests never depend on a live backend.
  http.get(`${API_BASE}/api/v1/node-types`, () =>
    HttpResponse.json(NODE_TYPES_FIXTURE),
  ),
  // Test-preset library default: empty. The TestPresetBar (embedded in both Test
  // panels) lists presets on mount; tests that care override this per-test.
  http.get(`${API_BASE}/api/v1/test-presets`, () =>
    HttpResponse.json({ items: [], page: 1, page_size: 100, total: 0 }),
  ),
];
