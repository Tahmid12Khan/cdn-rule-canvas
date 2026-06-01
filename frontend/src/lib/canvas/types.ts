import type { Edge, Node } from "reactflow";

export type CanvasKey = "anonymous" | "registered" | "customer";

// ---- processor config (mirrors BACKEND CONTRACT §6 ProcessorConfig) ----
// Generic, manifest-validated shape: one snake_case `type` discriminator (the
// canonical node-type kind) plus an open map of snake_case config fields. The
// node-type manifest (GET /api/v1/node-types) is the runtime contract — there
// is no per-type union, so a new node type needs ZERO frontend change. Round-
// trips the SAME wire JSON the backend/proxy exchange. REUSED for the expression
// node `action` (same `{ type, …fields }` shape — expression-nodes-spec §1).
export interface ProcessorConfig {
  type: string;
  [field: string]: unknown;
}

// ---- node data payloads (what lives in RF node.data — serializable) ----

// Entry marker. Exactly one per non-empty canvas; PERSISTED on the wire as a
// `start` node (expression-nodes-spec §1). No incoming edges; one outgoing edge.
export interface StartNodeData {
  label: string;
}
// Branches yes/no via a processor. UNCHANGED.
export interface DecisionNodeData {
  processor: ProcessorConfig;
}
// Performs one body action and passes through. `action` is the generic
// ProcessorConfig (`{ type, …fields }`). For `apply_outcome` the action carries
// `outcome_id`; `outcomeTitle` is a denormalized display cache (NOT persisted),
// re-resolved from the outcomes query on deserialize.
export interface ExpressionNodeData {
  action: ProcessorConfig;
  outcomeTitle?: string;
}
// Terminal. Stops the flow. Zero outgoing edges. ≥1 per non-empty canvas.
export interface EndNodeData {
  label: string;
}

// Fixed ids of the auto-injected start + end nodes (one each per canvas).
export const START_NODE_ID = "start";
export const END_NODE_ID = "end";

export type RFNode =
  | Node<StartNodeData, "startNode">
  | Node<DecisionNodeData, "decisionNode">
  | Node<ExpressionNodeData, "expressionNode">
  | Node<EndNodeData, "endNode">;

// edge.data carries the YES/NO branch for the LabeledEdge renderer
export type Branch = "yes" | "no";
export interface RFEdgeData {
  branch: Branch;
}
export type RFEdge = Edge<RFEdgeData> & { sourceHandle: Branch | null };
