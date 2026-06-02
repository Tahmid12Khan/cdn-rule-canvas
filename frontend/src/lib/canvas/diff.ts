// Pure rule-graph diff (version-diff-compare spec §4). Compares two RuleGraphs
// and reports, per canvas, which nodes were added / removed / modified and which
// edges were added / removed. Node x/y `position` is NEVER a change. No React, no
// manifest, no store — trivially unit-testable.
import type {
  Branch,
  CanvasGraph,
  Edge,
  GraphNode,
  RuleGraph,
} from "@/lib/api/ruleGraph";
import type { ProcessorConfig } from "@/lib/canvas/types";
import type { CanvasKey } from "@/lib/canvas/types";

export type NodeStatus = "added" | "removed" | "modified" | "unchanged";
export type EdgeStatus = "added" | "removed" | "unchanged";

// One changed field on a modified node: the raw config key (e.g. "operator",
// "value", "json_path"), or the literal "type" / "custom_label" / "kind".
export interface FieldChange {
  field: string;
  old: unknown;
  new: unknown;
}

export interface NodeDiff {
  id: string;
  status: NodeStatus;
  // The copy to render: the NEW version's node for added/modified/unchanged, the
  // OLD version's node for removed (so a removed node still has a position).
  node: GraphNode;
  // Present only when status === "modified".
  changes?: FieldChange[];
}

export interface EdgeDiff {
  key: string; // `${source}|${target}|${branch}` — stable across edge-id churn
  status: EdgeStatus;
  source: string;
  target: string;
  branch: Branch;
}

export interface CanvasDiff {
  nodes: NodeDiff[];
  edges: EdgeDiff[];
  changeCount: number; // nodes + edges whose status !== "unchanged"
}

export type RuleGraphDiff = Record<CanvasKey, CanvasDiff>;

// Order-independent deep equality for JSON values (objects, arrays, primitives).
function stableEqual(a: unknown, b: unknown): boolean {
  if (a === b) return true;
  if (a === null || b === null) return a === b;
  if (typeof a !== typeof b) return false;
  if (typeof a !== "object") return false;
  if (Array.isArray(a) || Array.isArray(b)) {
    if (!Array.isArray(a) || !Array.isArray(b) || a.length !== b.length) {
      return false;
    }
    return a.every((v, i) => stableEqual(v, (b as unknown[])[i]));
  }
  const ao = a as Record<string, unknown>;
  const bo = b as Record<string, unknown>;
  const ak = Object.keys(ao);
  if (ak.length !== Object.keys(bo).length) return false;
  return ak.every((k) => k in bo && stableEqual(ao[k], bo[k]));
}

// The processor (decision) or action (expression) config, or undefined for
// start/end nodes which carry no config.
function configOf(n: GraphNode): ProcessorConfig | undefined {
  if (n.kind === "decision") return n.processor;
  if (n.kind === "expression") return n.action;
  return undefined;
}

// Empty/whitespace custom_label === absent (spec §v2.3).
function normLabel(s: string | undefined): string | undefined {
  const t = s?.trim();
  return t ? t : undefined;
}

// Per-field config differences (the `type` discriminator is just another key).
function configChanges(
  o: ProcessorConfig | undefined,
  n: ProcessorConfig | undefined,
): FieldChange[] {
  const keys = new Set<string>([
    ...Object.keys(o ?? {}),
    ...Object.keys(n ?? {}),
  ]);
  const out: FieldChange[] = [];
  for (const k of keys) {
    const ov = o?.[k];
    const nv = n?.[k];
    if (!stableEqual(ov, nv)) {
      out.push({ field: k, old: ov ?? null, new: nv ?? null });
    }
  }
  return out;
}

// All field-level changes between two same-id nodes (position excluded).
function nodeChanges(o: GraphNode, n: GraphNode): FieldChange[] {
  const out: FieldChange[] = [];
  if (o.kind !== n.kind) out.push({ field: "kind", old: o.kind, new: n.kind });
  out.push(...configChanges(configOf(o), configOf(n)));
  const ol = o.kind === "expression" ? normLabel(o.custom_label) : undefined;
  const nl = n.kind === "expression" ? normLabel(n.custom_label) : undefined;
  if (ol !== nl) {
    out.push({ field: "custom_label", old: ol ?? null, new: nl ?? null });
  }
  return out;
}

function edgeKey(e: Edge): string {
  return `${e.source_node_id}|${e.target_node_id}|${e.branch}`;
}

function diffCanvas(o: CanvasGraph, n: CanvasGraph): CanvasDiff {
  const oldById = new Map(o.nodes.map((x) => [x.id, x]));
  const newById = new Map(n.nodes.map((x) => [x.id, x]));

  const nodes: NodeDiff[] = [];
  for (const id of new Set([...oldById.keys(), ...newById.keys()])) {
    const on = oldById.get(id);
    const nn = newById.get(id);
    if (on && nn) {
      const changes = nodeChanges(on, nn);
      nodes.push(
        changes.length === 0
          ? { id, status: "unchanged", node: nn }
          : { id, status: "modified", node: nn, changes },
      );
    } else if (nn) {
      nodes.push({ id, status: "added", node: nn });
    } else if (on) {
      nodes.push({ id, status: "removed", node: on });
    }
  }
  // Deterministic top-to-bottom order for the canvas + change list.
  nodes.sort(
    (a, b) =>
      a.node.position.y - b.node.position.y ||
      a.node.position.x - b.node.position.x,
  );

  const oldE = new Map(o.edges.map((e) => [edgeKey(e), e]));
  const newE = new Map(n.edges.map((e) => [edgeKey(e), e]));
  const edges: EdgeDiff[] = [];
  for (const k of new Set([...oldE.keys(), ...newE.keys()])) {
    const oe = oldE.get(k);
    const ne = newE.get(k);
    const base = (ne ?? oe) as Edge;
    const status: EdgeStatus = oe && ne ? "unchanged" : ne ? "added" : "removed";
    edges.push({
      key: k,
      status,
      source: base.source_node_id,
      target: base.target_node_id,
      branch: base.branch,
    });
  }

  const changeCount =
    nodes.filter((x) => x.status !== "unchanged").length +
    edges.filter((x) => x.status !== "unchanged").length;

  return { nodes, edges, changeCount };
}

export function diffRuleGraph(
  oldRg: RuleGraph,
  newRg: RuleGraph,
): RuleGraphDiff {
  return {
    anonymous: diffCanvas(oldRg.anonymous, newRg.anonymous),
    registered: diffCanvas(oldRg.registered, newRg.registered),
    customer: diffCanvas(oldRg.customer, newRg.customer),
  };
}
