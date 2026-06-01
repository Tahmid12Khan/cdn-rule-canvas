import { describe, expect, it } from "vitest";

import { RuleGraph } from "@/lib/api/ruleGraph";
import { deserializeRuleGraph } from "@/lib/canvas/deserialize";
import { serializeCanvas, serializeRuleGraph } from "@/lib/canvas/serialize";
import { START_NODE_ID, type RFEdge, type RFNode } from "@/lib/canvas/types";

const OUTCOME_ID = "33333333-3333-3333-3333-333333333333";

// A 3-node, 2-edge fixture in one canvas; the other two canvases empty.
const fixture: RuleGraph = {
  anonymous: {
    root_node_id: "n_meta",
    nodes: [
      {
        kind: "decision",
        id: "n_meta",
        processor: {
          type: "meta_tags",
          tag_name: "paywall",
          operator: "contains",
          value: "true",
        },
        position: { x: 80, y: 200 },
      },
      {
        kind: "decision",
        id: "n_dev",
        processor: { type: "device_type", operator: "equals", value: "mobile" },
        position: { x: 360, y: 120 },
      },
      {
        kind: "outcome",
        id: "n_out",
        outcome_id: OUTCOME_ID,
        position: { x: 640, y: 60 },
      },
    ],
    edges: [
      {
        id: "e1",
        source_node_id: "n_meta",
        target_node_id: "n_dev",
        branch: "yes",
      },
      {
        id: "e2",
        source_node_id: "n_dev",
        target_node_id: "n_out",
        branch: "no",
      },
    ],
  },
  registered: { nodes: [], edges: [], root_node_id: null },
  customer: { nodes: [], edges: [], root_node_id: null },
};

describe("serialize / deserialize round-trip", () => {
  it("deserialize(serialize(g)) === g across all canvases", () => {
    const canvases = deserializeRuleGraph(fixture, () => "Show Content");
    const back = serializeRuleGraph(canvases);
    expect(back).toEqual(fixture);
  });

  it("round-trips position as numbers", () => {
    const canvases = deserializeRuleGraph(fixture, () => "Show Content");
    const back = serializeRuleGraph(canvases);
    expect(back.anonymous.nodes[0].position).toEqual({ x: 80, y: 200 });
  });

  it("does not serialize outcome titles (re-resolved on deserialize)", () => {
    const canvases = deserializeRuleGraph(fixture, () => "Resolved Title");
    const outcomeNode = canvases.anonymous.nodes.find(
      (n) => n.type === "outcomeNode",
    );
    expect(
      outcomeNode && "title" in outcomeNode.data && outcomeNode.data.title,
    ).toBe("Resolved Title");
    const back = serializeRuleGraph(canvases);
    const serializedOutcome = back.anonymous.nodes.find(
      (n) => n.kind === "outcome",
    );
    // No `title` key on the serialized outcome node.
    expect(serializedOutcome && "title" in serializedOutcome).toBe(false);
  });
});

describe("serialize strips the frontend-only start node", () => {
  const start: RFNode = {
    id: START_NODE_ID,
    type: "startNode",
    position: { x: 260, y: 0 },
    data: { label: "Start" },
  };
  const root: RFNode = {
    id: "n_root",
    type: "decisionNode",
    position: { x: 0, y: 120 },
    data: {
      processor: {
        type: "meta_tags",
        tag_name: "paywall",
        operator: "exists",
        value: null,
      },
    },
  };
  const startEdge: RFEdge = {
    id: "edge_start",
    source: START_NODE_ID,
    target: "n_root",
    sourceHandle: "yes",
    type: "labeledEdge",
    data: { branch: "yes" },
  };

  it("omits the start node and any edge touching it", () => {
    const g = serializeCanvas([start, root], [startEdge], "n_root");
    expect(g.nodes.map((n) => n.id)).toEqual(["n_root"]);
    expect(g.edges).toHaveLength(0);
    // Root detection is unaffected by the start node.
    expect(g.root_node_id).toBe("n_root");
  });
});
