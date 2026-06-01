import { beforeEach, describe, expect, it } from "vitest";

import {
  computeRootNodeId,
  isDirty,
  useRuleBuilderStore,
} from "@/state/ruleBuilderStore";
import { DEFAULT_META_TAGS } from "@/lib/canvas/nodeTemplates";
import type { RFEdge, RFNode } from "@/lib/canvas/types";
import type { RuleGraph } from "@/lib/api/ruleGraph";

function decision(id: string): RFNode {
  return {
    id,
    type: "decisionNode",
    position: { x: 0, y: 0 },
    data: { processor: { ...DEFAULT_META_TAGS } },
  };
}
function outcome(id: string): RFNode {
  return {
    id,
    type: "outcomeNode",
    position: { x: 0, y: 0 },
    data: { outcomeId: "11111111-1111-1111-1111-111111111111", title: "X" },
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

// The store injects a frontend-only "start" node into every canvas on seed
// (Task C); helpers here count only the real (non-start) nodes.
function realNodes(k: "anonymous" | "registered" | "customer") {
  return useRuleBuilderStore
    .getState()
    .canvases[k].nodes.filter((n) => n.type !== "startNode");
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

  it("seeds exactly one non-deletable start node per canvas", () => {
    for (const k of ["anonymous", "registered", "customer"] as const) {
      const starts = useRuleBuilderStore
        .getState()
        .canvases[k].nodes.filter((n) => n.type === "startNode");
      expect(starts).toHaveLength(1);
      expect(starts[0].id).toBe("start");
      expect(starts[0].deletable).toBe(false);
    }
  });

  it("removeNode is a no-op for the start node", () => {
    const s = useRuleBuilderStore.getState();
    s.removeNode("anonymous", "start");
    const starts = useRuleBuilderStore
      .getState()
      .canvases.anonymous.nodes.filter((n) => n.type === "startNode");
    expect(starts).toHaveLength(1);
  });

  it("addEdge rejects an edge sourced from an outcome node", () => {
    const s = useRuleBuilderStore.getState();
    s.addNode("anonymous", outcome("o1"));
    s.addNode("anonymous", decision("d1"));
    const ok = useRuleBuilderStore
      .getState()
      .addEdge("anonymous", edge("e1", "o1", "d1"));
    expect(ok).toBe(false);
    expect(
      useRuleBuilderStore.getState().canvases.anonymous.edges,
    ).toHaveLength(0);
  });

  it("addEdge accepts an edge sourced from a decision node", () => {
    const s = useRuleBuilderStore.getState();
    s.addNode("anonymous", decision("d1"));
    s.addNode("anonymous", outcome("o1"));
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
    s.addNode("anonymous", outcome("o1"));
    s.addEdge("anonymous", edge("e1", "d1", "o1"));
    s.removeNode("anonymous", "d1");
    const canvas = useRuleBuilderStore.getState().canvases.anonymous;
    // o1 remains (start node also remains, but realNodes excludes it).
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

  it("isDirty becomes true after adding a node and false after markSaved", () => {
    const s = useRuleBuilderStore.getState();
    s.addNode("anonymous", decision("d1"));
    expect(isDirty(useRuleBuilderStore.getState())).toBe(true);
    const rg = {
      anonymous: {
        nodes: [
          {
            kind: "decision" as const,
            id: "d1",
            processor: { ...DEFAULT_META_TAGS },
            position: { x: 0, y: 0 },
          },
        ],
        edges: [],
        root_node_id: "d1",
      },
      registered: { nodes: [], edges: [], root_node_id: null },
      customer: { nodes: [], edges: [], root_node_id: null },
    };
    s.markSaved(rg);
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
});

describe("computeRootNodeId", () => {
  it("returns the single node with no incoming edges", () => {
    const nodes = [decision("a"), decision("b")];
    const edges = [edge("e", "a", "b")];
    expect(computeRootNodeId(nodes, edges)).toBe("a");
  });

  it("returns null when ambiguous", () => {
    const nodes = [decision("a"), decision("b")];
    expect(computeRootNodeId(nodes, [])).toBeNull();
  });

  it("returns null when empty", () => {
    expect(computeRootNodeId([], [])).toBeNull();
  });
});
