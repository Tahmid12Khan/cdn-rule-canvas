import { describe, expect, it } from "vitest";

import { RuleGraph } from "@/lib/api/ruleGraph";
import { deserializeRuleGraph } from "@/lib/canvas/deserialize";
import { serializeCanvas, serializeRuleGraph } from "@/lib/canvas/serialize";
import {
  END_NODE_ID,
  START_NODE_ID,
  type RFEdge,
  type RFNode,
} from "@/lib/canvas/types";

const OUTCOME_ID = "33333333-3333-3333-3333-333333333333";

// Canonical backend form of an empty canvas (spec §6): start -> end.
// Under the new spec the backend normalizes empty canvases to this shape.
const emptyCanonical = {
  root_node_id: "start",
  nodes: [
    { kind: "start" as const, id: "start", position: { x: 40, y: 160 } },
    { kind: "end" as const, id: "end", position: { x: 940, y: 160 } },
  ],
  edges: [
    {
      id: "e_start_end",
      source_node_id: "start",
      target_node_id: "end",
      branch: "yes" as const,
    },
  ],
};

// A start -> decision -> expression(apply_outcome) -> end fixture in one
// canvas; the other two canvases in canonical empty form.
const fixture: RuleGraph = {
  anonymous: {
    root_node_id: "start",
    nodes: [
      {
        kind: "start",
        id: "start",
        position: { x: 260, y: 0 },
      },
      {
        kind: "decision",
        id: "n_dev",
        processor: { type: "device_type", operator: "equals", value: "mobile" },
        position: { x: 360, y: 120 },
      },
      {
        kind: "expression",
        id: "n_act",
        action: { type: "apply_outcome", outcome_id: OUTCOME_ID },
        position: { x: 640, y: 240 },
      },
      {
        kind: "end",
        id: "end",
        position: { x: 640, y: 360 },
      },
    ],
    edges: [
      {
        id: "e0",
        source_node_id: "start",
        target_node_id: "n_dev",
        branch: "yes",
      },
      {
        id: "e1",
        source_node_id: "n_dev",
        target_node_id: "n_act",
        branch: "yes",
      },
      {
        id: "e2",
        source_node_id: "n_act",
        target_node_id: "end",
        branch: "yes",
      },
    ],
  },
  registered: emptyCanonical,
  customer: emptyCanonical,
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
    expect(back.anonymous.nodes[0].position).toEqual({ x: 260, y: 0 });
  });

  it("does not serialize the apply_outcome title (re-resolved on deserialize)", () => {
    const canvases = deserializeRuleGraph(fixture, () => "Resolved Title");
    const exprNode = canvases.anonymous.nodes.find(
      (n) => n.type === "expressionNode",
    );
    expect(
      exprNode && "outcomeTitle" in exprNode.data && exprNode.data.outcomeTitle,
    ).toBe("Resolved Title");
    const back = serializeRuleGraph(canvases);
    const serializedExpr = back.anonymous.nodes.find(
      (n) => n.kind === "expression",
    );
    // The wire action carries only outcome_id, never the resolved title.
    expect(
      serializedExpr && "action" in serializedExpr && serializedExpr.action,
    ).toEqual({ type: "apply_outcome", outcome_id: OUTCOME_ID });
  });
});

describe("deserialize of empty backend canvas (spec §6)", () => {
  const emptyBackend: RuleGraph = {
    anonymous: { nodes: [], edges: [], root_node_id: null },
    registered: { nodes: [], edges: [], root_node_id: null },
    customer: { nodes: [], edges: [], root_node_id: null },
  };

  it("yields start node with spec position {x:40,y:160}", () => {
    const canvases = deserializeRuleGraph(emptyBackend, () => "");
    const s = canvases.anonymous.nodes.find((n) => n.id === START_NODE_ID);
    expect(s?.type).toBe("startNode");
    expect(s?.position).toEqual({ x: 40, y: 160 });
  });

  it("yields end node with spec position {x:940,y:160}", () => {
    const canvases = deserializeRuleGraph(emptyBackend, () => "");
    const e = canvases.anonymous.nodes.find((n) => n.id === END_NODE_ID);
    expect(e?.type).toBe("endNode");
    expect(e?.position).toEqual({ x: 940, y: 160 });
  });

  it("yields a single edge e_start_end source=start target=end branch=yes", () => {
    const canvases = deserializeRuleGraph(emptyBackend, () => "");
    expect(canvases.anonymous.edges).toHaveLength(1);
    const edge = canvases.anonymous.edges[0];
    expect(edge.id).toBe("e_start_end");
    expect(edge.source).toBe(START_NODE_ID);
    expect(edge.target).toBe(END_NODE_ID);
    expect(edge.data?.branch).toBe("yes");
  });

  it("yields rootNodeId = 'start'", () => {
    const canvases = deserializeRuleGraph(emptyBackend, () => "");
    expect(canvases.anonymous.rootNodeId).toBe(START_NODE_ID);
    expect(canvases.registered.rootNodeId).toBe(START_NODE_ID);
    expect(canvases.customer.rootNodeId).toBe(START_NODE_ID);
  });
});

describe("serialize PERSISTS the start + end nodes", () => {
  const start: RFNode = {
    id: START_NODE_ID,
    type: "startNode",
    position: { x: 260, y: 0 },
    data: { label: "Start" },
    deletable: false,
  };
  const decision: RFNode = {
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
  const end: RFNode = {
    id: END_NODE_ID,
    type: "endNode",
    position: { x: 0, y: 360 },
    data: { label: "END" },
    deletable: false,
  };
  const startEdge: RFEdge = {
    id: "edge_start",
    source: START_NODE_ID,
    target: "n_root",
    sourceHandle: "yes",
    type: "labeledEdge",
    data: { branch: "yes" },
  };
  const endEdge: RFEdge = {
    id: "edge_end",
    source: "n_root",
    target: END_NODE_ID,
    sourceHandle: "yes",
    type: "labeledEdge",
    data: { branch: "yes" },
  };

  it("emits start / decision / end nodes and all edges (no stripping)", () => {
    const g = serializeCanvas(
      [start, decision, end],
      [startEdge, endEdge],
      "start",
    );
    expect(g.nodes.map((n) => n.kind)).toEqual(["start", "decision", "end"]);
    expect(g.edges.map((e) => e.id)).toEqual(["edge_start", "edge_end"]);
    expect(g.root_node_id).toBe("start");
  });
});
