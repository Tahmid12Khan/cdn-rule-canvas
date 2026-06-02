import { describe, expect, it } from "vitest";

import { diffRuleGraph, type CanvasDiff } from "@/lib/canvas/diff";
import type {
  CanvasGraph,
  Edge,
  GraphNode,
  RuleGraph,
} from "@/lib/api/ruleGraph";

const pos = (x = 0, y = 0) => ({ x, y });

const start = (p = pos(0, 0)): GraphNode => ({
  kind: "start",
  id: "start",
  position: p,
});
const end = (p = pos(0, 300)): GraphNode => ({
  kind: "end",
  id: "end",
  position: p,
});
const decision = (
  id: string,
  fields: Record<string, unknown> = { operator: "equals", value: "mobile" },
  p = pos(0, 150),
): GraphNode => ({
  kind: "decision",
  id,
  processor: { type: "device_type", ...fields },
  position: p,
});
const expression = (
  id: string,
  custom_label?: string,
  p = pos(0, 150),
): GraphNode => ({
  kind: "expression",
  id,
  action: { type: "apply_outcome", outcome_id: "o1" },
  ...(custom_label ? { custom_label } : {}),
  position: p,
});
const edge = (id: string, s: string, t: string, branch: "yes" | "no"): Edge => ({
  id,
  source_node_id: s,
  target_node_id: t,
  branch,
});

const empty: CanvasGraph = { nodes: [], edges: [], root_node_id: null };

function canvas(nodes: GraphNode[], edges: Edge[]): CanvasGraph {
  return { nodes, edges, root_node_id: nodes.length ? "start" : null };
}
function rg(anon: CanvasGraph): RuleGraph {
  return { anonymous: anon, registered: empty, customer: empty };
}
const findNode = (cd: CanvasDiff, id: string) =>
  cd.nodes.find((n) => n.id === id)!;

describe("diffRuleGraph", () => {
  it("treats position-only changes as unchanged", () => {
    const a = rg(canvas([start(), decision("d1"), end()], []));
    const b = rg(
      canvas([start(pos(99, 99)), decision("d1", undefined, pos(50, 999)), end()], []),
    );
    const d = diffRuleGraph(a, b).anonymous;
    expect(d.changeCount).toBe(0);
    expect(findNode(d, "d1").status).toBe("unchanged");
    expect(findNode(d, "start").status).toBe("unchanged");
  });

  it("identical graphs produce zero changes on every canvas", () => {
    const a = rg(canvas([start(), decision("d1"), end()], [edge("e1", "start", "d1", "yes")]));
    const diff = diffRuleGraph(a, a);
    expect(diff.anonymous.changeCount).toBe(0);
    expect(diff.registered.changeCount).toBe(0);
    expect(diff.customer.changeCount).toBe(0);
  });

  it("flags an added node (only in new)", () => {
    const a = rg(canvas([start(), end()], []));
    const b = rg(canvas([start(), decision("d2"), end()], []));
    const d = diffRuleGraph(a, b).anonymous;
    expect(findNode(d, "d2").status).toBe("added");
    expect(d.changeCount).toBe(1);
  });

  it("flags a removed node (only in old) and renders the old copy", () => {
    const a = rg(canvas([start(), decision("d2", undefined, pos(7, 7)), end()], []));
    const b = rg(canvas([start(), end()], []));
    const d = diffRuleGraph(a, b).anonymous;
    const removed = findNode(d, "d2");
    expect(removed.status).toBe("removed");
    expect(removed.node.position).toEqual(pos(7, 7));
  });

  it("flags a modified node with the changed field + old/new values", () => {
    const a = rg(canvas([decision("d1", { operator: "equals", value: "mobile" })], []));
    const b = rg(canvas([decision("d1", { operator: "equals", value: "desktop" })], []));
    const d = diffRuleGraph(a, b).anonymous;
    const m = findNode(d, "d1");
    expect(m.status).toBe("modified");
    expect(m.changes).toContainEqual({ field: "value", old: "mobile", new: "desktop" });
    expect(m.changes).toHaveLength(1);
  });

  it("flags a processor type change", () => {
    const a = rg(canvas([decision("d1", { operator: "equals" })], []));
    const b = rg(
      canvas([{ kind: "decision", id: "d1", processor: { type: "meta_tags", operator: "equals" }, position: pos() }], []),
    );
    const m = findNode(diffRuleGraph(a, b).anonymous, "d1");
    expect(m.status).toBe("modified");
    expect(m.changes).toContainEqual({ field: "type", old: "device_type", new: "meta_tags" });
  });

  it("flags a custom_label change on an expression (absent → set)", () => {
    const a = rg(canvas([expression("x1")], []));
    const b = rg(canvas([expression("x1", "paywall_block")], []));
    const m = findNode(diffRuleGraph(a, b).anonymous, "x1");
    expect(m.status).toBe("modified");
    expect(m.changes).toContainEqual({ field: "custom_label", old: null, new: "paywall_block" });
  });

  it("treats a whitespace-only custom_label as absent (no change)", () => {
    const a = rg(canvas([expression("x1")], []));
    const b = rg(canvas([expression("x1", "   ")], []));
    expect(findNode(diffRuleGraph(a, b).anonymous, "x1").status).toBe("unchanged");
  });

  it("flags added and removed edges by semantic key", () => {
    const a = rg(canvas([start(), decision("d1"), end()], [edge("e1", "start", "d1", "yes")]));
    const b = rg(
      canvas(
        [start(), decision("d1"), end()],
        [edge("e1", "start", "d1", "yes"), edge("e2", "d1", "end", "yes")],
      ),
    );
    const d = diffRuleGraph(a, b).anonymous;
    const added = d.edges.find((e) => e.status === "added")!;
    expect(added.source).toBe("d1");
    expect(added.target).toBe("end");
    expect(d.edges.filter((e) => e.status !== "unchanged")).toHaveLength(1);
  });

  it("treats a branch flip as one removed + one added edge", () => {
    const a = rg(canvas([decision("d1"), end()], [edge("e1", "d1", "end", "yes")]));
    const b = rg(canvas([decision("d1"), end()], [edge("e1", "d1", "end", "no")]));
    const d = diffRuleGraph(a, b).anonymous;
    expect(d.edges.find((e) => e.branch === "yes")?.status).toBe("removed");
    expect(d.edges.find((e) => e.branch === "no")?.status).toBe("added");
  });

  it("two empty canvases yield no changes", () => {
    const diff = diffRuleGraph(rg(empty), rg(empty));
    expect(diff.anonymous.nodes).toHaveLength(0);
    expect(diff.anonymous.changeCount).toBe(0);
  });
});
