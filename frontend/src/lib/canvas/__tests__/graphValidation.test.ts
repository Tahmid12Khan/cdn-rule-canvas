import { describe, expect, it } from "vitest";

import {
  CYCLE_MESSAGE,
  UNREACHABLE_OUTCOME_MESSAGE,
  buildClientValidationUserError,
  computeRoot,
  findCycle,
  findUnreachableOutcomeNodes,
  validateAllCanvases,
  validateCanvasGraph,
} from "@/lib/canvas/graphValidation";
import type { CanvasWorkingState } from "@/lib/canvas/serialize";
import { START_NODE_ID, type RFEdge, type RFNode } from "@/lib/canvas/types";

// --- builders --------------------------------------------------------------

function decision(id: string): RFNode {
  return {
    id,
    type: "decisionNode",
    position: { x: 0, y: 0 },
    data: { processor: { type: "device_type" } },
  };
}

function outcome(id: string): RFNode {
  return {
    id,
    type: "outcomeNode",
    position: { x: 0, y: 0 },
    data: { outcomeId: "00000000-0000-0000-0000-000000000000", title: "Out" },
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
  it("ignores the start node and its edge", () => {
    const nodes = [start(), decision("a"), outcome("b")];
    const edges = [edge(START_NODE_ID, "a"), edge("a", "b")];
    expect(computeRoot(nodes, edges)).toBe("a");
  });

  it("returns null when there is no single root", () => {
    const nodes = [decision("a"), decision("b")];
    expect(computeRoot(nodes, [])).toBeNull();
  });

  it("returns null for an empty (real-node) canvas", () => {
    expect(computeRoot([start()], [])).toBeNull();
  });
});

// --- findCycle -------------------------------------------------------------

describe("findCycle", () => {
  it("returns null for an acyclic diamond", () => {
    const nodes = [decision("a"), decision("b"), decision("c"), outcome("d")];
    const edges = [
      edge("a", "b", "yes"),
      edge("a", "c", "no"),
      edge("b", "d"),
      edge("c", "d"),
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

  it("excludes the start node + start edge from cycle detection", () => {
    const nodes = [start(), decision("a"), outcome("b")];
    const edges = [edge(START_NODE_ID, "a"), edge("a", "b")];
    expect(findCycle(nodes, edges)).toBeNull();
  });
});

// --- findUnreachableOutcomeNodes (mirrors backend §2.1) --------------------

describe("findUnreachableOutcomeNodes", () => {
  it("skips an empty canvas", () => {
    expect(findUnreachableOutcomeNodes([start()], [])).toEqual([]);
  });

  it("skips when there is no single root", () => {
    const nodes = [decision("a"), decision("b")];
    expect(findUnreachableOutcomeNodes(nodes, [])).toEqual([]);
  });

  it("passes an outcome-only root", () => {
    const nodes = [outcome("a")];
    expect(findUnreachableOutcomeNodes(nodes, [], "a")).toEqual([]);
  });

  it("passes a decision -> decision -> outcome chain", () => {
    const nodes = [decision("a"), decision("b"), outcome("c")];
    const edges = [edge("a", "b"), edge("b", "c")];
    expect(findUnreachableOutcomeNodes(nodes, edges, "a")).toEqual([]);
  });

  it("fails a dead-end decision reachable from the root", () => {
    // a -> b (outcome) on yes; a -> c (dead-end decision) on no.
    const nodes = [decision("a"), outcome("b"), decision("c")];
    const edges = [edge("a", "b", "yes"), edge("a", "c", "no")];
    const dead = findUnreachableOutcomeNodes(nodes, edges, "a");
    expect(dead).toEqual(["c"]);
  });
});

// --- validateCanvasGraph / validateAllCanvases -----------------------------

describe("validateCanvasGraph", () => {
  it("returns no errors for a valid rooted graph", () => {
    const c = canvas(
      [start(), decision("a"), outcome("b")],
      [edge(START_NODE_ID, "a"), edge("a", "b")],
    );
    const result = validateCanvasGraph(c);
    expect(result.nodeErrors).toEqual({});
    expect(result.problems).toEqual([]);
  });

  it("flags cycle members and dead-ends with server-aligned copy", () => {
    const c = canvas(
      [start(), decision("a"), decision("c")],
      [edge(START_NODE_ID, "a"), edge("a", "c"), edge("c", "a")],
    );
    const result = validateCanvasGraph(c);
    expect(result.nodeErrors["a"]).toBe(CYCLE_MESSAGE);
    expect(result.problems).toContain("some rules form a cycle");
  });

  it("flags an unreachable-outcome dead-end", () => {
    const c = canvas(
      [start(), decision("a"), outcome("b"), decision("c")],
      [
        edge(START_NODE_ID, "a"),
        edge("a", "b", "yes"),
        edge("a", "c", "no"),
      ],
    );
    const result = validateCanvasGraph(c);
    expect(result.nodeErrors["c"]).toBe(UNREACHABLE_OUTCOME_MESSAGE);
    expect(result.problems).toContain("some rules can't reach an outcome");
  });
});

describe("validateAllCanvases", () => {
  it("aggregates and labels per-canvas problems", () => {
    const bad = canvas(
      [start(), decision("a"), decision("c")],
      [edge(START_NODE_ID, "a"), edge("a", "c"), edge("c", "a")],
    );
    const good = canvas(
      [start(), decision("x"), outcome("y")],
      [edge(START_NODE_ID, "x"), edge("x", "y")],
    );
    const result = validateAllCanvases({
      anonymous: bad,
      registered: good,
      customer: good,
    });
    expect(result.problems.some((p) => p.startsWith("On the Anonymous canvas,"))).toBe(
      true,
    );
    expect(result.nodeErrors["a"]).toBe(CYCLE_MESSAGE);
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
