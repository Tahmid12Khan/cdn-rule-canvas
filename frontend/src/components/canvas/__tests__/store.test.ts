import { beforeEach, describe, expect, it } from "vitest";

import {
  computeJourneyPath,
  computeRootNodeId,
  isDirty,
  useRuleBuilderStore,
} from "@/state/ruleBuilderStore";
import { serializeRuleGraph } from "@/lib/canvas/serialize";
import { META_TAGS_DEFAULT } from "@/test/fixtures/nodeTypes";
import {
  END_NODE_ID,
  START_NODE_ID,
  type RFEdge,
  type RFNode,
} from "@/lib/canvas/types";
import type { RuleGraph } from "@/lib/api/ruleGraph";

function decision(id: string): RFNode {
  return {
    id,
    type: "decisionNode",
    position: { x: 0, y: 0 },
    data: { processor: { ...META_TAGS_DEFAULT } },
  };
}
function expression(id: string): RFNode {
  return {
    id,
    type: "expressionNode",
    position: { x: 0, y: 0 },
    data: {
      action: {
        type: "apply_outcome",
        outcome_id: "11111111-1111-1111-1111-111111111111",
      },
      outcomeTitle: "X",
    },
  };
}
function edge(id: string, source: string, target: string): RFEdge {
  return {
    id,
    source,
    target,
    sourceHandle: "yes",
    type: "labeledEdge",
    data: { branch: "yes" },
  };
}

const emptyGraph: RuleGraph = {
  anonymous: { nodes: [], edges: [], root_node_id: null },
  registered: { nodes: [], edges: [], root_node_id: null },
  customer: { nodes: [], edges: [], root_node_id: null },
};

// The store injects a start + end node into a canvas the first time a real node
// is added (one each per non-empty canvas — expression-nodes-spec §1/§3);
// helpers here count only the real (non-bookend) nodes.
function realNodes(k: "anonymous" | "registered" | "customer") {
  return useRuleBuilderStore
    .getState()
    .canvases[k].nodes.filter(
      (n) => n.type !== "startNode" && n.type !== "endNode",
    );
}

beforeEach(() => {
  useRuleBuilderStore.getState().seedFromRuleGraph(emptyGraph, "draft", () => "X");
});

describe("ruleBuilderStore", () => {
  it("addNode increments the canvas node count", () => {
    const s = useRuleBuilderStore.getState();
    s.addNode("anonymous", decision("n1"));
    expect(realNodes("anonymous")).toHaveLength(1);
  });

  it("a seeded EMPTY canvas has no nodes (empty canvas is valid)", () => {
    for (const k of ["anonymous", "registered", "customer"] as const) {
      expect(useRuleBuilderStore.getState().canvases[k].nodes).toHaveLength(0);
    }
  });

  it("adding the first real node injects one non-deletable start + end node", () => {
    useRuleBuilderStore.getState().addNode("anonymous", decision("n1"));
    const nodes = useRuleBuilderStore.getState().canvases.anonymous.nodes;
    const starts = nodes.filter((n) => n.type === "startNode");
    const ends = nodes.filter((n) => n.type === "endNode");
    expect(starts).toHaveLength(1);
    expect(starts[0].id).toBe(START_NODE_ID);
    expect(starts[0].deletable).toBe(false);
    expect(ends).toHaveLength(1);
    expect(ends[0].id).toBe(END_NODE_ID);
    expect(ends[0].deletable).toBe(false);
  });

  it("removeNode is a no-op for the start + end nodes", () => {
    const s = useRuleBuilderStore.getState();
    s.addNode("anonymous", decision("n1"));
    s.removeNode("anonymous", START_NODE_ID);
    s.removeNode("anonymous", END_NODE_ID);
    const nodes = useRuleBuilderStore.getState().canvases.anonymous.nodes;
    expect(nodes.filter((n) => n.type === "startNode")).toHaveLength(1);
    expect(nodes.filter((n) => n.type === "endNode")).toHaveLength(1);
  });

  it("removing the last real node returns the canvas to empty (bookends dropped)", () => {
    const s = useRuleBuilderStore.getState();
    s.addNode("anonymous", decision("d1"));
    s.removeNode("anonymous", "d1");
    expect(useRuleBuilderStore.getState().canvases.anonymous.nodes).toHaveLength(
      0,
    );
  });

  it("addEdge rejects an edge sourced from the end node", () => {
    const s = useRuleBuilderStore.getState();
    s.addNode("anonymous", decision("d1"));
    const ok = useRuleBuilderStore
      .getState()
      .addEdge("anonymous", edge("e1", END_NODE_ID, "d1"));
    expect(ok).toBe(false);
    expect(
      useRuleBuilderStore.getState().canvases.anonymous.edges,
    ).toHaveLength(0);
  });

  it("addEdge accepts an edge sourced from a decision node", () => {
    const s = useRuleBuilderStore.getState();
    s.addNode("anonymous", decision("d1"));
    s.addNode("anonymous", expression("o1"));
    const ok = useRuleBuilderStore
      .getState()
      .addEdge("anonymous", edge("e1", "d1", "o1"));
    expect(ok).toBe(true);
    expect(
      useRuleBuilderStore.getState().canvases.anonymous.edges,
    ).toHaveLength(1);
  });

  it("removeNode also removes connected edges", () => {
    const s = useRuleBuilderStore.getState();
    s.addNode("anonymous", decision("d1"));
    s.addNode("anonymous", expression("o1"));
    s.addEdge("anonymous", edge("e1", "d1", "o1"));
    s.removeNode("anonymous", "d1");
    const canvas = useRuleBuilderStore.getState().canvases.anonymous;
    // o1 remains (start + end nodes also remain, but realNodes excludes them).
    expect(realNodes("anonymous")).toHaveLength(1);
    expect(canvas.edges).toHaveLength(0);
  });

  it("switching selected canvas keeps both canvases unchanged", () => {
    const s = useRuleBuilderStore.getState();
    s.addNode("anonymous", decision("d1"));
    s.setSelected("registered");
    s.addNode("registered", decision("d2"));
    const state = useRuleBuilderStore.getState();
    expect(realNodes("anonymous")).toHaveLength(1);
    expect(realNodes("registered")).toHaveLength(1);
    expect(state.selected).toBe("registered");
  });

  it("toggleEdit flips the flag for a DRAFT version", () => {
    expect(useRuleBuilderStore.getState().isEditing).toBe(false);
    useRuleBuilderStore.getState().toggleEdit();
    expect(useRuleBuilderStore.getState().isEditing).toBe(true);
  });

  it("toggleEdit enters edit mode for a non-DRAFT version (edits stay local)", () => {
    useRuleBuilderStore.getState().seedFromRuleGraph(emptyGraph, "live", () => "X");
    expect(useRuleBuilderStore.getState().isEditing).toBe(false);
    useRuleBuilderStore.getState().toggleEdit();
    expect(useRuleBuilderStore.getState().isEditing).toBe(true);
    // versionStatus is still tracked so the UI can show a "local edits" banner.
    expect(useRuleBuilderStore.getState().versionStatus).toBe("live");
  });

  it("updateNodeProcessor updates a decision node's processor", () => {
    const s = useRuleBuilderStore.getState();
    s.addNode("anonymous", decision("d1"));
    s.updateNodeProcessor("anonymous", "d1", {
      type: "device_type",
      operator: "equals",
      value: "mobile",
    });
    const node = useRuleBuilderStore
      .getState()
      .canvases.anonymous.nodes.find((n) => n.id === "d1");
    expect(node?.type).toBe("decisionNode");
    expect(node && "processor" in node.data && node.data.processor.type).toBe(
      "device_type",
    );
  });

  it("updateNodeAction updates an expression node's action + outcome title", () => {
    const s = useRuleBuilderStore.getState();
    s.addNode("anonymous", expression("a1"));
    s.updateNodeAction(
      "anonymous",
      "a1",
      {
        type: "apply_outcome",
        outcome_id: "22222222-2222-2222-2222-222222222222",
      },
      "New Outcome",
    );
    const node = useRuleBuilderStore
      .getState()
      .canvases.anonymous.nodes.find((n) => n.id === "a1");
    expect(node?.type).toBe("expressionNode");
    expect(node && "action" in node.data && node.data.action.outcome_id).toBe(
      "22222222-2222-2222-2222-222222222222",
    );
    expect(node && "outcomeTitle" in node.data && node.data.outcomeTitle).toBe(
      "New Outcome",
    );
  });

  it("isDirty becomes true after adding a node and false after markSaved", () => {
    const s = useRuleBuilderStore.getState();
    s.addNode("anonymous", decision("d1"));
    expect(isDirty(useRuleBuilderStore.getState())).toBe(true);
    // The saved baseline = the current serialized graph.
    s.markSaved(serializeRuleGraph(useRuleBuilderStore.getState().canvases));
    expect(isDirty(useRuleBuilderStore.getState())).toBe(false);
  });

  it("dirty flag flips true on addNode and resets on markSaved/seed", () => {
    const s = useRuleBuilderStore.getState();
    expect(useRuleBuilderStore.getState().dirty).toBe(false);
    s.addNode("anonymous", decision("d1"));
    expect(useRuleBuilderStore.getState().dirty).toBe(true);
    useRuleBuilderStore.getState().seedFromRuleGraph(emptyGraph, "draft", () => "X");
    expect(useRuleBuilderStore.getState().dirty).toBe(false);
  });

  it("a pure selection change does not mark the canvas dirty", () => {
    const s = useRuleBuilderStore.getState();
    s.addNode("anonymous", decision("d1"));
    useRuleBuilderStore.getState().markSaved(emptyGraph);
    expect(useRuleBuilderStore.getState().dirty).toBe(false);
    useRuleBuilderStore
      .getState()
      .onNodesChange("anonymous", [{ id: "d1", type: "select", selected: true }]);
    expect(useRuleBuilderStore.getState().dirty).toBe(false);
  });

  it("setTestHighlight stores the path; clearTestHighlight removes it", () => {
    const s = useRuleBuilderStore.getState();
    s.setTestHighlight({
      nodeIds: new Set(["d1"]),
      edgeIds: new Set(["e1"]),
      outcomeNodeId: "o1",
      deadEnd: false,
    });
    expect(useRuleBuilderStore.getState().testHighlight?.nodeIds.has("d1")).toBe(
      true,
    );
    useRuleBuilderStore.getState().clearTestHighlight();
    expect(useRuleBuilderStore.getState().testHighlight).toBeNull();
  });

  it("setNodeErrors stores per-node messages; clearNodeErrors removes them", () => {
    const s = useRuleBuilderStore.getState();
    s.setNodeErrors({ d1: "cycle detected" });
    expect(useRuleBuilderStore.getState().nodeErrors.d1).toBe("cycle detected");
    s.clearNodeErrors();
    expect(useRuleBuilderStore.getState().nodeErrors).toEqual({});
  });

  it("setNodePositions bulk-updates positions and flips dirty", () => {
    const s = useRuleBuilderStore.getState();
    s.addNode("anonymous", decision("d1"));
    s.addNode("anonymous", expression("o1"));
    useRuleBuilderStore.getState().markSaved(emptyGraph);
    expect(useRuleBuilderStore.getState().dirty).toBe(false);

    useRuleBuilderStore.getState().setNodePositions(
      "anonymous",
      new Map([
        ["d1", { x: 100, y: 200 }],
        ["o1", { x: 300, y: 400 }],
      ]),
    );
    const nodes = useRuleBuilderStore.getState().canvases.anonymous.nodes;
    expect(nodes.find((n) => n.id === "d1")?.position).toEqual({ x: 100, y: 200 });
    expect(nodes.find((n) => n.id === "o1")?.position).toEqual({ x: 300, y: 400 });
    expect(useRuleBuilderStore.getState().dirty).toBe(true);
  });
});

describe("computeJourneyPath", () => {
  it("includes nodes/edges on any start->end path", () => {
    const s = useRuleBuilderStore.getState();
    s.addNode("anonymous", decision("d1"));
    // bookends now exist (start + end); wire start -> d1 -> end.
    s.addEdge("anonymous", edge("e0", START_NODE_ID, "d1"));
    s.addEdge("anonymous", edge("e1", "d1", END_NODE_ID));
    const journey = computeJourneyPath(
      useRuleBuilderStore.getState().canvases.anonymous,
    );
    expect(journey.nodeIds.has(START_NODE_ID)).toBe(true);
    expect(journey.nodeIds.has("d1")).toBe(true);
    expect(journey.nodeIds.has(END_NODE_ID)).toBe(true);
    expect(journey.edgeIds.has("e0")).toBe(true);
    expect(journey.edgeIds.has("e1")).toBe(true);
  });

  it("excludes a dead-end branch that never reaches an end", () => {
    const s = useRuleBuilderStore.getState();
    s.addNode("anonymous", decision("d1"));
    s.addNode("anonymous", decision("dead"));
    s.addEdge("anonymous", edge("e0", START_NODE_ID, "d1"));
    s.addEdge("anonymous", edge("e_yes", "d1", END_NODE_ID));
    // a "no" branch into a dead-end decision with no outgoing edge.
    s.addEdge("anonymous", {
      id: "e_no",
      source: "d1",
      target: "dead",
      sourceHandle: "no",
      type: "labeledEdge",
      data: { branch: "no" },
    });
    const journey = computeJourneyPath(
      useRuleBuilderStore.getState().canvases.anonymous,
    );
    expect(journey.nodeIds.has(END_NODE_ID)).toBe(true);
    expect(journey.nodeIds.has("dead")).toBe(false);
    expect(journey.edgeIds.has("e_no")).toBe(false);
  });
});

describe("computeRootNodeId", () => {
  it("returns the start node when present", () => {
    const nodes: RFNode[] = [
      {
        id: START_NODE_ID,
        type: "startNode",
        position: { x: 0, y: 0 },
        data: { label: "Start" },
      },
      decision("a"),
    ];
    expect(computeRootNodeId(nodes, [])).toBe(START_NODE_ID);
  });

  it("falls back to the single no-incoming node when there is no start", () => {
    const nodes = [decision("a"), decision("b")];
    const edges = [edge("e", "a", "b")];
    expect(computeRootNodeId(nodes, edges)).toBe("a");
  });

  it("returns null when empty", () => {
    expect(computeRootNodeId([], [])).toBeNull();
  });
});
