import type { Edge, Node } from "reactflow";

export type CanvasKey = "anonymous" | "registered" | "customer";

// ---- processor config (mirrors BACKEND CONTRACT §6 ProcessorConfig, serde
// tag="type", snake_case) ----
export type MetaTagsOperator = "contains" | "equals" | "exists";
export type DeviceOperator = "equals" | "contains";
export type DeviceValue = "mobile" | "desktop" | "tablet";
export type ArticleUrlOperator =
  | "contains"
  | "matches"
  | "starts_with"
  | "equals";

export type ProcessorConfig =
  | {
      type: "meta_tags";
      tag_name: string;
      operator: MetaTagsOperator;
      value?: string | null;
    }
  | { type: "device_type"; operator: DeviceOperator; value: DeviceValue }
  | { type: "article_url"; operator: ArticleUrlOperator; value: string };

// ---- node data payloads (what lives in RF node.data — serializable) ----
export interface DecisionNodeData {
  processor: ProcessorConfig;
}
// title cached for label; outcomeId is canonical
export interface OutcomeNodeData {
  outcomeId: string;
  title: string;
}
// Post-MVP categories (rendered, but only decision/outcome are creatable in MVP):
export interface SubRuleNodeData {
  label: string;
}
export interface ActionNodeData {
  label: string;
}
// Frontend-only visual entry marker (Phase 1, Task C). Never persisted: the
// backend RuleGraph allows only decision/outcome nodes, and the graph entry is
// DERIVED via computeRootNodeId. The start node is injected on empty/seed and
// stripped on serialize, so it never affects root detection or the wire format.
export interface StartNodeData {
  label: string;
}

// Fixed id of the single injected start node (one per canvas).
export const START_NODE_ID = "start";

export type RFNode =
  | Node<DecisionNodeData, "decisionNode">
  | Node<OutcomeNodeData, "outcomeNode">
  | Node<SubRuleNodeData, "subRuleNode">
  | Node<ActionNodeData, "actionNode">
  | Node<StartNodeData, "startNode">;

// edge.data carries the YES/NO branch for the LabeledEdge renderer
export type Branch = "yes" | "no";
export interface RFEdgeData {
  branch: Branch;
}
export type RFEdge = Edge<RFEdgeData> & { sourceHandle: Branch | null };
