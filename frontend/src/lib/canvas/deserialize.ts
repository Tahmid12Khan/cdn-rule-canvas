// Backend RuleGraph -> React Flow graph (BACKEND CONTRACT §6 ⇄ §3). Inverse of
// serialize.ts. (Task 14)
//
// Invariant (unit test): deserialize(serialize(g)) === g for all three
// canvases. `outcome.title` is NOT serialized (server state) — it is
// re-resolved here from the outcomes query via outcomeTitleById.
import type { CanvasGraph, RuleGraph } from "@/lib/api/ruleGraph";
import type { CanvasKey, RFEdge, RFNode } from "@/lib/canvas/types";
import type { CanvasWorkingState } from "@/lib/canvas/serialize";

export function deserializeCanvas(
  g: CanvasGraph,
  outcomeTitleById: (id: string) => string,
): CanvasWorkingState {
  const nodes = g.nodes.map<RFNode>((n) => {
    if (n.kind === "decision") {
      return {
        id: n.id,
        type: "decisionNode",
        position: { x: n.position.x, y: n.position.y },
        data: { processor: n.processor },
      };
    }
    return {
      id: n.id,
      type: "outcomeNode",
      position: { x: n.position.x, y: n.position.y },
      data: { outcomeId: n.outcome_id, title: outcomeTitleById(n.outcome_id) },
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
