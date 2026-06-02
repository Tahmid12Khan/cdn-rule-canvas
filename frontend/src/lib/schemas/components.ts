import { z } from "zod";

import { Placement } from "@/lib/api/enums";

// Component configuration zod schemas (FRONTEND CONTRACT §5.8, Task 16).
//
// These mirror the BACKEND `ComponentConfig` (serde tag = "type", snake_case)
// and the WRITE-side `ComponentCreate`/`ComponentUpdate` shapes. They are the
// single source of truth used by the component-config modal forms to validate
// user input before firing add/update mutations.
//
// NOTE: on READ, `components.ts` exposes `config` as `z.unknown()` (the backend
// serializes it as `serde_json::Value`). The editor re-parses that raw value
// with these discriminated schemas (keyed on `type`) before populating a form.

export const HtmlPlacementMode = z.enum([
  "replace",
  "append",
  "prepend",
  "before",
  "after",
]);
export type HtmlPlacementMode = z.infer<typeof HtmlPlacementMode>;

// ---- HTML Injection -------------------------------------------------------

export const HtmlInjectionConfig = z.object({
  type: z.literal("html_injection"),
  target_selector: z
    .string()
    .min(1, "Enter a CSS selector (e.g. .article-body) for where to inject"),
  placement_mode: HtmlPlacementMode,
  html_body: z.string().min(1, "Add the HTML to inject — this can't be empty"),
  theme: z.string().nullish(),
});
export type HtmlInjectionConfig = z.infer<typeof HtmlInjectionConfig>;

// ---- Content Truncation ---------------------------------------------------

export const ContentTruncationConfig = z.object({
  type: z.literal("content_truncation"),
  target_selector: z
    .string()
    .min(1, "Enter a CSS selector (e.g. .article-body) for the content to truncate"),
  word_count: z
    .number({ error: "Enter a number for the word count" })
    .int("Word count must be a whole number (no decimals)")
    .min(1, "Word count must be at least 1")
    .max(10000, "Word count can't exceed 10000 — pick a smaller value"),
  fade_out: z.boolean().default(false),
});
export type ContentTruncationConfig = z.infer<typeof ContentTruncationConfig>;

// ---- JSON mutation (full-body) — mirrors BACKEND §2.3 ----------------------
//
// `target_path` is a SIMPLE path (dot + [index], e.g. `$.user.premium`,
// `$.items[0].price`) — NOT a filter expression (those are read-only / for the
// json_expression node + applicability). The proxy walks the parsed path.

const TargetPath = z
  .string()
  .min(1, "Enter a JSON path (e.g. $.user.premium)")
  .max(500, "JSON path is too long — keep it under 500 characters");

// type = "json_remove" — delete the value(s) at target_path.
export const JsonRemoveConfig = z.object({
  type: z.literal("json_remove"),
  target_path: TargetPath,
});
export type JsonRemoveConfig = z.infer<typeof JsonRemoveConfig>;

// type = "json_set" — upsert (create or overwrite) the value at target_path.
export const JsonSetConfig = z.object({
  type: z.literal("json_set"),
  target_path: TargetPath,
  value: z.unknown(),
});
export type JsonSetConfig = z.infer<typeof JsonSetConfig>;

// type = "json_replace" — overwrite ONLY if target_path already exists.
export const JsonReplaceConfig = z.object({
  type: z.literal("json_replace"),
  target_path: TargetPath,
  value: z.unknown(),
});
export type JsonReplaceConfig = z.infer<typeof JsonReplaceConfig>;

// ---- Discriminated union (mirrors BACKEND §5 ComponentConfig) --------------

export const ComponentConfig = z.discriminatedUnion("type", [
  HtmlInjectionConfig,
  ContentTruncationConfig,
  JsonRemoveConfig,
  JsonSetConfig,
  JsonReplaceConfig,
]);
export type ComponentConfig = z.infer<typeof ComponentConfig>;

// The creatable component types. HTML features use the first two; JSON features
// use the json_* trio (the UI picks the tab set by feature type).
export const ComponentType = z.enum([
  "html_injection",
  "content_truncation",
  "json_remove",
  "json_set",
  "json_replace",
]);
export type ComponentType = z.infer<typeof ComponentType>;

// ---- Write payloads (mirror BACKEND §5 ComponentCreate / ComponentUpdate) --

export const ComponentCreate = z.object({
  slug: z
    .string()
    .min(1, "Give this component a slug so you can reference it")
    .max(120, "Slug is too long — keep it under 120 characters"),
  type: z.string(),
  config: ComponentConfig,
  placement: Placement,
  order_index: z.number().int().optional(),
});
export type ComponentCreate = z.infer<typeof ComponentCreate>;

export const ComponentUpdate = z.object({
  slug: z.string().min(1).max(120).optional(),
  type: z.string().optional(),
  config: ComponentConfig.optional(),
  placement: Placement.optional(),
  order_index: z.number().int().optional(),
});
export type ComponentUpdate = z.infer<typeof ComponentUpdate>;

// Sensible empty defaults for a freshly-added component of each type.
// Overloaded so callers get the narrowed config type back.
export function defaultConfigFor(type: "html_injection"): HtmlInjectionConfig;
export function defaultConfigFor(
  type: "content_truncation",
): ContentTruncationConfig;
export function defaultConfigFor(type: "json_remove"): JsonRemoveConfig;
export function defaultConfigFor(type: "json_set"): JsonSetConfig;
export function defaultConfigFor(type: "json_replace"): JsonReplaceConfig;
export function defaultConfigFor(type: ComponentType): ComponentConfig;
export function defaultConfigFor(type: ComponentType): ComponentConfig {
  switch (type) {
    case "html_injection":
      return {
        type: "html_injection",
        target_selector: "",
        placement_mode: "append",
        html_body: "",
        theme: null,
      };
    case "content_truncation":
      return {
        type: "content_truncation",
        target_selector: "",
        word_count: 100,
        fade_out: false,
      };
    case "json_remove":
      return {
        type: "json_remove",
        target_path: "",
      };
    case "json_set":
      return {
        type: "json_set",
        target_path: "",
        value: null,
      };
    case "json_replace":
      return {
        type: "json_replace",
        target_path: "",
        value: null,
      };
  }
}
