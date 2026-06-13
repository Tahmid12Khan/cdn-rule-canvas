import { z } from "zod";

import { SLUG_RE } from "@/lib/api/features";

// Zod mirror of the backend Component Template DTOs (component-editor design
// §3.2 / §5.2). Components are a GLOBAL library of independently-versioned,
// mustache HTML templates that hold ONLY a result payload; rules supply the
// control flow + variable values. The rendered result is produced at request
// time by the proxy. These schemas validate every API response (FRONTEND
// CONTRACT: zod-validate API responses).

// ── ComponentVariable ───────────────────────────────────────────────────────
// The mustache key (`{{title}}` → name "title") plus author metadata (title +
// description) that drives the rule-side population UI.
export const ComponentVariable = z.object({
  name: z
    .string()
    .min(1, "Variable name is required")
    .max(64, "Variable name must be at most 64 characters"),
  title: z
    .string()
    .min(1, "Title is required")
    .max(100, "Title must be at most 100 characters"),
  description: z
    .string()
    .max(500, "Description must be at most 500 characters")
    .optional(),
});
export type ComponentVariable = z.infer<typeof ComponentVariable>;

// ── default_mode ─────────────────────────────────────────────────────────────
// How a component's movable "default" pointer resolves: `latest` auto-advances
// to the highest version; `pinned` follows `default_version_number`.
export const DefaultMode = z.enum(["latest", "pinned"]);
export type DefaultMode = z.infer<typeof DefaultMode>;

// ── Version summary (rows inside ComponentTemplateRead.versions) ─────────────
export const ComponentTemplateVersionSummary = z.object({
  id: z.string(),
  version_number: z.number().int(),
  description: z.string().nullish(),
  is_default: z.boolean(),
  created_at: z.string(), // ISO8601
  updated_at: z.string(),
});
export type ComponentTemplateVersionSummary = z.infer<
  typeof ComponentTemplateVersionSummary
>;

// ── Full version read (includes html_body + variables) ──────────────────────
export const ComponentTemplateVersionRead = z.object({
  id: z.string(),
  version_number: z.number().int(),
  description: z.string().nullish(),
  html_body: z.string(),
  variables: z.array(ComponentVariable),
  is_default: z.boolean(),
  created_at: z.string(),
  updated_at: z.string(),
});
export type ComponentTemplateVersionRead = z.infer<
  typeof ComponentTemplateVersionRead
>;

// ── Component read (detail) ──────────────────────────────────────────────────
export const ComponentTemplateRead = z.object({
  id: z.string(),
  slug: z.string(),
  name: z.string(),
  description: z.string().nullish(),
  default_mode: DefaultMode,
  default_version_number: z.number().int().nullish(),
  latest_version_number: z.number().int(),
  versions: z.array(ComponentTemplateVersionSummary),
  created_at: z.string(),
  updated_at: z.string(),
});
export type ComponentTemplateRead = z.infer<typeof ComponentTemplateRead>;

// ── Component summary (list rows) ────────────────────────────────────────────
export const ComponentTemplateSummary = z.object({
  id: z.string(),
  slug: z.string(),
  name: z.string(),
  description: z.string().nullish(),
  default_version_number: z.number().int().nullish(),
  latest_version_number: z.number().int(),
  updated_at: z.string(),
});
export type ComponentTemplateSummary = z.infer<typeof ComponentTemplateSummary>;

// ── Resolved (proxy-facing) ──────────────────────────────────────────────────
// `version_number` is the version that was actually resolved (after the
// default/fallback logic), the rendered `html_body`, and the declared variables.
export const ResolvedComponentRead = z.object({
  version_number: z.number().int(),
  html_body: z.string(),
  variables: z.array(ComponentVariable),
});
export type ResolvedComponentRead = z.infer<typeof ResolvedComponentRead>;

// ── Write DTOs ────────────────────────────────────────────────────────────────
export const ComponentTemplateCreate = z.object({
  slug: z
    .string()
    .min(3, "Slug must be at least 3 characters")
    .max(120, "Slug must be at most 120 characters")
    .regex(SLUG_RE, "Use lowercase kebab-case (e.g. paywall-cta)"),
  name: z
    .string()
    .min(1, "Name is required")
    .max(200, "Name must be at most 200 characters"),
  description: z
    .string()
    .max(2000, "Description must be at most 2000 characters")
    .optional(),
  html_body: z.string().optional(),
  variables: z.array(ComponentVariable).optional(),
});
export type ComponentTemplateCreate = z.infer<typeof ComponentTemplateCreate>;

// Update manages the default pointer. `default_version_number` is required when
// switching to `pinned` (enforced by the backend; the form also guards it).
export const ComponentTemplateUpdate = z.object({
  name: z.string().min(1).max(200).optional(),
  description: z.string().max(2000).optional(),
  default_mode: DefaultMode.optional(),
  default_version_number: z.number().int().positive().optional(),
});
export type ComponentTemplateUpdate = z.infer<typeof ComponentTemplateUpdate>;

export const VersionCreate = z.object({
  description: z.string().max(2000).optional(),
  html_body: z.string().optional(),
  variables: z.array(ComponentVariable).optional(),
  make_default: z.boolean().optional(),
});
export type VersionCreate = z.infer<typeof VersionCreate>;

export const VersionUpdate = z.object({
  description: z.string().max(2000).optional(),
  html_body: z.string().optional(),
  variables: z.array(ComponentVariable).optional(),
});
export type VersionUpdate = z.infer<typeof VersionUpdate>;
