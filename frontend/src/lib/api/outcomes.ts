import { z } from "zod";

import { apiGet, apiSend } from "@/lib/api/client";
import { ComponentRead } from "@/lib/api/components";

// Outcome DTOs — mirror BACKEND CONTRACT §5 (OutcomeRead/Create/Update,
// ReorderItem). OutcomeRead nests its components (ordered by order_index ASC).

export const OutcomeRead = z.object({
  id: z.guid(),
  version_id: z.guid(),
  title: z.string(),
  description: z.string().nullable(),
  is_builtin: z.boolean(),
  order_index: z.number().int(),
  components: z.array(ComponentRead),
  created_at: z.string(),
  updated_at: z.string(),
});
export type OutcomeRead = z.infer<typeof OutcomeRead>;

export const OutcomeCreate = z.object({
  title: z.string().min(1).max(100),
  description: z.string().max(500).optional(),
});
export type OutcomeCreate = z.infer<typeof OutcomeCreate>;

export const OutcomeUpdate = z.object({
  title: z.string().min(1).max(100).optional(),
  description: z.string().max(500).optional(),
  order_index: z.number().int().optional(),
});
export type OutcomeUpdate = z.infer<typeof OutcomeUpdate>;

export const ReorderItem = z.object({
  id: z.guid(),
  order_index: z.number().int(),
});
export type ReorderItem = z.infer<typeof ReorderItem>;

export const listOutcomes = (vid: string): Promise<OutcomeRead[]> =>
  apiGet(`/api/v1/versions/${vid}/outcomes`, z.array(OutcomeRead));

export const getOutcome = (oid: string): Promise<OutcomeRead> =>
  apiGet(`/api/v1/outcomes/${oid}`, OutcomeRead);

export const createOutcome = (
  vid: string,
  body: OutcomeCreate,
): Promise<OutcomeRead> =>
  apiSend("POST", `/api/v1/versions/${vid}/outcomes`, OutcomeRead, body);

export const updateOutcome = (
  oid: string,
  body: OutcomeUpdate,
): Promise<OutcomeRead> =>
  apiSend("PATCH", `/api/v1/outcomes/${oid}`, OutcomeRead, body);

export const deleteOutcome = (oid: string): Promise<void> =>
  apiSend("DELETE", `/api/v1/outcomes/${oid}`, z.void());

export const cloneOutcome = (oid: string): Promise<OutcomeRead> =>
  apiSend("POST", `/api/v1/outcomes/${oid}/clone`, OutcomeRead);

export const reorderComponents = (
  oid: string,
  items: ReorderItem[],
): Promise<ComponentRead[]> =>
  apiSend(
    "POST",
    `/api/v1/outcomes/${oid}/reorder`,
    z.array(ComponentRead),
    items,
  );
