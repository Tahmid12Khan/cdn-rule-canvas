import { describe, expect, it } from "vitest";

import type { RuleGraph } from "@/lib/api/ruleGraph";
import type { CanvasWorkingState } from "@/lib/canvas/serialize";
import type { CanvasKey, RFNode } from "@/lib/canvas/types";
import {
  buildValidationUserError,
  mapValidationErrors,
} from "@/lib/canvas/validationMapping";

const graph: RuleGraph = {
  anonymous: {
    root_node_id: "n_meta",
    nodes: [
      {
        kind: "decision",
        id: "n_meta",
        processor: {
          type: "meta_tags",
          tag_name: "x",
          operator: "exists",
          value: null,
        },
        position: { x: 0, y: 0 },
      },
      {
        kind: "outcome",
        id: "n_out",
        outcome_id: "33333333-3333-3333-3333-333333333333",
        position: { x: 100, y: 0 },
      },
    ],
    edges: [
      {
        id: "e1",
        source_node_id: "n_meta",
        target_node_id: "n_out",
        branch: "yes",
      },
    ],
  },
  registered: { nodes: [], edges: [], root_node_id: null },
  customer: { nodes: [], edges: [], root_node_id: null },
};

describe("mapValidationErrors", () => {
  it("maps a node loc to the node id", () => {
    const errors = mapValidationErrors(
      [
        {
          loc: "rule_graph.anonymous.nodes[0]",
          msg: "outcome ref missing",
          rule_id: "outcome_ref_exists",
        },
      ],
      graph,
    );
    expect(errors).toEqual({ n_meta: "outcome ref missing" });
  });

  it("attributes an edge loc to its source node id", () => {
    const errors = mapValidationErrors(
      [
        {
          loc: "rule_graph.anonymous.edges[0]",
          msg: "cycle detected",
          rule_id: "no_cycles",
        },
      ],
      graph,
    );
    expect(errors).toEqual({ n_meta: "cycle detected" });
  });

  it("ignores unparseable locs and missing details", () => {
    expect(mapValidationErrors(undefined, graph)).toEqual({});
    expect(
      mapValidationErrors(
        [{ loc: "something.else", msg: "x", rule_id: "y" }],
        graph,
      ),
    ).toEqual({});
  });
});

// RF canvases (store shape) used to resolve human node labels.
function canvas(nodes: RFNode[], rootNodeId: string | null): CanvasWorkingState {
  return { nodes, edges: [], rootNodeId };
}

const storeCanvases: Record<CanvasKey, CanvasWorkingState> = {
  anonymous: canvas(
    [
      {
        id: "n_meta",
        type: "decisionNode",
        position: { x: 0, y: 0 },
        data: {
          processor: {
            type: "meta_tags",
            tag_name: "Reg Wall",
            operator: "exists",
            value: null,
          },
        },
      },
      {
        id: "n_out",
        type: "outcomeNode",
        position: { x: 100, y: 0 },
        data: { outcomeId: "33333333-3333-3333-3333-333333333333", title: "Show Content" },
      },
    ],
    "n_meta",
  ),
  registered: canvas([], null),
  customer: canvas([], null),
};

describe("buildValidationUserError", () => {
  it("names the failing nodes by label and groups them by canvas", () => {
    const err = buildValidationUserError(
      [
        {
          loc: "rule_graph.anonymous.nodes[0]",
          msg: "outcome ref missing",
          rule_id: "outcome_ref_exists",
        },
        {
          loc: "rule_graph.anonymous.nodes[1]",
          msg: "outcome ref missing",
          rule_id: "outcome_ref_exists",
        },
      ],
      graph,
      storeCanvases,
    );
    expect(err.title).toBe("2 rule nodes need attention");
    expect(err.why).toContain("Anonymous canvas");
    expect(err.why).toContain("'Reg Wall'");
    expect(err.why).toContain("'Show Content'");
    expect(err.why).toContain("don't exist in this version");
    expect(err.howToFix).toMatch(/open each highlighted node/i);
    expect(err.retryable).toBe(false);
  });

  it("uses singular title for one node", () => {
    const err = buildValidationUserError(
      [
        {
          loc: "rule_graph.anonymous.nodes[0]",
          msg: "outcome ref missing",
          rule_id: "outcome_ref_exists",
        },
      ],
      graph,
      storeCanvases,
    );
    expect(err.title).toBe("1 rule node needs attention");
  });

  it("falls back to a generic message when no node loc resolves", () => {
    const err = buildValidationUserError(
      [{ loc: "body", msg: "bad", rule_id: "x" }],
      graph,
      storeCanvases,
    );
    expect(err.title).toMatch(/invalid/i);
    expect(err.why).toMatch(/couldn't pinpoint/i);
  });
});
