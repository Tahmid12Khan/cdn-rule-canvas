// Client-side pre-flight graph validation (spec §4a). Mirrors the SERVER rules
// (backend §2.1) so cycles / dead-ends are caught before the save round-trip;
// the server stays authoritative on 422. Pure: no React, no store import.
//
// Operates on a canvas WORKING STATE (the same RFNode/RFEdge shape the store
// holds). The frontend-only start node and its edge are excluded from every
// rule (root detection, cycle detection, reachability) so they never affect the
// real-graph semantics — exactly as serialize.ts strips them on the wire.
import type { CanvasWorkingState } from "@/lib/canvas/serialize";
import { START_NODE_ID, type CanvasKey, type RFEdge, type RFNode } from "@/lib/canvas/types";
import type { UserError } from "@/lib/errors/userError";

const CANVAS_LABEL: Record<CanvasKey, string> = {
  anonymous: "Anonymous",
  registered: "Registered",
  customer: "Customer",
};

// Message copy kept byte-aligned with the SERVER (validationMapping.reasonFor):
// the substrings "form a cycle" / "cannot reach an outcome" must match so the
// client and the 422 path read consistently.
export const CYCLE_MESSAGE = "is part of a cycle (rules can't form a cycle)";
export const UNREACHABLE_OUTCOME_MESSAGE =
  "cannot reach an outcome (every branch must end at an outcome)";

// --- helpers ---------------------------------------------------------------

// Real (non-start) nodes only.
function realNodes(nodes: RFNode[]): RFNode[] {
  return nodes.filter((n) => n.type !== "startNode");
}

// Real edges only: drop any edge touching the start node (mirrors serialize).
function realEdges(edges: RFEdge[]): RFEdge[] {
  return edges.filter(
    (e) => e.source !== START_NODE_ID && e.target !== START_NODE_ID,
  );
}

// root = the unique real node with no incoming real edge. Mirrors the store's
// computeRootNodeId (and backend §2.1 step 1). Null when ambiguous/empty.
export function computeRoot(nodes: RFNode[], edges: RFEdge[]): string | null {
  const real = realNodes(nodes);
  if (real.length === 0) return null;
  const hasIncoming = new Set(realEdges(edges).map((e) => e.target));
  const roots = real.filter((n) => !hasIncoming.has(n.id));
  return roots.length === 1 ? roots[0].id : null;
}

// Adjacency (forward) over real edges.
function buildAdjacency(edges: RFEdge[]): Map<string, string[]> {
  const adj = new Map<string, string[]>();
  for (const e of realEdges(edges)) {
    const list = adj.get(e.source);
    if (list) list.push(e.target);
    else adj.set(e.source, [e.target]);
  }
  return adj;
}

// --- no_cycles -------------------------------------------------------------

// Three-colour DFS over real edges. Returns the set of node ids that lie on a
// back-edge cycle, or null when the graph is acyclic. (Mirrors backend
// no_cycles.)
export function findCycle(
  nodes: RFNode[],
  edges: RFEdge[],
): Set<string> | null {
  const adj = buildAdjacency(edges);
  const ids = realNodes(nodes).map((n) => n.id);

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

// --- outcome_reachable -----------------------------------------------------

// Replicates backend §2.1: every node reachable from the root must be able to
// reach at least one outcome node. Returns the offending node ids. Skips
// (returns []) when there is no single root or no real nodes.
export function findUnreachableOutcomeNodes(
  nodes: RFNode[],
  edges: RFEdge[],
  rootNodeId?: string | null,
): string[] {
  const real = realNodes(nodes);
  if (real.length === 0) return [];

  const root = rootNodeId ?? computeRoot(nodes, edges);
  if (!root) return [];

  const fwd = buildAdjacency(edges);

  // Reverse adjacency for the reverse-BFS from outcomes.
  const rev = new Map<string, string[]>();
  for (const e of realEdges(edges)) {
    const list = rev.get(e.target);
    if (list) list.push(e.source);
    else rev.set(e.target, [e.source]);
  }

  const outcomeIds = real
    .filter((n) => n.type === "outcomeNode")
    .map((n) => n.id);

  // can_reach_outcome = reverse-BFS from all outcome nodes.
  const canReachOutcome = new Set<string>();
  const stackR = [...outcomeIds];
  for (const id of outcomeIds) canReachOutcome.add(id);
  while (stackR.length > 0) {
    const cur = stackR.pop() as string;
    for (const prev of rev.get(cur) ?? []) {
      if (!canReachOutcome.has(prev)) {
        canReachOutcome.add(prev);
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

  return [...reachable].filter((id) => !canReachOutcome.has(id));
}

// --- combine ---------------------------------------------------------------

export interface CanvasValidation {
  nodeErrors: Record<string, string>;
  problems: string[];
}

// Validate ONE canvas. Combines no_cycles + outcome_reachable into per-node
// errors (keyed by node id) plus human-readable problem sentences.
export function validateCanvasGraph(
  canvas: CanvasWorkingState,
): CanvasValidation {
  const nodeErrors: Record<string, string> = {};
  const problems: string[] = [];

  const cycle = findCycle(canvas.nodes, canvas.edges);
  if (cycle) {
    for (const id of cycle) nodeErrors[id] = CYCLE_MESSAGE;
    problems.push("some rules form a cycle");
  }

  const deadEnds = findUnreachableOutcomeNodes(
    canvas.nodes,
    canvas.edges,
    canvas.rootNodeId,
  );
  for (const id of deadEnds) {
    // Don't overwrite a cycle marker (a node may be both).
    if (!nodeErrors[id]) nodeErrors[id] = UNREACHABLE_OUTCOME_MESSAGE;
  }
  if (deadEnds.length > 0) {
    problems.push("some rules can't reach an outcome");
  }

  return { nodeErrors, problems };
}

// Validate all three canvases. Aggregates node errors across canvases (ids are
// unique per canvas, so a flat merge is safe) and prefixes each problem with
// its canvas label so the SaveBar banner can name the offending canvas.
export function validateAllCanvases(
  canvases: Record<CanvasKey, CanvasWorkingState>,
): CanvasValidation {
  const nodeErrors: Record<string, string> = {};
  const problems: string[] = [];

  for (const key of Object.keys(canvases) as CanvasKey[]) {
    const result = validateCanvasGraph(canvases[key]);
    Object.assign(nodeErrors, result.nodeErrors);
    for (const p of result.problems) {
      problems.push(`On the ${CANVAS_LABEL[key]} canvas, ${p}.`);
    }
  }

  return { nodeErrors, problems };
}

// Build a DESCRIPTIVE UserError for the SaveBar pre-flight gate (req 2). The
// `problems` sentences come from validateAllCanvases.
export function buildClientValidationUserError(problems: string[]): UserError {
  return {
    title: "Fix the rule graph before saving",
    why:
      problems.length > 0
        ? problems.join(" ")
        : "The rule graph has unresolved problems.",
    howToFix:
      "Resolve the highlighted nodes (remove cycles, connect every branch to an outcome), then save again.",
    retryable: false,
  };
}
