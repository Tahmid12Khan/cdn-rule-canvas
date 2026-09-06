"use client";

// Zustand store for the rule-builder canvas (FRONTEND CONTRACT §4). Client-only.
// Owns ONLY canvas working state (the single in-memory canvas, edit mode, dirty
// tracking, per-node validation errors) + ephemeral UI. It does NOT cache
// server entities — it is seeded once from a Query result and persisted back
// via a mutation.
import {
  applyEdgeChanges,
  applyNodeChanges,
  type EdgeChange,
  type Node,
  type NodeChange,
} from "@xyflow/react";
import { create } from "zustand";

import type { RuleGraph } from "@/lib/api/ruleGraph";
import { deserializeRuleGraph } from "@/lib/canvas/deserialize";
import {
  hashRuleGraph,
  serializeRuleGraph,
  type CanvasWorkingState,
} from "@/lib/canvas/serialize";
import {
  END_NODE_ID,
  START_NODE_ID,
  type ProcessorConfig,
  type RFEdge,
  type RFNode,
} from "@/lib/canvas/types";

// The auto-injected start + end nodes (expression-nodes-spec §1/§8). One each
// per non-empty canvas, both non-deletable, both PERSISTED. Injected on
// seed/empty (never via addNode, so they don't flip `dirty` on their own).
function startNode(): RFNode {
  return {
    id: START_NODE_ID,
    type: "startNode",
    position: { x: 260, y: 0 },
    data: { label: "Start" },
    deletable: false,
  };
}

function endNode(): RFNode {
  return {
    id: END_NODE_ID,
    type: "endNode",
    position: { x: 260, y: 480 },
    data: { label: "END" },
    deletable: false,
  };
}

// Every canvas — including one with zero real (non-bookend) nodes — carries
// exactly one start node, one end node, and the connecting edge (spec §6).
// Positions and ids are locked by the spec and must match the backend default.
function emptyCanvas(): CanvasWorkingState {
  const s: RFNode = {
    id: START_NODE_ID,
    type: "startNode",
    position: { x: 40, y: 160 },
    data: { label: "Start" },
    deletable: false,
  };
  const e: RFNode = {
    id: END_NODE_ID,
    type: "endNode",
    position: { x: 940, y: 160 },
    data: { label: "END" },
    deletable: false,
  };
  const edge: RFEdge = {
    id: "e_start_end",
    source: START_NODE_ID,
    target: END_NODE_ID,
    sourceHandle: "out",
    type: "labeledEdge",
    data: { branch: "yes" },
  };
  return { nodes: [s, e], edges: [edge], rootNodeId: START_NODE_ID };
}

// Ensure a deserialized canvas with ≥1 node carries a start + an end node.
// Migrated/legacy graphs are expected to already include them; this is a
// defensive guard so a graph saved before the migration still renders an entry
// + terminal. Pure: returns a NEW working-state; never flips dirty.
function withStartEnd(canvas: CanvasWorkingState): CanvasWorkingState {
  if (canvas.nodes.length === 0) return canvas;
  const hasStart = canvas.nodes.some((n) => n.type === "startNode");
  const hasEnd = canvas.nodes.some((n) => n.type === "endNode");
  if (hasStart && hasEnd) return canvas;
  const nodes = [
    ...(hasStart ? [] : [startNode()]),
    ...canvas.nodes,
    ...(hasEnd ? [] : [endNode()]),
  ];
  return { ...canvas, nodes };
}

// root = the start node (expression-nodes-spec §3). For a non-empty canvas the
// root is the unique `start` node. An empty canvas (no real nodes) has no root.
export function computeRootNodeId(
  nodes: RFNode[],
  _edges: RFEdge[],
): string | null {
  const start = nodes.find((n) => n.type === "startNode");
  if (start) return start.id;
  // Defensive fallback (pre-injection / legacy): the unique node with no
  // incoming edge. Returns null when ambiguous or empty.
  const real = nodes;
  if (real.length === 0) return null;
  const hasIncoming = new Set(_edges.map((e) => e.target));
  const roots = real.filter((n) => !hasIncoming.has(n.id));
  return roots.length === 1 ? roots[0].id : null;
}

// Result of a "Test a rule" run: which canvas nodes/edges were traversed and
// the matched outcome node. Highlighting reads this from the store.
export interface TestHighlight {
  nodeIds: Set<string>;
  edgeIds: Set<string>;
  outcomeNodeId: string | null;
  deadEnd: boolean;
}

export interface RuleBuilderState {
  // --- canvas data (single graph) ---
  canvas: CanvasWorkingState;
  // Memoized always-on journey path (req 1 perf). The two-BFS computation runs
  // ONCE inside each reducer that can change reachability — NOT per node/edge
  // on every selector call. Node and edge renderers read
  // journeyPath.nodeIds/edgeIds.has(id) (a stable boolean), so a drag frame no
  // longer re-runs BFS for every element.
  journeyPath: JourneyPath;
  // --- editor mode / status ---
  isEditing: boolean;
  versionStatus: string;
  // --- dirty tracking & validation ---
  baselineHash: string;
  // O(1) dirty flag — flipped true inside mutation actions and reset on
  // seed/markSaved. Replaces the per-event serialize+hash (WS2 perf fix).
  dirty: boolean;
  nodeErrors: Record<string, string>;
  // --- ephemeral UI ---
  configNodeId: string | null;
  lastSavedAt: number | null;
  // --- test-a-rule highlight ---
  testHighlight: TestHighlight | null;

  // --- seeding ---
  seedFromRuleGraph: (
    rg: RuleGraph,
    status: string,
    outcomeTitleById: (id: string) => string,
    componentNameById?: (id: string) => string | undefined,
  ) => void;

  // --- mode ---
  toggleEdit: () => void;

  // --- canvas mutations ---
  addNode: (node: RFNode) => void;
  removeNode: (nodeId: string) => void;
  updateNodePosition: (nodeId: string, pos: { x: number; y: number }) => void;
  // Bulk position set for auto-layout ("Tidy layout"). Flips dirty (positions
  // are part of the serialized hash). Does NOT clear testHighlight.
  setNodePositions: (positions: Map<string, { x: number; y: number }>) => void;
  // Auto-layout-on-seed variant: applies positions and RE-TAKES the baseline
  // hash so the freshly-laid-out graph is NOT reported as dirty (it is the new
  // "saved" baseline). Used once after seeding a graph whose nodes lack a
  // meaningful layout. Does NOT set lastSavedAt.
  applySeedLayout: (positions: Map<string, { x: number; y: number }>) => void;
  addEdge: (edge: RFEdge) => boolean;
  removeEdge: (edgeId: string) => void;
  onNodesChange: (changes: NodeChange[]) => void;
  onEdgesChange: (changes: EdgeChange[]) => void;

  // --- processor / action config ---
  openNodeConfig: (nodeId: string | null) => void;
  updateNodeProcessor: (nodeId: string, processor: ProcessorConfig) => void;
  // Update an expression node's action config (+ optional resolved outcome
  // title for apply_outcome display, + optional resolved component name for
  // apply_component / apply_component_json display, + optional custom_label —
  // spec §v2.3).
  updateNodeAction: (
    nodeId: string,
    action: ProcessorConfig,
    outcomeTitle?: string,
    customLabel?: string,
    componentName?: string,
  ) => void;

  // --- save support ---
  setNodeErrors: (errs: Record<string, string>) => void;
  clearNodeErrors: () => void;
  markSaved: (rg: RuleGraph) => void;

  // --- test-a-rule highlight ---
  setTestHighlight: (h: TestHighlight) => void;
  clearTestHighlight: () => void;
}

export const useRuleBuilderStore = create<RuleBuilderState>((set, get) => ({
  canvas: emptyCanvas(),
  journeyPath: computeJourneyPath(emptyCanvas()),
  isEditing: false,
  versionStatus: "draft",
  baselineHash: hashRuleGraph(serializeRuleGraph(emptyCanvas())),
  dirty: false,
  nodeErrors: {},
  configNodeId: null,
  lastSavedAt: null,
  testHighlight: null,

  seedFromRuleGraph: (rg, status, outcomeTitleById, componentNameById) => {
    const deserialized = deserializeRuleGraph(
      rg,
      outcomeTitleById,
      componentNameById,
    );
    // Guard the non-empty canvas to carry a start + end node (legacy graphs).
    const canvas = withStartEnd(deserialized);
    set({
      canvas,
      journeyPath: computeJourneyPath(canvas),
      versionStatus: status,
      isEditing: false,
      dirty: false,
      nodeErrors: {},
      configNodeId: null,
      testHighlight: null,
      baselineHash: hashRuleGraph(serializeRuleGraph(canvas)),
      // Reset so the SaveBar's lastSavedAt-driven "Saved" indicator doesn't
      // carry over from a prior version into this freshly-seeded baseline.
      lastSavedAt: null,
    });
  },

  toggleEdit: () => {
    // Edit mode can be entered on ANY version status (W1). For a non-draft
    // version the edits stay purely in-browser — nothing is written to the
    // server until the user does "Save as New Version" (the inline Save button
    // is hidden for non-draft, see SaveBar). versionStatus is still tracked so
    // the banner/gating can distinguish draft vs. local-edit-on-published.
    set((s) => ({ isEditing: !s.isEditing }));
  },

  addNode: (node) =>
    set((state) => {
      const canvas = state.canvas;
      // First real node on an empty canvas auto-injects the start + end pair
      // (one per non-empty canvas — expression-nodes-spec §1/§3).
      const needsBookends = canvas.nodes.length === 0;
      const nodes = needsBookends
        ? [startNode(), node, endNode()]
        : [...canvas.nodes, node];
      const nextCanvas = {
        ...canvas,
        nodes,
        rootNodeId: computeRootNodeId(nodes, canvas.edges),
      };
      return {
        dirty: true,
        testHighlight: null,
        canvas: nextCanvas,
        journeyPath: computeJourneyPath(nextCanvas),
      };
    }),

  removeNode: (nodeId) =>
    set((state) => {
      const canvas = state.canvas;
      // The start + end nodes are non-deletable.
      const target = canvas.nodes.find((n) => n.id === nodeId);
      if (target?.type === "startNode" || target?.type === "endNode") return {};
      let nodes = canvas.nodes.filter((n) => n.id !== nodeId);
      let edges = canvas.edges.filter(
        (e) => e.source !== nodeId && e.target !== nodeId,
      );
      // If that was the last REAL node, reset the canvas to the default
      // start → end state (spec §6: empty canvas always has start + end + edge).
      const realLeft = nodes.filter(
        (n) => n.type !== "startNode" && n.type !== "endNode",
      );
      if (realLeft.length === 0) {
        const def = emptyCanvas();
        nodes = def.nodes;
        edges = def.edges;
      }
      const nextErrors = { ...state.nodeErrors };
      delete nextErrors[nodeId];
      const nextCanvas = {
        nodes,
        edges,
        rootNodeId: computeRootNodeId(nodes, edges),
      };
      return {
        dirty: true,
        testHighlight: null,
        canvas: nextCanvas,
        journeyPath: computeJourneyPath(nextCanvas),
        nodeErrors: nextErrors,
        configNodeId: state.configNodeId === nodeId ? null : state.configNodeId,
      };
    }),

  updateNodePosition: (nodeId, pos) =>
    set((state) => ({
      dirty: true,
      canvas: {
        ...state.canvas,
        nodes: state.canvas.nodes.map((n) =>
          n.id === nodeId ? { ...n, position: pos } : n,
        ),
      },
    })),

  setNodePositions: (positions) =>
    set((state) => ({
      dirty: true,
      canvas: {
        ...state.canvas,
        nodes: state.canvas.nodes.map((n) => {
          const pos = positions.get(n.id);
          return pos ? { ...n, position: pos } : n;
        }),
      },
    })),

  applySeedLayout: (positions) =>
    set((state) => {
      const canvas = {
        ...state.canvas,
        nodes: state.canvas.nodes.map((n) => {
          const pos = positions.get(n.id);
          return pos ? { ...n, position: pos } : n;
        }),
      };
      // Re-take the baseline so the laid-out graph is the new "saved" state
      // (not an unsaved edit). dirty stays false; lastSavedAt is untouched.
      // Positions don't change reachability, but this runs once on seed (not
      // per-frame) so recomputing the journey path keeps it trivially correct.
      return {
        canvas,
        journeyPath: computeJourneyPath(canvas),
        dirty: false,
        baselineHash: hashRuleGraph(serializeRuleGraph(canvas)),
      };
    }),

  addEdge: (edge) => {
    const canvas = get().canvas;
    const sourceNode = canvas.nodes.find((n) => n.id === edge.source);
    // End nodes are terminals — reject outgoing edges (end_terminal /
    // edge_source_kind, expression-nodes-spec §3).
    if (!sourceNode || sourceNode.type === "endNode") return false;
    // Soft branch_unique guard: drop any existing edge from the same source on
    // the same branch (the backend enforces hard validation).
    const branch = edge.sourceHandle ?? edge.data?.branch ?? "yes";
    const edges = canvas.edges
      .filter(
        (e) =>
          !(
            e.source === edge.source &&
            (e.sourceHandle ?? e.data?.branch) === branch
          ),
      )
      .concat(edge);
    set((state) => {
      const nextCanvas = {
        ...state.canvas,
        edges,
        rootNodeId: computeRootNodeId(state.canvas.nodes, edges),
      };
      return {
        dirty: true,
        testHighlight: null,
        canvas: nextCanvas,
        journeyPath: computeJourneyPath(nextCanvas),
      };
    });
    return true;
  },

  removeEdge: (edgeId) =>
    set((state) => {
      const canvas = state.canvas;
      const edges = canvas.edges.filter((e) => e.id !== edgeId);
      const nextCanvas = {
        ...canvas,
        edges,
        rootNodeId: computeRootNodeId(canvas.nodes, edges),
      };
      return {
        dirty: true,
        testHighlight: null,
        canvas: nextCanvas,
        journeyPath: computeJourneyPath(nextCanvas),
      };
    }),

  onNodesChange: (changes) =>
    set((state) => {
      const canvas = state.canvas;
      const nodes = applyNodeChanges(
        changes,
        canvas.nodes as Node[],
      ) as RFNode[];
      // Pure selection/measurement ticks (drag-less clicks, dimension probes)
      // are not real edits — don't flip dirty for those (WS2).
      const mutated = changes.some(
        (c) => c.type !== "select" && c.type !== "dimensions",
      );
      const nextCanvas = {
        ...canvas,
        nodes,
        rootNodeId: computeRootNodeId(nodes, canvas.edges),
      };
      // Only node add/remove can move journey membership — NOT position/select/
      // dimension ticks. Recompute only then so drag frames stay cheap.
      const structural = changes.some(
        (c) => c.type === "add" || c.type === "remove",
      );
      return {
        dirty: state.dirty || mutated,
        canvas: nextCanvas,
        journeyPath: structural
          ? computeJourneyPath(nextCanvas)
          : state.journeyPath,
      };
    }),

  onEdgesChange: (changes) =>
    set((state) => {
      const canvas = state.canvas;
      const edges = applyEdgeChanges(changes, canvas.edges) as RFEdge[];
      const mutated = changes.some((c) => c.type !== "select");
      const nextCanvas = {
        ...canvas,
        edges,
        rootNodeId: computeRootNodeId(canvas.nodes, edges),
      };
      return {
        dirty: state.dirty || mutated,
        canvas: nextCanvas,
        // Edge add/remove changes reachability; selection ticks don't.
        journeyPath: mutated
          ? computeJourneyPath(nextCanvas)
          : state.journeyPath,
      };
    }),

  openNodeConfig: (nodeId) => set({ configNodeId: nodeId }),

  updateNodeProcessor: (nodeId, processor) =>
    set((state) => ({
      dirty: true,
      testHighlight: null,
      canvas: {
        ...state.canvas,
        nodes: state.canvas.nodes.map((n) =>
          n.id === nodeId && n.type === "decisionNode"
            ? { ...n, data: { ...n.data, processor } }
            : n,
        ),
      },
    })),

  updateNodeAction: (
    nodeId,
    action,
    outcomeTitle,
    customLabel,
    componentName,
  ) =>
    set((state) => {
      // Persist a trimmed custom_label; "" / blank => undefined (no custom name).
      const custom_label = customLabel?.trim() ? customLabel.trim() : undefined;
      return {
        dirty: true,
        testHighlight: null,
        canvas: {
          ...state.canvas,
          nodes: state.canvas.nodes.map((n) =>
            n.id === nodeId && n.type === "expressionNode"
              ? {
                  ...n,
                  data: {
                    ...n.data,
                    action,
                    outcomeTitle,
                    componentName,
                    custom_label,
                  },
                }
              : n,
          ),
        },
      };
    }),

  setNodeErrors: (errs) => set({ nodeErrors: errs }),
  clearNodeErrors: () => set({ nodeErrors: {} }),

  markSaved: (rg) =>
    set({
      baselineHash: hashRuleGraph(rg),
      lastSavedAt: Date.now(),
      dirty: false,
      nodeErrors: {},
    }),

  setTestHighlight: (h) => set({ testHighlight: h }),
  clearTestHighlight: () => set({ testHighlight: null }),
}));

// --- derived selectors (NOT stored) ---

// isDirty = current serialized graph hash !== last-saved baseline hash.
export function isDirty(state: RuleBuilderState): boolean {
  const current = hashRuleGraph(serializeRuleGraph(state.canvas));
  return current !== state.baselineHash;
}

export function selectCurrentRuleGraph(state: RuleBuilderState): RuleGraph {
  return serializeRuleGraph(state.canvas);
}

// --- always-on journey path (expression-nodes-spec §3 / req 1) ---
//
// The set of node + edge ids on ANY START -> END path: nodes that are
// forward-reachable from the start node AND can reach an `end` node, plus the
// edges whose endpoints are both on that path. Distinct from `testHighlight`
// (which is the per-test trace and renders brighter). The UI styles this as the
// always-on baseline so the user can always see where "Start" leads.
export interface JourneyPath {
  nodeIds: Set<string>;
  edgeIds: Set<string>;
}

export function computeJourneyPath(canvas: CanvasWorkingState): JourneyPath {
  const nodeIds = new Set<string>();
  const edgeIds = new Set<string>();

  // Anchor forward-reachability at the start node (the graph root).
  const root = computeRootNodeId(canvas.nodes, canvas.edges);

  const fwd = new Map<string, string[]>();
  for (const e of canvas.edges) {
    const list = fwd.get(e.source);
    if (list) list.push(e.target);
    else fwd.set(e.source, [e.target]);
  }
  const reachable = new Set<string>();
  if (root) {
    reachable.add(root);
    const stackF = [root];
    while (stackF.length > 0) {
      const cur = stackF.pop() as string;
      for (const next of fwd.get(cur) ?? []) {
        if (!reachable.has(next)) {
          reachable.add(next);
          stackF.push(next);
        }
      }
    }
  }

  // can-reach-end: reverse-BFS from end nodes (over ALL edges).
  const rev = new Map<string, string[]>();
  for (const e of canvas.edges) {
    const list = rev.get(e.target);
    if (list) list.push(e.source);
    else rev.set(e.target, [e.source]);
  }
  const endIds = canvas.nodes
    .filter((n) => n.type === "endNode")
    .map((n) => n.id);
  const canReach = new Set<string>(endIds);
  const stackR = [...endIds];
  while (stackR.length > 0) {
    const cur = stackR.pop() as string;
    for (const prev of rev.get(cur) ?? []) {
      if (!canReach.has(prev)) {
        canReach.add(prev);
        stackR.push(prev);
      }
    }
  }

  // Journey nodes: reachable-from-start AND can-reach-end.
  for (const id of reachable) {
    if (canReach.has(id)) nodeIds.add(id);
  }

  // Journey edges: both endpoints on the journey.
  for (const e of canvas.edges) {
    if (nodeIds.has(e.source) && nodeIds.has(e.target)) edgeIds.add(e.id);
  }

  return { nodeIds, edgeIds };
}

// Selector form for the canvas. Returns the MEMOIZED field (recomputed inside
// the structural reducers) — it does NOT re-run the two BFS per call, so
// node/edge renderers can read it on every render frame cheaply.
export function selectJourneyPath(state: RuleBuilderState): JourneyPath {
  return state.journeyPath;
}
