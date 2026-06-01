import { z } from "zod";

import { apiGet } from "@/lib/api/client";

// Node-type manifest (BACKEND CONTRACT §6, GET /api/v1/node-types). The backend
// is the source of truth: it serves backend/config/node_types.json verbatim.
// ALL object keys are snake_case. A new node type is a backend-only JSON entry —
// zero frontend changes — so this schema is intentionally permissive about the
// set of kinds/categories/fields (it validates SHAPE, not specific values).

export const NodeFieldControl = z.enum(["select", "text", "number"]);
export type NodeFieldControl = z.infer<typeof NodeFieldControl>;

export const NodeFieldOption = z.object({
  value: z.unknown(),
  label: z.string(),
  symbol: z.string().optional(),
});
export type NodeFieldOption = z.infer<typeof NodeFieldOption>;

export const NodeFieldSpec = z.object({
  name: z.string(),
  label: z.string(),
  control: NodeFieldControl,
  required: z.boolean().optional(),
  required_unless: z
    .object({ field: z.string(), value: z.unknown() })
    .optional(),
  default: z.unknown().optional(),
  placeholder: z.string().optional(),
  options: z.array(NodeFieldOption).optional(),
  required_message: z.string().optional(),
});
export type NodeFieldSpec = z.infer<typeof NodeFieldSpec>;

export const NodeBranch = z.object({
  id: z.enum(["yes", "no"]),
  label: z.string(),
});
export type NodeBranch = z.infer<typeof NodeBranch>;

// Palette-availability discriminator (spec §1.2). Gates a node type by the
// feature's content kind: "all" (default when omitted) shows everywhere,
// "html"/"json" only for that feature type. Backend serves it verbatim.
export const NodeAppliesTo = z.enum(["all", "html", "json"]);
export type NodeAppliesTo = z.infer<typeof NodeAppliesTo>;

export const NodeTypeSpec = z.object({
  kind: z.string(),
  label: z.string(),
  category: z.string(),
  summary: z.string(),
  applies_to: NodeAppliesTo.optional(),
  fields: z.array(NodeFieldSpec),
  output: z.object({ branches: z.array(NodeBranch) }),
});
export type NodeTypeSpec = z.infer<typeof NodeTypeSpec>;

export const NodeCategory = z.object({
  id: z.string(),
  label: z.string(),
  coming_soon: z.boolean().optional(),
});
export type NodeCategory = z.infer<typeof NodeCategory>;

export const NodeDisplayConfig = z.object({ value_max_chars: z.number() });
export type NodeDisplayConfig = z.infer<typeof NodeDisplayConfig>;

export const NodeManifest = z.object({
  categories: z.array(NodeCategory),
  node_types: z.array(NodeTypeSpec),
  display: NodeDisplayConfig.optional(),
});
export type NodeManifest = z.infer<typeof NodeManifest>;

export const getNodeTypes = () =>
  apiGet("/api/v1/node-types", NodeManifest);
