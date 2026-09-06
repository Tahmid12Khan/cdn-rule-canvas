import { describe, expect, it } from "vitest";

import {
  CYCLE_MESSAGE,
  UNREACHABLE_END_MESSAGE,
  buildClientValidationUserError,
  computeRoot,
  findCycle,
  findUnreachableEndNodes,
  validateCanvasGraph,
} from "@/lib/canvas/graphValidation";
import type { CanvasWorkingState } from "@/lib/canvas/serialize";
import {
  END_NODE_ID,
  START_NODE_ID,
  type RFEdge,
  type RFNode,
} from "@/lib/canvas/types";

// --- builders --------------------------------------------------------------

function decision(id: string): RFNode {
  return {
    id,
    type: "decisionNode",
    position: { x: 0, y: 0 },
    data: { processor: { type: "device_type" } },
  };
}

function start(): RFNode {
  return {
    id: START_NODE_ID,
    type: "startNode",
    position: { x: 0, y: 0 },
    data: { label: "Start" },
    deletable: false,
  };
}

function end(id: string = END_NODE_ID): RFNode {
  return {
    id,
    type: "endNode",
    position: { x: 0, y: 0 },
    data: { label: "END" },
    deletable: false,
  };
}

function edge(source: string, target: string, branch: "yes" | "no" = "yes"): RFEdge {
  return {
    id: `e_${source}_${target}_${branch}`,
    source,
    target,
    sourceHandle: branch,
    type: "labeledEdge",
    data: { branch },
  } as RFEdge;
}

function canvas(nodes: RFNode[], edges: RFEdge[]): CanvasWorkingState {
  // rootNodeId derived to match the store's contract.
  return { nodes, edges, rootNodeId: computeRoot(nodes, edges) };
}

// --- computeRoot -----------------------------------------------------------

describe("computeRoot", () => {
  it("returns the start node", () => {
    const nodes = [start(), decision("a"), end()];
    const edges = [edge(START_NODE_ID, "a"), edge("a", END_NODE_ID)];
    expect(computeRoot(nodes, edges)).toBe(START_NODE_ID);
  });

  it("falls back to a single no-incoming node when there is no start", () => {
    const nodes = [decision("a"), decision("b")];
    expect(computeRoot(nodes, [edge("a", "b")])).toBe("a");
  });

  it("returns null for an empty canvas", () => {
    expect(computeRoot([], [])).toBeNull();
  });
});

// --- findCycle -------------------------------------------------------------

describe("findCycle", () => {
  it("returns null for an acyclic diamond", () => {
    const nodes = [decision("a"), decision("b"), decision("c"), end()];
    const edges = [
      edge("a", "b", "yes"),
      edge("a", "c", "no"),
      edge("b", END_NODE_ID),
      edge("c", END_NODE_ID),
    ];
    expect(findCycle(nodes, edges)).toBeNull();
  });

  it("detects a back-edge cycle and names its members", () => {
    const nodes = [decision("a"), decision("b"), decision("c")];
    const edges = [edge("a", "b"), edge("b", "c"), edge("c", "a")];
    const cycle = findCycle(nodes, edges);
    expect(cycle).not.toBeNull();
    expect([...(cycle ?? [])].sort()).toEqual(["a", "b", "c"]);
  });

  it("a start -> decision -> end chain is acyclic", () => {
    const nodes = [start(), decision("a"), end()];
    const edges = [edge(START_NODE_ID, "a"), edge("a", END_NODE_ID)];
    expect(findCycle(nodes, edges)).toBeNull();
  });
});

// --- findUnreachableEndNodes (mirrors backend all_paths_reach_end) ---------

describe("findUnreachableEndNodes", () => {
  it("skips an empty canvas", () => {
    expect(findUnreachableEndNodes([], [])).toEqual([]);
  });

  it("passes a start -> decision -> end chain", () => {
    const nodes = [start(), decision("a"), end()];
    const edges = [edge(START_NODE_ID, "a"), edge("a", END_NODE_ID)];
    expect(findUnreachableEndNodes(nodes, edges, START_NODE_ID)).toEqual([]);
  });

  it("fails a dead-end decision reachable from the start", () => {
    // start -> a; a -> end on yes; a -> c (dead-end decision) on no.
    const nodes = [start(), decision("a"), end(), decision("c")];
    const edges = [
      edge(START_NODE_ID, "a"),
      edge("a", END_NODE_ID, "yes"),
      edge("a", "c", "no"),
    ];
    const dead = findUnreachableEndNodes(nodes, edges, START_NODE_ID);
    expect(dead).toEqual(["c"]);
  });
});

// --- validateCanvasGraph ----------------------------------------------------

describe("validateCanvasGraph", () => {
  it("returns no errors for a valid start -> decision -> end graph", () => {
    const c = canvas(
      [start(), decision("a"), end()],
      [edge(START_NODE_ID, "a"), edge("a", END_NODE_ID)],
    );
    const result = validateCanvasGraph(c);
    expect(result.nodeErrors).toEqual({});
    expect(result.problems).toEqual([]);
  });

  it("an empty canvas is valid", () => {
    const result = validateCanvasGraph(canvas([], []));
    expect(result.problems).toEqual([]);
    expect(result.nodeErrors).toEqual({});
  });

  it("flags cycle members with server-aligned copy", () => {
    const c = canvas(
      [start(), decision("a"), decision("c"), end()],
      [
        edge(START_NODE_ID, "a"),
        edge("a", "c"),
        edge("c", "a"),
        edge("a", END_NODE_ID),
      ],
    );
    const result = validateCanvasGraph(c);
    expect(result.nodeErrors["a"]).toBe(CYCLE_MESSAGE);
    expect(result.problems).toContain("some rules form a cycle");
  });

  it("flags an unreachable-end dead-end", () => {
    const c = canvas(
      [start(), decision("a"), end(), decision("c")],
      [
        edge(START_NODE_ID, "a"),
        edge("a", END_NODE_ID, "yes"),
        edge("a", "c", "no"),
      ],
    );
    const result = validateCanvasGraph(c);
    expect(result.nodeErrors["c"]).toBe(UNREACHABLE_END_MESSAGE);
    expect(result.problems).toContain("some rules can't reach an end");
  });

  it("flags a missing end node", () => {
    const c = canvas([start(), decision("a")], [edge(START_NODE_ID, "a")]);
    const result = validateCanvasGraph(c);
    expect(
      result.problems.some((p) => p.includes("at least one end node")),
    ).toBe(true);
  });
});

describe("buildClientValidationUserError", () => {
  it("packs problems into the why line", () => {
    const ue = buildClientValidationUserError(["thing one.", "thing two."]);
    expect(ue.title).toBe("Fix the rule graph before saving");
    expect(ue.why).toContain("thing one.");
    expect(ue.retryable).toBe(false);
  });
});
