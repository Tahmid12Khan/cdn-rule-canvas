// Client-side pre-flight graph validation (expression-nodes-spec §3). Mirrors
// the SERVER rules so cycles / dead-ends / missing start+end are caught before
// the save round-trip; the server stays authoritative on 422. Pure: no React,
// no store import.
//
// Operates on a canvas WORKING STATE (the same RFNode/RFEdge shape the store
// holds). Start / End are now REAL persisted nodes (not stripped), so they are
// part of every rule: root = the start node; reachability = "can reach an end".
import type { CanvasWorkingState } from "@/lib/canvas/serialize";
import type { RFEdge, RFNode } from "@/lib/canvas/types";
import type { UserError } from "@/lib/errors/userError";

// Message copy kept byte-aligned with the SERVER (validationMapping.reasonFor):
// the substrings "form a cycle" / "reach an end" must match so the client and
// the 422 path read consistently.
export const CYCLE_MESSAGE = "is part of a cycle (rules can't form a cycle)";
export const UNREACHABLE_END_MESSAGE =
  "cannot reach an end (every branch must end at an END node)";
export const MISSING_START_MESSAGE = "missing start node";
export const MISSING_END_MESSAGE = "missing end node";

// --- helpers ---------------------------------------------------------------

// root = the start node (expression-nodes-spec §3). Null when there is no start.
export function computeRoot(nodes: RFNode[], _edges: RFEdge[]): string | null {
  const start = nodes.find((n) => n.type === "startNode");
  if (start) return start.id;
  if (nodes.length === 0) return null;
  const hasIncoming = new Set(_edges.map((e) => e.target));
  const roots = nodes.filter((n) => !hasIncoming.has(n.id));
  return roots.length === 1 ? roots[0].id : null;
}

// Adjacency (forward).
function buildAdjacency(edges: RFEdge[]): Map<string, string[]> {
  const adj = new Map<string, string[]>();
  for (const e of edges) {
    const list = adj.get(e.source);
    if (list) list.push(e.target);
    else adj.set(e.source, [e.target]);
  }
  return adj;
}

// --- no_cycles -------------------------------------------------------------

// Three-colour DFS. Returns the set of node ids that lie on a back-edge cycle,
// or null when the graph is acyclic. (Mirrors backend no_cycles.)
export function findCycle(
  nodes: RFNode[],
  edges: RFEdge[],
): Set<string> | null {
  const adj = buildAdjacency(edges);
  const ids = nodes.map((n) => n.id);

  const WHITE = 0;
  const GREY = 1;
  const BLACK = 2;
  const colour = new Map<string, number>(ids.map((id) => [id, WHITE]));
  const onCycle = new Set<string>();

  // Iterative DFS keeping the grey stack so we can collect the cycle members.
  function visit(start: string): boolean {
    const stack: { id: string; iter: number }[] = [{ id: start, iter: 0 }];
    colour.set(start, GREY);
    while (stack.length > 0) {
      const frame = stack[stack.length - 1];
      const neighbours = adj.get(frame.id) ?? [];
      if (frame.iter < neighbours.length) {
        const next = neighbours[frame.iter];
        frame.iter += 1;
        const c = colour.get(next);
        if (c === GREY) {
          // Back edge -> cycle. Collect every grey node currently on the stack
          // from `next` to the top (the cycle members).
          let found = false;
          for (const f of stack) {
            if (f.id === next) found = true;
            if (found) onCycle.add(f.id);
          }
          return true;
        }
        if (c === WHITE || c === undefined) {
          colour.set(next, GREY);
          stack.push({ id: next, iter: 0 });
        }
      } else {
        colour.set(frame.id, BLACK);
        stack.pop();
      }
    }
    return false;
  }

  let cyclic = false;
  for (const id of ids) {
    if (colour.get(id) === WHITE) {
      if (visit(id)) cyclic = true;
    }
  }
  return cyclic && onCycle.size > 0 ? onCycle : null;
}

// --- all_paths_reach_end ---------------------------------------------------

// Replicates backend all_paths_reach_end: every node reachable from the start
// must be able to reach at least one `end` node. Returns the offending node
// ids. Skips (returns []) when there is no root or no nodes.
export function findUnreachableEndNodes(
  nodes: RFNode[],
  edges: RFEdge[],
  rootNodeId?: string | null,
): string[] {
  if (nodes.length === 0) return [];

  const root = rootNodeId ?? computeRoot(nodes, edges);
  if (!root) return [];

  const fwd = buildAdjacency(edges);

  // Reverse adjacency for the reverse-BFS from end nodes.
  const rev = new Map<string, string[]>();
  for (const e of edges) {
    const list = rev.get(e.target);
    if (list) list.push(e.source);
    else rev.set(e.target, [e.source]);
  }

  const endIds = nodes.filter((n) => n.type === "endNode").map((n) => n.id);

  // can_reach_end = reverse-BFS from all end nodes.
  const canReachEnd = new Set<string>();
  const stackR = [...endIds];
  for (const id of endIds) canReachEnd.add(id);
  while (stackR.length > 0) {
    const cur = stackR.pop() as string;
    for (const prev of rev.get(cur) ?? []) {
      if (!canReachEnd.has(prev)) {
        canReachEnd.add(prev);
        stackR.push(prev);
      }
    }
  }

  // reachable = forward-BFS from root.
  const reachable = new Set<string>();
  const stackF = [root];
  reachable.add(root);
  while (stackF.length > 0) {
    const cur = stackF.pop() as string;
    for (const next of fwd.get(cur) ?? []) {
      if (!reachable.has(next)) {
        reachable.add(next);
        stackF.push(next);
      }
    }
  }

  return [...reachable].filter((id) => !canReachEnd.has(id));
}

// --- combine ---------------------------------------------------------------

export interface CanvasValidation {
  nodeErrors: Record<string, string>;
  problems: string[];
}

// Validate ONE canvas. Combines start_present / end_present + no_cycles +
// all_paths_reach_end into per-node errors plus human-readable problems. An
// empty canvas (zero nodes) is valid (no start/end required).
export function validateCanvasGraph(
  canvas: CanvasWorkingState,
): CanvasValidation {
  const nodeErrors: Record<string, string> = {};
  const problems: string[] = [];

  if (canvas.nodes.length === 0) {
    return { nodeErrors, problems };
  }

  const starts = canvas.nodes.filter((n) => n.type === "startNode");
  const ends = canvas.nodes.filter((n) => n.type === "endNode");
  if (starts.length !== 1) {
    problems.push("the canvas needs exactly one start node");
  }
  if (ends.length === 0) {
    problems.push("the canvas needs at least one end node");
  }

  const cycle = findCycle(canvas.nodes, canvas.edges);
  if (cycle) {
    for (const id of cycle) nodeErrors[id] = CYCLE_MESSAGE;
    problems.push("some rules form a cycle");
  }

  const deadEnds = findUnreachableEndNodes(
    canvas.nodes,
    canvas.edges,
    canvas.rootNodeId,
  );
  for (const id of deadEnds) {
    // Don't overwrite a cycle marker (a node may be both).
    if (!nodeErrors[id]) nodeErrors[id] = UNREACHABLE_END_MESSAGE;
  }
  if (deadEnds.length > 0) {
    problems.push("some rules can't reach an end");
  }

  return { nodeErrors, problems };
}

// Build a DESCRIPTIVE UserError for the SaveBar pre-flight gate (req 2). The
// `problems` sentences come from validateCanvasGraph.
export function buildClientValidationUserError(problems: string[]): UserError {
  return {
    title: "Fix the rule graph before saving",
    why:
      problems.length > 0
        ? problems.join(" ")
        : "The rule graph has unresolved problems.",
    howToFix:
      "Resolve the highlighted nodes (remove cycles, connect every branch to an END node), then save again.",
    retryable: false,
  };
}
