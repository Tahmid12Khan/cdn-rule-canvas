import { describe, expect, it } from "vitest";

import { autoLayout } from "@/lib/canvas/layout";
import { START_NODE_ID, type RFEdge, type RFNode } from "@/lib/canvas/types";

function decision(id: string): RFNode {
  return {
    id,
    type: "decisionNode",
    position: { x: 0, y: 0 },
    data: { processor: { type: "device_type" } },
  };
}
function end(id: string): RFNode {
  return {
    id,
    type: "endNode",
    position: { x: 0, y: 0 },
    data: { label: "END" },
    deletable: false,
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
function edge(source: string, target: string): RFEdge {
  return {
    id: `e_${source}_${target}`,
    source,
    target,
    sourceHandle: "yes",
    type: "labeledEdge",
    data: { branch: "yes" },
  } as RFEdge;
}

describe("autoLayout", () => {
  const nodes = [start(), decision("a"), end("b"), end("c")];
  const edges = [
    edge(START_NODE_ID, "a"),
    edge("a", "b"),
    edge("a", "c"),
  ];

  it("assigns a position to every node", () => {
    const positions = autoLayout(nodes, edges);
    for (const n of nodes) {
      expect(positions.has(n.id)).toBe(true);
    }
  });

  it("ranks the start node above its successor (TB)", () => {
    const positions = autoLayout(nodes, edges);
    const startPos = positions.get(START_NODE_ID)!;
    const aPos = positions.get("a")!;
    const bPos = positions.get("b")!;
    expect(startPos.y).toBeLessThan(aPos.y);
    expect(aPos.y).toBeLessThan(bPos.y);
  });

  it("returns finite numeric coordinates", () => {
    const positions = autoLayout(nodes, edges);
    for (const { x, y } of positions.values()) {
      expect(Number.isFinite(x)).toBe(true);
      expect(Number.isFinite(y)).toBe(true);
    }
  });

  it("does not crash on a single isolated node with no edges", () => {
    const positions = autoLayout([decision("solo")], []);
    expect(positions.get("solo")).toBeDefined();
  });
});
