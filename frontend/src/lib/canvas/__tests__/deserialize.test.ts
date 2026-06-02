import { describe, expect, it } from "vitest";

import { deserializeCanvas } from "@/lib/canvas/deserialize";
import { serializeCanvas } from "@/lib/canvas/serialize";
import type { CanvasGraph } from "@/lib/api/ruleGraph";

// Regression: React Flow v12 DROPS an edge whose `sourceHandle` doesn't match a
// real source handle on the node (error #008). Start/expression nodes expose a
// single "out" handle; only decision nodes have "yes"/"no". So deserialize must
// map sourceHandle to the real handle id while keeping the wire branch in
// data.branch. https://reactflow.dev/error#008
const graph: CanvasGraph = {
  root_node_id: "start",
  nodes: [
    { kind: "start", id: "start", position: { x: 0, y: 0 } },
    {
      kind: "decision",
      id: "d1",
      processor: { type: "json_expression" },
      position: { x: 100, y: 0 },
    },
    {
      kind: "expression",
      id: "t1",
      action: { type: "trim_json" },
      position: { x: 200, y: 0 },
    },
    { kind: "end", id: "end", position: { x: 300, y: 0 } },
  ],
  edges: [
    { id: "e_start", source_node_id: "start", target_node_id: "d1", branch: "yes" },
    { id: "e_yes", source_node_id: "d1", target_node_id: "t1", branch: "yes" },
    { id: "e_no", source_node_id: "d1", target_node_id: "end", branch: "no" },
    { id: "e_trim", source_node_id: "t1", target_node_id: "end", branch: "yes" },
  ],
};

const byId = (ws: ReturnType<typeof deserializeCanvas>, id: string) =>
  ws.edges.find((e) => e.id === id)!;

describe("deserializeCanvas — sourceHandle render mapping (RF v12 #008)", () => {
  const ws = deserializeCanvas(graph, () => "Outcome");

  it('maps a start-sourced edge to the "out" handle, branch stays yes', () => {
    expect(byId(ws, "e_start").sourceHandle).toBe("out");
    expect(byId(ws, "e_start").data?.branch).toBe("yes");
  });

  it('maps an expression-sourced edge to the "out" handle', () => {
    expect(byId(ws, "e_trim").sourceHandle).toBe("out");
    expect(byId(ws, "e_trim").data?.branch).toBe("yes");
  });

  it("keeps decision-sourced edges on their yes/no handles", () => {
    expect(byId(ws, "e_yes").sourceHandle).toBe("yes");
    expect(byId(ws, "e_no").sourceHandle).toBe("no");
  });

  it("round-trips wire branches despite the out-handle remap", () => {
    const back = serializeCanvas(ws.nodes, ws.edges, ws.rootNodeId);
    const branchById = Object.fromEntries(
      back.edges.map((e) => [e.id, e.branch]),
    );
    expect(branchById).toEqual({
      e_start: "yes",
      e_yes: "yes",
      e_no: "no",
      e_trim: "yes",
    });
  });
});
