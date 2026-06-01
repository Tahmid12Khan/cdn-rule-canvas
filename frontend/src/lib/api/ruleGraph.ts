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

export const GraphNode = z.discriminatedUnion("kind", [
  z.object({
    kind: z.literal("decision"),
    id: z.string(),
    processor: ProcessorConfig,
    position: Position,
  }),
  z.object({
    kind: z.literal("outcome"),
    id: z.string(),
    outcome_id: z.string().uuid(),
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
