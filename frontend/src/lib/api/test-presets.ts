import { z } from "zod";

import { apiGet, apiSend, Page } from "@/lib/api/client";
import { SLUG_RE } from "@/lib/api/features";

// Test-preset API client. Mirrors the backend TestPresetCreate / TestPresetUpdate
// / TestPresetRead DTOs (CONTRACTS.md §3 migration 0011 + §7 routes). The
// `payload` is an opaque JSON object owned by the rule-builder Test panels — the
// backend only validates it is a JSON object within a size cap, so the client
// keeps it permissive (a record of unknown values).

export const TestPresetKind = z.enum(["rule", "url"]);
export type TestPresetKind = z.infer<typeof TestPresetKind>;

// Permissive payload: any JSON object. The panels own the per-field shape; the
// backend only checks it is an object.
export const TestPresetPayload = z.record(z.string(), z.unknown());
export type TestPresetPayload = z.infer<typeof TestPresetPayload>;

export const TestPresetRead = z.object({
  slug: z.string(),
  name: z.string(),
  kind: TestPresetKind,
  // Opaque object owned by the frontend panels. `.catch({})` tolerates a stale
  // backend that returns a non-object payload.
  payload: TestPresetPayload.catch({}),
  created_at: z.string(), // ISO8601 (DateTime<Utc>)
  updated_at: z.string(),
});
export type TestPresetRead = z.infer<typeof TestPresetRead>;

export const TestPresetCreate = z.object({
  slug: z
    .string()
    .min(3, "Slug must be at least 3 characters")
    .max(64, "Slug must be at most 64 characters")
    .regex(SLUG_RE, "Use lowercase kebab-case (e.g. my-test)"),
  name: z
    .string()
    .min(1, "Name is required")
    .max(200, "Name must be at most 200 characters"),
  kind: TestPresetKind,
  payload: TestPresetPayload,
});
export type TestPresetCreate = z.infer<typeof TestPresetCreate>;

// TestPresetUpdate: slug + kind are immutable; only name and payload may change.
export const TestPresetUpdate = z.object({
  name: z.string().min(1).max(200).optional(),
  payload: TestPresetPayload.optional(),
});
export type TestPresetUpdate = z.infer<typeof TestPresetUpdate>;

export interface ListTestPresetsParams {
  page: number;
  page_size: number;
  q?: string;
  kind?: TestPresetKind;
}

export const listTestPresets = (
  p: ListTestPresetsParams = { page: 1, page_size: 20 },
) => {
  const params = new URLSearchParams({
    page: String(p.page),
    page_size: String(p.page_size),
  });
  if (p.q && p.q.trim() !== "") params.set("q", p.q.trim());
  if (p.kind) params.set("kind", p.kind);
  return apiGet(`/api/v1/test-presets?${params.toString()}`, Page(TestPresetRead));
};

export const getTestPreset = (slug: string) =>
  apiGet(`/api/v1/test-presets/${slug}`, TestPresetRead);

export const createTestPreset = (b: TestPresetCreate) =>
  apiSend("POST", `/api/v1/test-presets`, TestPresetRead, b);

export const updateTestPreset = (slug: string, b: TestPresetUpdate) =>
  apiSend("PATCH", `/api/v1/test-presets/${slug}`, TestPresetRead, b);

export const deleteTestPreset = (slug: string) =>
  apiSend("DELETE", `/api/v1/test-presets/${slug}`, z.void());
