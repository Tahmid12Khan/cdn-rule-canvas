// Maps backend 422 validation details (BACKEND CONTRACT §1/§6) to per-node
// errors keyed by React Flow node id (Task 14). The backend `loc` is
// index-based against the EXACT serialized payload that was sent, e.g.
// `rule_graph.anonymous.edges[3]` / `rule_graph.anonymous.nodes[1]`. We resolve
// the index against the same RuleGraph we just serialized, then attribute the
// message to the node (an edge maps to its source node).
import type { ApiErrorDetail } from "@/lib/api/client";
import type { RuleGraph } from "@/lib/api/ruleGraph";
import type { CanvasWorkingState } from "@/lib/canvas/serialize";
import type { CanvasKey, RFNode } from "@/lib/canvas/types";
import type { UserError } from "@/lib/errors/userError";

const LOC_RE =
  /^rule_graph\.(anonymous|registered|customer)\.(nodes|edges)\[(\d+)\]/;

const CANVAS_LABEL: Record<CanvasKey, string> = {
  anonymous: "Anonymous",
  registered: "Registered",
  customer: "Customer",
};

export function mapValidationErrors(
  details: ApiErrorDetail[] | undefined,
  sentGraph: RuleGraph,
): Record<string, string> {
  const errors: Record<string, string> = {};
  if (!details) return errors;

  for (const detail of details) {
    const match = LOC_RE.exec(detail.loc);
    if (!match) continue;
    const canvas = match[1] as keyof RuleGraph;
    const collection = match[2] as "nodes" | "edges";
    const idx = Number(match[3]);
    const graph = sentGraph[canvas];

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

// Mirrors the on-node display labels (DecisionNode.processorTitle /
// OutcomeNode title) so the banner names a node the same way the canvas does.
function nodeLabel(node: RFNode | undefined): string {
  if (!node) return "a node";
  if (node.type === "decisionNode") {
    const p = node.data.processor;
    if (p.type === "meta_tags") return p.tag_name?.trim() ? p.tag_name : "Meta Tags";
    return "Device";
  }
  if (node.type === "outcomeNode") {
    return node.data.title?.trim() ? node.data.title : "Outcome";
  }
  return "a node";
}

// Resolve which canvas a node id lives on + its RF node, from the store
// canvases (the sent RuleGraph carries ids/processor only, not display titles).
function findNode(
  canvases: Record<CanvasKey, CanvasWorkingState>,
  nodeId: string,
): { canvas: CanvasKey; node: RFNode } | null {
  for (const key of Object.keys(canvases) as CanvasKey[]) {
    const node = canvases[key].nodes.find((n) => n.id === nodeId);
    if (node) return { canvas: key, node };
  }
  return null;
}

// Derive a short human reason from the backend rule_id / msg.
function reasonFor(detail: ApiErrorDetail): string {
  switch (detail.rule_id) {
    case "outcome_ref_exists":
      return "reference outcomes that don't exist in this version";
    case "no_cycles":
      return "form a cycle (a rule can't loop back on itself)";
    case "branch_unique":
      return "have duplicate branches from the same node";
    case "outcome_terminal":
    case "outcome_branch_forbidden":
      return "have outgoing connections from a terminal outcome";
    default:
      // Fall back to the backend message, lower-cased to read as a clause.
      return detail.msg ? detail.msg.replace(/\.$/, "") : "are invalid";
  }
}

/**
 * Build a DESCRIPTIVE UserError from a 422 payload: names the offending nodes
 * by their on-canvas label, groups them by canvas, and explains how to fix.
 * Falls back to a generic message when no node loc resolves.
 */
export function buildValidationUserError(
  details: ApiErrorDetail[] | undefined,
  sentGraph: RuleGraph,
  canvases: Record<CanvasKey, CanvasWorkingState>,
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

  // Group node labels + reasons by canvas, preserving the dominant reason per
  // canvas (first resolved). msg is keyed by rule_id via reasonFor.
  const detailById = new Map<string, ApiErrorDetail>();
  if (details) {
    for (const d of details) {
      const match = LOC_RE.exec(d.loc);
      if (!match) continue;
      const canvas = match[1] as keyof RuleGraph;
      const collection = match[2] as "nodes" | "edges";
      const idx = Number(match[3]);
      const id =
        collection === "nodes"
          ? sentGraph[canvas].nodes[idx]?.id
          : sentGraph[canvas].edges[idx]?.source_node_id;
      if (id && !detailById.has(id)) detailById.set(id, d);
    }
  }

  const byCanvas = new Map<CanvasKey, { labels: string[]; reason: string }>();
  for (const id of nodeIds) {
    const found = findNode(canvases, id);
    const canvas = found?.canvas ?? "anonymous";
    const label = nodeLabel(found?.node);
    const detail = detailById.get(id);
    const reason = detail ? reasonFor(detail) : "are invalid";
    const entry = byCanvas.get(canvas);
    if (entry) {
      if (!entry.labels.includes(label)) entry.labels.push(label);
    } else {
      byCanvas.set(canvas, { labels: [label], reason });
    }
  }

  const sentences = [...byCanvas.entries()].map(([canvas, { labels, reason }]) => {
    const quoted = labels.map((l) => `'${l}'`).join(", ");
    return `On the ${CANVAS_LABEL[canvas]} canvas: ${quoted} ${reason}.`;
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
