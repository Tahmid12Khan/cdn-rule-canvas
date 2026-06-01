import { z } from "zod";

// Mirrors BACKEND CONTRACT §6 exactly. Shared by versions.ts +
// serialize/deserialize. STABLE — do not change shape without bumping the
// contract.

export const Position = z.object({ x: z.number(), y: z.number() });
export type Position = z.infer<typeof Position>;

export const Branch = z.enum(["yes", "no"]);
export type Branch = z.infer<typeof Branch>;

export const MetaTagsOperator = z.enum(["contains", "equals", "exists"]);
export type MetaTagsOperator = z.infer<typeof MetaTagsOperator>;

export const DeviceOperator = z.enum(["equals", "contains"]);
export type DeviceOperator = z.infer<typeof DeviceOperator>;

export const DeviceValue = z.enum(["mobile", "desktop", "tablet"]);
export type DeviceValue = z.infer<typeof DeviceValue>;

export const ArticleUrlOperator = z.enum([
  "contains",
  "matches",
  "starts_with",
  "equals",
]);
export type ArticleUrlOperator = z.infer<typeof ArticleUrlOperator>;

export const ProcessorConfig = z.discriminatedUnion("type", [
  z.object({
    type: z.literal("meta_tags"),
    tag_name: z.string(),
    operator: MetaTagsOperator,
    value: z.string().nullish(),
  }),
  z.object({
    type: z.literal("device_type"),
    operator: DeviceOperator,
    value: DeviceValue,
  }),
  z.object({
    type: z.literal("article_url"),
    operator: ArticleUrlOperator,
    value: z.string(),
  }),
]);
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
