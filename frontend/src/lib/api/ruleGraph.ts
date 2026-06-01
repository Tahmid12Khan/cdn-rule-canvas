import { z } from "zod";

// Mirrors BACKEND CONTRACT §6 exactly. Shared by versions.ts +
// serialize/deserialize. STABLE — do not change shape without bumping the
// contract.

export const Position = z.object({ x: z.number(), y: z.number() });
export type Position = z.infer<typeof Position>;

export const Branch = z.enum(["yes", "no"]);
export type Branch = z.infer<typeof Branch>;

// Generic, manifest-validated processor config (BACKEND CONTRACT §6): one
// snake_case `type` discriminator plus an OPEN map of snake_case config fields.
// `.passthrough()` preserves any extra fields on round-trip (forward-compatible);
// the node-type manifest is the runtime contract, not a fixed zod union.
export const ProcessorConfig = z
  .object({ type: z.string() })
  .passthrough();
export type ProcessorConfig = z.infer<typeof ProcessorConfig>;

// Node taxonomy (expression-nodes-spec §1): real persisted Start / Decision /
// Expression / End. Outcome is REMOVED — apply_outcome now lives on an
// Expression node's `action` (kept alive for `rre.outcomes`).
export const GraphNode = z.discriminatedUnion("kind", [
  z.object({
    kind: z.literal("start"),
    id: z.string(),
    position: Position,
  }),
  z.object({
    kind: z.literal("decision"),
    id: z.string(),
    processor: ProcessorConfig,
    position: Position,
  }),
  z.object({
    kind: z.literal("expression"),
    id: z.string(),
    action: ProcessorConfig,
    position: Position,
  }),
  z.object({
    kind: z.literal("end"),
    id: z.string(),
    position: Position,
  }),
]);
export type GraphNode = z.infer<typeof GraphNode>;

export const Edge = z.object({
  id: z.string(),
  source_node_id: z.string(),
  target_node_id: z.string(),
  branch: Branch,
});
export type Edge = z.infer<typeof Edge>;

export const CanvasGraph = z.object({
  nodes: z.array(GraphNode),
  edges: z.array(Edge),
  root_node_id: z.string().nullable(),
});
export type CanvasGraph = z.infer<typeof CanvasGraph>;

export const RuleGraph = z.object({
  anonymous: CanvasGraph,
  registered: CanvasGraph,
  customer: CanvasGraph,
});
export type RuleGraph = z.infer<typeof RuleGraph>;

// Version-level applicability gate (spec §2.2 / req 8). Both nullable: empty /
// null = "always apply when the response content-type matches the feature".
// `html_selector` = CSS selector that must match ≥1 element; `json_selector` =
// JSONPath that must match ≥1 node. Mirrors the backend `Applicability` DTO.
export const Applicability = z.object({
  html_selector: z.string().nullish(),
  json_selector: z.string().nullish(),
});
export type Applicability = z.infer<typeof Applicability>;
