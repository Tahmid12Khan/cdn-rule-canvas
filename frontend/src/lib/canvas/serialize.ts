// React Flow graph <-> backend RuleGraph (BACKEND CONTRACT §6 ⇄
// expression-nodes-spec §1/§3). Pure functions; no React.
import type { CanvasGraph, RuleGraph } from "@/lib/api/ruleGraph";
import { type Branch, type CanvasKey, type RFEdge, type RFNode } from "@/lib/canvas/types";

export interface CanvasWorkingState {
  nodes: RFNode[];
  edges: RFEdge[];
  rootNodeId: string | null;
}

// RF graph (one canvas) -> backend CanvasGraph. Start / Decision / Expression /
// End are ALL persisted on the wire (the start node is no longer stripped —
// expression-nodes-spec §1).
export function serializeCanvas(
  nodes: RFNode[],
  edges: RFEdge[],
  rootNodeId: string | null,
): CanvasGraph {
  return {
    root_node_id: rootNodeId,
    nodes: nodes
      .map((n): CanvasGraph["nodes"][number] | null => {
        const position = { x: n.position.x, y: n.position.y };
        if (n.type === "startNode") {
          return { kind: "start", id: n.id, position };
        }
        if (n.type === "decisionNode") {
          return {
            kind: "decision",
            id: n.id,
            processor: n.data.processor,
            position,
          };
        }
        if (n.type === "expressionNode") {
          // Omit custom_label when empty/absent so the wire mirrors the
          // backend's skip_serializing_if = Option::is_none (spec §v2.3).
          const customLabel = n.data.custom_label?.trim();
          return {
            kind: "expression",
            id: n.id,
            action: n.data.action,
            ...(customLabel ? { custom_label: customLabel } : {}),
            position,
          };
        }
        if (n.type === "endNode") {
          return { kind: "end", id: n.id, position };
        }
        return null;
      })
      .filter((n): n is CanvasGraph["nodes"][number] => n !== null),
    edges: edges.map((e) => ({
      id: e.id,
      source_node_id: e.source,
      target_node_id: e.target,
      // data.branch is the canonical wire value (yes/no). sourceHandle is a
      // render-only handle id ("out" for start/expression), so it must NOT feed
      // the wire branch. Decision sourceHandle ("yes"/"no") is used as a fallback.
      branch: (e.data?.branch ??
        (e.sourceHandle === "no" ? "no" : "yes")) as Branch,
    })),
  };
}

// store.canvases -> RuleGraph { anonymous, registered, customer }.
export function serializeRuleGraph(
  canvases: Record<CanvasKey, CanvasWorkingState>,
): RuleGraph {
  return {
    anonymous: serializeCanvas(
      canvases.anonymous.nodes,
      canvases.anonymous.edges,
      canvases.anonymous.rootNodeId,
    ),
    registered: serializeCanvas(
      canvases.registered.nodes,
      canvases.registered.edges,
      canvases.registered.rootNodeId,
    ),
    customer: serializeCanvas(
      canvases.customer.nodes,
      canvases.customer.edges,
      canvases.customer.rootNodeId,
    ),
  };
}

// Stable hash of a RuleGraph for dirty-tracking. JSON.stringify with sorted
// keys keeps the hash deterministic regardless of property insertion order.
export function hashRuleGraph(rg: RuleGraph): string {
  return stableStringify(rg);
}

function stableStringify(value: unknown): string {
  if (value === null || typeof value !== "object") {
    return JSON.stringify(value);
  }
  if (Array.isArray(value)) {
    return `[${value.map(stableStringify).join(",")}]`;
  }
  const obj = value as Record<string, unknown>;
  const keys = Object.keys(obj).sort();
  return `{${keys
    .map((k) => `${JSON.stringify(k)}:${stableStringify(obj[k])}`)
    .join(",")}}`;
}
