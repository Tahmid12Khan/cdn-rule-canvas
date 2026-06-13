import { z } from "zod";

import { apiSend } from "@/lib/api/client";
import { Placement } from "@/lib/api/enums";

// Component DTOs — mirror BACKEND CONTRACT §5 (ComponentRead/Create/Update +
// the typed ComponentConfig discriminated union). On READ, `config` is an
// opaque serde_json::Value (`z.unknown()`); the editor re-parses it with
// `ComponentConfig` before populating a config form. On WRITE, `config` MUST be
// a valid `ComponentConfig` whose `type` matches the row `type` column.

export const HtmlPlacementMode = z.enum([
  "replace",
  "append",
  "prepend",
  "before",
  "after",
]);
export type HtmlPlacementMode = z.infer<typeof HtmlPlacementMode>;

// `target_path` is a SIMPLE path (dot + [index], e.g. `$.user.premium`) — the
// proxy walks the parsed path. Mirrors BACKEND §2.3 ComponentConfig JSON arms.
const JsonTargetPath = z.string().min(1).max(500);

export const ComponentConfig = z.discriminatedUnion("type", [
  z.object({
    type: z.literal("html_injection"),
    target_selector: z.string().min(1),
    placement_mode: HtmlPlacementMode,
    html_body: z.string(),
    theme: z.string().nullish(),
  }),
  z.object({
    type: z.literal("content_truncation"),
    target_selector: z.string().min(1),
    word_count: z.number().int().min(1).max(10000),
    fade_out: z.boolean().default(false),
  }),
  z.object({
    type: z.literal("json_remove"),
    target_path: JsonTargetPath,
  }),
  z.object({
    type: z.literal("json_set"),
    target_path: JsonTargetPath,
    value: z.unknown(),
  }),
  z.object({
    type: z.literal("json_replace"),
    target_path: JsonTargetPath,
    value: z.unknown(),
  }),
  // component_ref / component_ref_json reference a GLOBAL library component
  // (componentTemplates.ts) by id + version; the proxy resolves + mustache-
  // renders it at request time. Field names mirror the backend exactly.
  z.object({
    type: z.literal("component_ref"),
    component_id: z.guid(),
    version: z.union([z.literal("default"), z.number().int().positive()]),
    variables: z.record(z.string(), z.string()),
    target_selector: z.string().min(1),
    placement_mode: HtmlPlacementMode,
  }),
  z.object({
    type: z.literal("component_ref_json"),
    component_id: z.guid(),
    version: z.union([z.literal("default"), z.number().int().positive()]),
    variables: z.record(z.string(), z.string()),
    target_path: JsonTargetPath,
  }),
]);
export type ComponentConfig = z.infer<typeof ComponentConfig>;

export const ComponentType = z.enum([
  "html_injection",
  "content_truncation",
  "json_remove",
  "json_set",
  "json_replace",
  "component_ref",
  "component_ref_json",
]);
export type ComponentType = z.infer<typeof ComponentType>;

export const ComponentRead = z.object({
  id: z.guid(),
  outcome_id: z.guid(),
  slug: z.string(),
  type: z.string(),
  config: z.unknown(), // serde_json::Value on read
  placement: Placement,
  order_index: z.number().int(),
  created_at: z.string(),
  updated_at: z.string(),
});
export type ComponentRead = z.infer<typeof ComponentRead>;

export const ComponentCreate = z.object({
  slug: z.string().min(1).max(120),
  type: z.string(),
  config: ComponentConfig,
  placement: Placement,
  order_index: z.number().int().optional(),
});
export type ComponentCreate = z.infer<typeof ComponentCreate>;

export const ComponentUpdate = z.object({
  slug: z.string().optional(),
  type: z.string().optional(),
  config: ComponentConfig.optional(),
  placement: Placement.optional(),
  order_index: z.number().int().optional(),
});
export type ComponentUpdate = z.infer<typeof ComponentUpdate>;

export const addComponent = (
  oid: string,
  body: ComponentCreate,
): Promise<ComponentRead> =>
  apiSend("POST", `/api/v1/outcomes/${oid}/components`, ComponentRead, body);

export const updateComponent = (
  cid: string,
  body: ComponentUpdate,
): Promise<ComponentRead> =>
  apiSend("PATCH", `/api/v1/components/${cid}`, ComponentRead, body);

export const deleteComponent = (cid: string): Promise<void> =>
  apiSend("DELETE", `/api/v1/components/${cid}`, z.void());
