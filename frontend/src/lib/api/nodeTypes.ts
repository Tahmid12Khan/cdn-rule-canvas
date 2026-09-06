import { z } from "zod";

import { apiGet } from "@/lib/api/client";

// Node-type manifest (BACKEND CONTRACT §6, GET /api/v1/node-types). The backend
// is the source of truth: it serves backend/config/node_types.json verbatim.
// ALL object keys are snake_case. A new node type is a backend-only JSON entry —
// zero frontend changes — so this schema is intentionally permissive about the
// set of kinds/categories/fields (it validates SHAPE, not specific values).

// `outcome_select` (expression-nodes-spec §2) is a dynamic dropdown of the
// version's outcomes; its options are supplied by the client/validator, NOT the
// manifest.
// `site_select` (sites-host-config-spec §6) is a searchable combobox of the
// configured Sites; it stores the selected site's SLUG. Options are fetched at
// render time (searchSites), NOT supplied by the manifest.
// `component_select` / `component_version_select` (component-editor design §5.4)
// are dynamic dropdowns for apply_component / apply_component_json: the component
// list and the chosen component's version numbers are fetched client-side, NOT
// supplied by the manifest (like outcome_select, they skip option-membership
// validation).
// `product_select` is a searchable combobox of the configured Products (used by
// the `has_product` decision node); it stores the selected product's LABEL.
// Options are fetched at render time (searchProducts), NOT supplied by the
// manifest.
export const NodeFieldControl = z.enum([
  "select",
  "text",
  "number",
  "outcome_select",
  "site_select",
  "component_select",
  "component_version_select",
  "product_select",
]);
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

// `id` is the manifest output-branch identifier (display metadata). Decision
// nodes use "yes"/"no"; expression nodes use a single passthrough "out" branch
// (expression-nodes-spec §2). Mirror the backend `BranchSpec.id: String` — keep
// this a free string so new node types add branches with ZERO frontend change.
export const NodeBranch = z.object({
  id: z.string(),
  label: z.string(),
});
export type NodeBranch = z.infer<typeof NodeBranch>;

// Palette-availability discriminator (spec §1.2). Gates a node type by the
// feature's content kind: "all" (default when omitted) shows everywhere,
// "html"/"json" only for that feature type. Backend serves it verbatim.
export const NodeAppliesTo = z.enum(["all", "html", "json"]);
export type NodeAppliesTo = z.infer<typeof NodeAppliesTo>;

// `node_kind` (expression-nodes-spec §2) drives which RF node type the frontend
// creates on drop and which validation path applies. Defaults to "decision"
// when omitted (backend `#[serde(default)]`).
export const NodeKind = z.enum(["decision", "expression"]);
export type NodeKind = z.infer<typeof NodeKind>;

export const NodeTypeSpec = z.object({
  kind: z.string(),
  label: z.string(),
  category: z.string(),
  summary: z.string(),
  applies_to: NodeAppliesTo.optional(),
  node_kind: NodeKind.optional(),
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
