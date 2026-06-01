// Backend RuleGraph -> React Flow graph (BACKEND CONTRACT §6 ⇄
// expression-nodes-spec §1). Inverse of serialize.ts.
//
// Invariant (unit test): deserialize(serialize(g)) === g for all three
// canvases. The apply_outcome action's outcome title is NOT serialized (server
// state) — it is re-resolved here from the outcomes query via outcomeTitleById
// and cached on the expression node's data.
import type { CanvasGraph, RuleGraph } from "@/lib/api/ruleGraph";
import type { CanvasKey, RFEdge, RFNode } from "@/lib/canvas/types";
import type { CanvasWorkingState } from "@/lib/canvas/serialize";

export function deserializeCanvas(
  g: CanvasGraph,
  outcomeTitleById: (id: string) => string,
): CanvasWorkingState {
  const nodes = g.nodes.map<RFNode>((n) => {
    const position = { x: n.position.x, y: n.position.y };
    if (n.kind === "start") {
      return {
        id: n.id,
        type: "startNode",
        position,
        data: { label: "Start" },
        deletable: false,
      };
    }
    if (n.kind === "decision") {
      return {
        id: n.id,
        type: "decisionNode",
        position,
        data: { processor: n.processor },
      };
    }
    if (n.kind === "expression") {
      // apply_outcome carries outcome_id — re-resolve a display title for it.
      const outcomeId = n.action.outcome_id;
      const outcomeTitle =
        typeof outcomeId === "string" ? outcomeTitleById(outcomeId) : undefined;
      return {
        id: n.id,
        type: "expressionNode",
        position,
        data: { action: n.action, outcomeTitle },
      };
    }
    return {
      id: n.id,
      type: "endNode",
      position,
      data: { label: "END" },
      deletable: false,
    };
  });

  const edges = g.edges.map<RFEdge>((e) => ({
    id: e.id,
    source: e.source_node_id,
    target: e.target_node_id,
    sourceHandle: e.branch,
    type: "labeledEdge",
    data: { branch: e.branch },
  }));

  return { nodes, edges, rootNodeId: g.root_node_id };
}

export function deserializeRuleGraph(
  rg: RuleGraph,
  outcomeTitleById: (id: string) => string,
): Record<CanvasKey, CanvasWorkingState> {
  return {
    anonymous: deserializeCanvas(rg.anonymous, outcomeTitleById),
    registered: deserializeCanvas(rg.registered, outcomeTitleById),
    customer: deserializeCanvas(rg.customer, outcomeTitleById),
  };
}
