// Maps backend 422 validation details (BACKEND CONTRACT §1/§6) to per-node
// errors keyed by React Flow node id (Task 14). The backend `loc` is
// index-based against the EXACT serialized payload that was sent, e.g.
// `rule_graph.canvas.edges[3]` / `rule_graph.canvas.nodes[1]`. We resolve
// the index against the same RuleGraph we just serialized, then attribute the
// message to the node (an edge maps to its source node).
import type { ApiErrorDetail } from "@/lib/api/client";
import type { RuleGraph } from "@/lib/api/ruleGraph";
import type { CanvasWorkingState } from "@/lib/canvas/serialize";
import type { RFNode } from "@/lib/canvas/types";
import type { UserError } from "@/lib/errors/userError";

const LOC_RE = /^rule_graph\.canvas\.(nodes|edges)\[(\d+)\]/;

export function mapValidationErrors(
  details: ApiErrorDetail[] | undefined,
  sentGraph: RuleGraph,
): Record<string, string> {
  const errors: Record<string, string> = {};
  if (!details) return errors;

  for (const detail of details) {
    const match = LOC_RE.exec(detail.loc);
    if (!match) continue;
    const collection = match[1] as "nodes" | "edges";
    const idx = Number(match[2]);
    const graph = sentGraph.canvas;

    if (collection === "nodes") {
      const node = graph.nodes[idx];
      if (node) errors[node.id] = detail.msg;
    } else {
      const edge = graph.edges[idx];
      // Attribute edge errors to the source node so the user sees a marker.
      if (edge) errors[edge.source_node_id] = detail.msg;
    }
  }

  return errors;
}

// --- descriptive validation UserError -------------------------------------

// Names a node for the descriptive error banner. The node-type manifest is not
// available in this pure path, so a decision/expression node is named by its
// config `type` (the canonical snake_case kind) — generic, with ZERO per-type
// code.
function nodeLabel(node: RFNode | undefined): string {
  if (!node) return "a node";
  if (node.type === "decisionNode") {
    return node.data.processor.type || "Decision";
  }
  if (node.type === "expressionNode") {
    if (node.data.action.type === "apply_outcome" && node.data.outcomeTitle) {
      return node.data.outcomeTitle;
    }
    return node.data.action.type || "Action";
  }
  if (node.type === "startNode") return "Start";
  if (node.type === "endNode") return "END";
  return "a node";
}

// Resolve a node id to its RF node, from the store canvas (the sent RuleGraph
// carries ids/processor only, not display titles).
function findNode(
  canvas: CanvasWorkingState,
  nodeId: string,
): RFNode | undefined {
  return canvas.nodes.find((n) => n.id === nodeId);
}

// Derive a short human reason from the backend rule_id / msg
// (expression-nodes-spec §3 rule ids).
function reasonFor(detail: ApiErrorDetail): string {
  switch (detail.rule_id) {
    case "apply_outcome_ref_exists":
      return "reference outcomes that don't exist in this version";
    case "no_cycles":
      return "form a cycle (a rule can't loop back on itself)";
    case "all_paths_reach_end":
      return "have no path to an END (every branch must end at an END node)";
    case "branch_unique":
      return "have duplicate branches from the same node";
    case "end_terminal":
    case "edge_source_kind":
      return "have outgoing connections from a terminal END node";
    case "start_present":
      return "need exactly one start node";
    case "end_present":
      return "need at least one END node";
    case "start_no_incoming":
      return "have a connection into the start node";
    case "start_single_out":
    case "expression_single_out":
      return "must have exactly one outgoing connection";
    default:
      // Fall back to the backend message, lower-cased to read as a clause.
      return detail.msg ? detail.msg.replace(/\.$/, "") : "are invalid";
  }
}

/**
 * Build a DESCRIPTIVE UserError from a 422 payload: names the offending nodes
 * by their on-canvas label, grouped by reason, and explains how to fix. Falls
 * back to a generic message when no node loc resolves.
 */
export function buildValidationUserError(
  details: ApiErrorDetail[] | undefined,
  sentGraph: RuleGraph,
  canvas: CanvasWorkingState,
): UserError {
  const nodeErrors = mapValidationErrors(details, sentGraph);
  const nodeIds = Object.keys(nodeErrors);

  if (nodeIds.length === 0) {
    return {
      title: "Some rule nodes are invalid",
      why: "The server rejected the graph, but couldn't pinpoint which nodes.",
      howToFix:
        "Review your rule nodes' configuration and connections, then save again.",
      retryable: false,
    };
  }

  // Preserve the dominant reason per node (first resolved). msg is keyed by
  // rule_id via reasonFor.
  const detailById = new Map<string, ApiErrorDetail>();
  if (details) {
    for (const d of details) {
      const match = LOC_RE.exec(d.loc);
      if (!match) continue;
      const collection = match[1] as "nodes" | "edges";
      const idx = Number(match[2]);
      const id =
        collection === "nodes"
          ? sentGraph.canvas.nodes[idx]?.id
          : sentGraph.canvas.edges[idx]?.source_node_id;
      if (id && !detailById.has(id)) detailById.set(id, d);
    }
  }

  // Group node labels by reason.
  const byReason = new Map<string, string[]>();
  for (const id of nodeIds) {
    const node = findNode(canvas, id);
    const label = nodeLabel(node);
    const detail = detailById.get(id);
    const reason = detail ? reasonFor(detail) : "are invalid";
    const labels = byReason.get(reason);
    if (labels) {
      if (!labels.includes(label)) labels.push(label);
    } else {
      byReason.set(reason, [label]);
    }
  }

  const sentences = [...byReason.entries()].map(([reason, labels]) => {
    const quoted = labels.map((l) => `'${l}'`).join(", ");
    return `${quoted} ${reason}.`;
  });

  return {
    title:
      nodeIds.length === 1
        ? "1 rule node needs attention"
        : `${nodeIds.length} rule nodes need attention`,
    why: sentences.join(" "),
    howToFix:
      "Open each highlighted node and fix its configuration (e.g. pick a valid outcome, or re-add a missing one), then save again.",
    retryable: false,
  };
}
