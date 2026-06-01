import { http, HttpResponse } from "msw";

const API_BASE = process.env.NEXT_PUBLIC_API_BASE ?? "http://localhost:8000";

// Base MSW handlers. Domain (LEAF) test files add their own handlers via
// `server.use(...)` per test; these are the shared defaults.
export const handlers = [
  http.get(`${API_BASE}/health`, () =>
    HttpResponse.json({ status: "ok", version: "0.1.0", git_commit: "dev" }),
  ),
];
