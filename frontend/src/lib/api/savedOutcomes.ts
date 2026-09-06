import { z } from "zod";

import { apiGet, apiSend, Page } from "@/lib/api/client";
import { SLUG_RE } from "@/lib/api/features";

const BASE = "/api/v1/saved-outcomes";

export const SavedOutcomeRead = z.object({
  id: z.string(),
  slug: z.string(),
  name: z.string(),
  component_id: z.string(),
  component_name: z.string(),
  version_number: z.number().nullable(),
  variables: z.record(z.string(), z.string()),
  created_at: z.string(),
  updated_at: z.string(),
});
export type SavedOutcomeRead = z.infer<typeof SavedOutcomeRead>;

export const SavedOutcomeCreate = z.object({
  slug: z.string().min(3).max(120).regex(SLUG_RE, "Use lowercase kebab-case"),
  name: z.string().min(1).max(200),
  component_id: z.string(),
  version_number: z.number().nullable().optional(),
  variables: z.record(z.string(), z.string()).default({}),
});
export type SavedOutcomeCreate = z.infer<typeof SavedOutcomeCreate>;

export const SavedOutcomeUpdate = z.object({
  name: z.string().min(1).max(200).optional(),
  version_number: z.number().nullable().optional(),
  variables: z.record(z.string(), z.string()).optional(),
});
export type SavedOutcomeUpdate = z.infer<typeof SavedOutcomeUpdate>;

export const ResolvedSavedOutcomeRead = z.object({ html_body: z.string() });
export type ResolvedSavedOutcomeRead = z.infer<typeof ResolvedSavedOutcomeRead>;

export interface ListSavedOutcomesParams {
  page: number;
  page_size: number;
  q?: string;
}

export const listSavedOutcomes = (p: ListSavedOutcomesParams = { page: 1, page_size: 20 }) => {
  const params = new URLSearchParams({ page: String(p.page), page_size: String(p.page_size) });
  if (p.q && p.q.trim() !== "") params.set("q", p.q.trim());
  return apiGet(`${BASE}?${params.toString()}`, Page(SavedOutcomeRead));
};

export const getSavedOutcome = (id: string) => apiGet(`${BASE}/${id}`, SavedOutcomeRead);

export const createSavedOutcome = (b: SavedOutcomeCreate) =>
  apiSend("POST", BASE, SavedOutcomeRead, b);

export const updateSavedOutcome = (id: string, b: SavedOutcomeUpdate) =>
  apiSend("PATCH", `${BASE}/${id}`, SavedOutcomeRead, b);

export const deleteSavedOutcome = (id: string) => apiSend("DELETE", `${BASE}/${id}`, z.void());

export const resolveSavedOutcome = (id: string) =>
  apiGet(`${BASE}/${id}/resolve`, ResolvedSavedOutcomeRead);

const SEARCH_PAGE_SIZE = 10;
export const searchSavedOutcomes = (q: string) =>
  listSavedOutcomes({ page: 1, page_size: SEARCH_PAGE_SIZE, q });

export const savedOutcomeKeys = {
  list: (p: ListSavedOutcomesParams) => ["saved-outcomes", "list", p] as const,
  detail: (id: string) => ["saved-outcomes", "detail", id] as const,
};
