// Backend RuleGraph -> React Flow graph (BACKEND CONTRACT §6 ⇄
// expression-nodes-spec §1). Inverse of serialize.ts.
//
// Invariant (unit test): deserialize(serialize(g)) === g for all three
// canvases. The apply_outcome action's outcome title is NOT serialized (server
// state) — it is re-resolved here from the outcomes query via outcomeTitleById
// and cached on the expression node's data.
import type { CanvasGraph, RuleGraph } from "@/lib/api/ruleGraph";
import {
  END_NODE_ID,
  START_NODE_ID,
  type CanvasKey,
  type RFEdge,
  type RFNode,
} from "@/lib/canvas/types";
import type { CanvasWorkingState } from "@/lib/canvas/serialize";

// Default start → end canvas (spec §6). Returned whenever the backend sends
// an empty canvas (zero nodes). Positions and ids are locked by the spec.
function defaultCanvas(): CanvasWorkingState {
  const s: RFNode = {
    id: START_NODE_ID,
    type: "startNode",
    position: { x: 40, y: 160 },
    data: { label: "Start" },
    deletable: false,
  };
  const e: RFNode = {
    id: END_NODE_ID,
    type: "endNode",
    position: { x: 940, y: 160 },
    data: { label: "END" },
    deletable: false,
  };
  const edge: RFEdge = {
    id: "e_start_end",
    source: START_NODE_ID,
    // Start node's only source handle is "out" — sourceHandle MUST match it or
    // React Flow v12 drops the edge (error #008). Branch stays "yes" by convention.
    sourceHandle: "out",
    target: END_NODE_ID,
    type: "labeledEdge",
    data: { branch: "yes" },
  };
  return { nodes: [s, e], edges: [edge], rootNodeId: START_NODE_ID };
}

export function deserializeCanvas(
  g: CanvasGraph,
  outcomeTitleById: (id: string) => string,
  // Re-resolves an apply_component's display name from the components query.
  // Optional (defaults to none) so legacy call sites stay valid.
  componentNameById: (id: string) => string | undefined = () => undefined,
): CanvasWorkingState {
  // Empty backend canvas → inject the default start → end graph (spec §6).
  if (g.nodes.length === 0) return defaultCanvas();

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
      // apply_component / apply_component_json carry component_id — re-resolve a
      // display name so canvas/journey stay current (mirrors outcomeTitle).
      const componentId = n.action.component_id;
      const componentName =
        typeof componentId === "string"
          ? componentNameById(componentId)
          : undefined;
      return {
        id: n.id,
        type: "expressionNode",
        position,
        // custom_label round-trips off the wire (spec §v2.3); absent when unset.
        data: {
          action: n.action,
          outcomeTitle,
          componentName,
          custom_label: n.custom_label,
        },
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

  // Source-node kind decides the render handle id: a decision routes via its
  // "yes"/"no" handles; start/expression have a single "out" handle. Setting
  // sourceHandle to the wire branch ("yes") for a start/expression source makes
  // React Flow v12 drop the edge (error #008), so map it to the real handle.
  const kindById = new Map(g.nodes.map((n) => [n.id, n.kind]));
  const edges = g.edges.map<RFEdge>((e) => ({
    id: e.id,
    source: e.source_node_id,
    target: e.target_node_id,
    sourceHandle:
      kindById.get(e.source_node_id) === "decision" ? e.branch : "out",
    type: "labeledEdge",
    data: { branch: e.branch },
  }));

  return { nodes, edges, rootNodeId: g.root_node_id };
}

export function deserializeRuleGraph(
  rg: RuleGraph,
  outcomeTitleById: (id: string) => string,
  componentNameById: (id: string) => string | undefined = () => undefined,
): Record<CanvasKey, CanvasWorkingState> {
  return {
    anonymous: deserializeCanvas(
      rg.anonymous,
      outcomeTitleById,
      componentNameById,
    ),
    registered: deserializeCanvas(
      rg.registered,
      outcomeTitleById,
      componentNameById,
    ),
    customer: deserializeCanvas(
      rg.customer,
      outcomeTitleById,
      componentNameById,
    ),
  };
}
