import type { Edge, Node } from "@xyflow/react";

export type CanvasKey = "anonymous" | "registered" | "customer";

// ---- processor config (mirrors BACKEND CONTRACT §6 ProcessorConfig) ----
// Generic, manifest-validated shape: one snake_case `type` discriminator (the
// canonical node-type kind) plus an open map of snake_case config fields. The
// node-type manifest (GET /api/v1/node-types) is the runtime contract — there
// is no per-type union, so a new node type needs ZERO frontend change. Round-
// trips the SAME wire JSON the backend/proxy exchange. REUSED for the expression
// node `action` (same `{ type, …fields }` shape — expression-nodes-spec §1).
export type ProcessorConfig = {
  type: string;
  [field: string]: unknown;
};

// ---- node data payloads (what lives in RF node.data — serializable) ----
// NOTE: these MUST be `type` aliases, not `interface`s — React Flow v12's
// `Node<Data>` constrains `Data extends Record<string, unknown>`, and only type
// aliases of object literals satisfy that (interfaces lack an implicit index
// signature).

// Entry marker. Exactly one per non-empty canvas; PERSISTED on the wire as a
// `start` node (expression-nodes-spec §1). No incoming edges; one outgoing edge.
export type StartNodeData = {
  label: string;
};
// Branches yes/no via a processor. UNCHANGED.
export type DecisionNodeData = {
  processor: ProcessorConfig;
};
// Performs one body action and passes through. `action` is the generic
// ProcessorConfig (`{ type, …fields }`). For `apply_outcome` the action carries
// `outcome_id`; `outcomeTitle` is a denormalized display cache (NOT persisted),
// re-resolved from the outcomes query on deserialize. For `apply_component` /
// `apply_component_json` the action carries `component_id` + `variables`;
// `componentName` is the equivalent denormalized display cache (NOT persisted),
// re-resolved from the component-templates query on deserialize. `custom_label`
// is an optional snake_case display name persisted on the wire (spec §v2.3);
// absent / "" = no custom name.
export type ExpressionNodeData = {
  action: ProcessorConfig;
  outcomeTitle?: string;
  componentName?: string;
  custom_label?: string;
};
// Terminal. Stops the flow. Zero outgoing edges. ≥1 per non-empty canvas.
export type EndNodeData = {
  label: string;
};

// Fixed ids of the auto-injected start + end nodes (one each per canvas).
export const START_NODE_ID = "start";
export const END_NODE_ID = "end";

// Typed React Flow nodes (v12 NodeProps takes the Node type, not the data type).
export type RFStartNode = Node<StartNodeData, "startNode">;
export type RFDecisionNode = Node<DecisionNodeData, "decisionNode">;
export type RFExpressionNode = Node<ExpressionNodeData, "expressionNode">;
export type RFEndNode = Node<EndNodeData, "endNode">;

export type RFNode = RFStartNode | RFDecisionNode | RFExpressionNode | RFEndNode;

// edge.data carries the YES/NO branch for the LabeledEdge renderer + serialize.
export type Branch = "yes" | "no";
export type RFEdgeData = {
  branch: Branch;
};
// `sourceHandle` is the RENDER handle id and MUST match a real source handle on
// the node: "yes"/"no" for a decision, "out" for start/expression (React Flow
// v12 drops an edge whose sourceHandle doesn't exist — error #008). The canonical
// wire branch lives in `data.branch`, NOT here.
export type RFEdge = Edge<RFEdgeData> & { sourceHandle: string | null };
