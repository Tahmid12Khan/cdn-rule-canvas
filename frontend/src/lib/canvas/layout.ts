// Dagre top-down auto-layout for the canvas (spec §4a / req 1). Pure: no React.
// Returns NEW positions keyed by node id; the caller (store.setNodePositions)
// applies them. The start node is included so it lands at rank 0, giving a clean
// "start -> ... -> outcome" journey.
//
// IMPORTANT: node sizes are HARDCODED per type (sizeFor) — React Flow's measured
// `node.width/height` are `number | null` until its ResizeObserver runs (null on
// first seed), so relying on them would collapse the ranks. Keep these in sync
// with the redesigned node dimensions (DecisionNode ~h-28 w-28 + title pill).
import dagre from "@dagrejs/dagre";

import type { RFEdge, RFNode } from "@/lib/canvas/types";

export interface XY {
  x: number;
  y: number;
}

export interface AutoLayoutOptions {
  rankdir?: "TB" | "LR";
  ranksep?: number;
  nodesep?: number;
}

// Hardcoded node footprint (px) by RF node type. Width/height include the
// label/title chrome so dagre leaves enough gutter between ranks.
function sizeFor(node: RFNode): { width: number; height: number } {
  switch (node.type) {
    case "decisionNode":
      return { width: 140, height: 150 };
    case "expressionNode":
      return { width: 180, height: 64 };
    case "startNode":
      return { width: 96, height: 40 };
    case "endNode":
      return { width: 96, height: 40 };
    default:
      return { width: 140, height: 60 };
  }
}

// Compute new top-left positions for every node via dagre. Edges define rank
// order; unconnected nodes still get placed. dagre returns CENTER coordinates —
// we convert to React Flow's TOP-LEFT origin (subtract half width/height).
export function autoLayout(
  nodes: RFNode[],
  edges: RFEdge[],
  opts: AutoLayoutOptions = {},
): Map<string, XY> {
  const g = new dagre.graphlib.Graph();
  g.setGraph({
    rankdir: opts.rankdir ?? "TB",
    ranksep: opts.ranksep ?? 80,
    nodesep: opts.nodesep ?? 60,
    marginx: 20,
    marginy: 20,
  });
  g.setDefaultEdgeLabel(() => ({}));

  for (const n of nodes) {
    g.setNode(n.id, sizeFor(n));
  }
  // Only edges whose endpoints both exist as nodes (defensive against stale
  // edges); dagre throws on an edge to an unknown node.
  const ids = new Set(nodes.map((n) => n.id));
  for (const e of edges) {
    if (ids.has(e.source) && ids.has(e.target)) {
      g.setEdge(e.source, e.target);
    }
  }

  dagre.layout(g);

  const positions = new Map<string, XY>();
  for (const n of nodes) {
    const laid = g.node(n.id);
    if (!laid || typeof laid.x !== "number" || typeof laid.y !== "number") {
      continue;
    }
    const size = sizeFor(n);
    positions.set(n.id, {
      x: laid.x - size.width / 2,
      y: laid.y - size.height / 2,
    });
  }
  return positions;
}

// True when a seeded canvas lacks a meaningful layout and should be auto-laid
// out once: two or more REAL (non-start) nodes share the same position, or every
// real node sits at the origin. A well-positioned saved graph returns false so
// its layout is respected (no surprise relayout, no spurious dirty). Pure.
export function needsLayout(nodes: RFNode[]): boolean {
  const real = nodes.filter(
    (n) => n.type !== "startNode" && n.type !== "endNode",
  );
  if (real.length < 2) return false;
  const seen = new Set<string>();
  for (const n of real) {
    const key = `${Math.round(n.position.x)},${Math.round(n.position.y)}`;
    if (seen.has(key)) return true;
    seen.add(key);
  }
  return false;
}
