"use client";

// Zustand store for the rule-builder canvas (FRONTEND CONTRACT §4). Client-only.
// Owns ONLY canvas working state (the three in-memory canvases, selection, edit
// mode, dirty tracking, per-node validation errors) + ephemeral UI. It does NOT
// cache server entities — it is seeded once from a Query result and persisted
// back via a mutation.
import {
  applyEdgeChanges,
  applyNodeChanges,
  type EdgeChange,
  type Node,
  type NodeChange,
} from "reactflow";
import { create } from "zustand";

import type { RuleGraph } from "@/lib/api/ruleGraph";
import { deserializeRuleGraph } from "@/lib/canvas/deserialize";
import {
  hashRuleGraph,
  serializeRuleGraph,
  type CanvasWorkingState,
} from "@/lib/canvas/serialize";
import {
  START_NODE_ID,
  type CanvasKey,
  type ProcessorConfig,
  type RFEdge,
  type RFNode,
} from "@/lib/canvas/types";

const CANVAS_KEYS: CanvasKey[] = ["anonymous", "registered", "customer"];

// The single frontend-only start node (Task C). Injected on empty/seed only
// (never via addNode, so it doesn't flip `dirty`), non-deletable, stripped on
// serialize. Positioned top-center as the visual entry marker.
function startNode(): RFNode {
  return {
    id: START_NODE_ID,
    type: "startNode",
    position: { x: 260, y: 0 },
    data: { label: "Start" },
    deletable: false,
  };
}

function startEdge(rootNodeId: string): RFEdge {
  return {
    id: "edge_start",
    source: START_NODE_ID,
    target: rootNodeId,
    sourceHandle: "yes",
    type: "labeledEdge",
    data: { branch: "yes" },
    deletable: false,
  } as RFEdge;
}

function emptyCanvas(): CanvasWorkingState {
  return { nodes: [startNode()], edges: [], rootNodeId: null };
}

// Ensure a canvas (just deserialized from a RuleGraph) has exactly one start
// node. If a computed root exists, also wire start -> root so Start is the
// visual entry. Pure: returns a NEW working-state; never flips dirty.
function withStartNode(canvas: CanvasWorkingState): CanvasWorkingState {
  if (canvas.nodes.some((n) => n.type === "startNode")) return canvas;
  const root = computeRootNodeId(canvas.nodes, canvas.edges);
  const edges = root ? [...canvas.edges, startEdge(root)] : canvas.edges;
  return {
    ...canvas,
    nodes: [startNode(), ...canvas.nodes],
    edges,
  };
}

function emptyCanvases(): Record<CanvasKey, CanvasWorkingState> {
  return {
    anonymous: emptyCanvas(),
    registered: emptyCanvas(),
    customer: emptyCanvas(),
  };
}

// root = the (decision/outcome) node with no incoming edges. The frontend-only
// start node and its outgoing edge are excluded so root detection among the
// real nodes is UNCHANGED. If ambiguous/empty -> null.
export function computeRootNodeId(
  nodes: RFNode[],
  edges: RFEdge[],
): string | null {
  const realNodes = nodes.filter((n) => n.type !== "startNode");
  if (realNodes.length === 0) return null;
  const hasIncoming = new Set(
    edges.filter((e) => e.source !== START_NODE_ID).map((e) => e.target),
  );
  const roots = realNodes.filter((n) => !hasIncoming.has(n.id));
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
  // --- canvas data (3 independent graphs) ---
  canvases: Record<CanvasKey, CanvasWorkingState>;
  selected: CanvasKey;
  // Memoized always-on journey path for the SELECTED canvas (req 1 perf). The
  // two-BFS computation runs ONCE inside each reducer that can change
  // reachability or selection — NOT per node/edge on every selector call. Node
  // and edge renderers read journeyPath.nodeIds/edgeIds.has(id) (a stable
  // boolean), so a drag frame no longer re-runs BFS for every element.
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
  ) => void;

  // --- selection / mode ---
  setSelected: (k: CanvasKey) => void;
  toggleEdit: () => void;

  // --- canvas mutations ---
  addNode: (k: CanvasKey, node: RFNode) => void;
  removeNode: (k: CanvasKey, nodeId: string) => void;
  updateNodePosition: (
    k: CanvasKey,
    nodeId: string,
    pos: { x: number; y: number },
  ) => void;
  // Bulk position set for auto-layout ("Tidy layout"). Flips dirty (positions
  // are part of the serialized hash). Does NOT clear testHighlight.
  setNodePositions: (
    k: CanvasKey,
    positions: Map<string, { x: number; y: number }>,
  ) => void;
  // Auto-layout-on-seed variant: applies positions across ALL canvases and
  // RE-TAKES the baseline hash so the freshly-laid-out graph is NOT reported as
  // dirty (it is the new "saved" baseline). Used once after seeding a graph
  // whose nodes lack a meaningful layout. Does NOT set lastSavedAt.
  applySeedLayout: (
    positionsByCanvas: Record<
      CanvasKey,
      Map<
        string,
        {
          x: number;
          y: number;
        }
      >
    >,
  ) => void;
  addEdge: (k: CanvasKey, edge: RFEdge) => boolean;
  removeEdge: (k: CanvasKey, edgeId: string) => void;
  onNodesChange: (k: CanvasKey, changes: NodeChange[]) => void;
  onEdgesChange: (k: CanvasKey, changes: EdgeChange[]) => void;

  // --- processor config ---
  openNodeConfig: (nodeId: string | null) => void;
  updateNodeProcessor: (
    k: CanvasKey,
    nodeId: string,
    processor: ProcessorConfig,
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
  canvases: emptyCanvases(),
  selected: "anonymous",
  journeyPath: computeJourneyPath(emptyCanvas()),
  isEditing: false,
  versionStatus: "draft",
  baselineHash: hashRuleGraph(serializeRuleGraph(emptyCanvases())),
  dirty: false,
  nodeErrors: {},
  configNodeId: null,
  lastSavedAt: null,
  testHighlight: null,

  seedFromRuleGraph: (rg, status, outcomeTitleById) => {
    const deserialized = deserializeRuleGraph(rg, outcomeTitleById);
    // Inject the frontend-only start node (+ start->root edge) into each canvas.
    const canvases = {
      anonymous: withStartNode(deserialized.anonymous),
      registered: withStartNode(deserialized.registered),
      customer: withStartNode(deserialized.customer),
    };
    set({
      canvases,
      journeyPath: computeJourneyPath(canvases[get().selected]),
      versionStatus: status,
      isEditing: false,
      dirty: false,
      nodeErrors: {},
      configNodeId: null,
      testHighlight: null,
      baselineHash: hashRuleGraph(serializeRuleGraph(canvases)),
      // Reset so the SaveBar's lastSavedAt-driven "Saved" indicator doesn't
      // carry over from a prior version into this freshly-seeded baseline.
      lastSavedAt: null,
    });
  },

  setSelected: (k) =>
    set((state) => ({
      selected: k,
      journeyPath: computeJourneyPath(state.canvases[k]),
    })),

  toggleEdit: () => {
    // Edit mode can be entered on ANY version status (W1). For a non-draft
    // version the edits stay purely in-browser — nothing is written to the
    // server until the user does "Save as New Version" (the inline Save button
    // is hidden for non-draft, see SaveBar). versionStatus is still tracked so
    // the banner/gating can distinguish draft vs. local-edit-on-published.
    set((s) => ({ isEditing: !s.isEditing }));
  },

  addNode: (k, node) =>
    set((state) => {
      const canvas = state.canvases[k];
      const nodes = [...canvas.nodes, node];
      const canvases = {
        ...state.canvases,
        [k]: {
          ...canvas,
          nodes,
          rootNodeId: computeRootNodeId(nodes, canvas.edges),
        },
      };
      return {
        dirty: true,
        testHighlight: null,
        canvases,
        journeyPath: computeJourneyPath(canvases[state.selected]),
      };
    }),

  removeNode: (k, nodeId) =>
    set((state) => {
      const canvas = state.canvases[k];
      // The start node is non-deletable.
      const target = canvas.nodes.find((n) => n.id === nodeId);
      if (target?.type === "startNode") return {};
      const nodes = canvas.nodes.filter((n) => n.id !== nodeId);
      const edges = canvas.edges.filter(
        (e) => e.source !== nodeId && e.target !== nodeId,
      );
      const nextErrors = { ...state.nodeErrors };
      delete nextErrors[nodeId];
      const canvases = {
        ...state.canvases,
        [k]: { nodes, edges, rootNodeId: computeRootNodeId(nodes, edges) },
      };
      return {
        dirty: true,
        testHighlight: null,
        canvases,
        journeyPath: computeJourneyPath(canvases[state.selected]),
        nodeErrors: nextErrors,
        configNodeId: state.configNodeId === nodeId ? null : state.configNodeId,
      };
    }),

  updateNodePosition: (k, nodeId, pos) =>
    set((state) => {
      const canvas = state.canvases[k];
      return {
        dirty: true,
        canvases: {
          ...state.canvases,
          [k]: {
            ...canvas,
            nodes: canvas.nodes.map((n) =>
              n.id === nodeId ? { ...n, position: pos } : n,
            ),
          },
        },
      };
    }),

  setNodePositions: (k, positions) =>
    set((state) => {
      const canvas = state.canvases[k];
      return {
        dirty: true,
        canvases: {
          ...state.canvases,
          [k]: {
            ...canvas,
            nodes: canvas.nodes.map((n) => {
              const pos = positions.get(n.id);
              return pos ? { ...n, position: pos } : n;
            }),
          },
        },
      };
    }),

  applySeedLayout: (positionsByCanvas) =>
    set((state) => {
      const canvases = {} as Record<CanvasKey, CanvasWorkingState>;
      for (const key of CANVAS_KEYS) {
        const canvas = state.canvases[key];
        const positions = positionsByCanvas[key];
        canvases[key] = {
          ...canvas,
          nodes: canvas.nodes.map((n) => {
            const pos = positions?.get(n.id);
            return pos ? { ...n, position: pos } : n;
          }),
        };
      }
      // Re-take the baseline so the laid-out graph is the new "saved" state
      // (not an unsaved edit). dirty stays false; lastSavedAt is untouched.
      // Positions don't change reachability, but this runs once on seed (not
      // per-frame) so recomputing the journey path keeps it trivially correct.
      return {
        canvases,
        journeyPath: computeJourneyPath(canvases[state.selected]),
        dirty: false,
        baselineHash: hashRuleGraph(serializeRuleGraph(canvases)),
      };
    }),

  addEdge: (k, edge) => {
    const canvas = get().canvases[k];
    const sourceNode = canvas.nodes.find((n) => n.id === edge.source);
    // Outcome nodes are terminals — reject outgoing edges (BACKEND
    // outcome_branch_forbidden / outcome_terminal).
    if (!sourceNode || sourceNode.type === "outcomeNode") return false;
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
      const canvases = {
        ...state.canvases,
        [k]: {
          ...state.canvases[k],
          edges,
          rootNodeId: computeRootNodeId(state.canvases[k].nodes, edges),
        },
      };
      return {
        dirty: true,
        testHighlight: null,
        canvases,
        journeyPath: computeJourneyPath(canvases[state.selected]),
      };
    });
    return true;
  },

  removeEdge: (k, edgeId) =>
    set((state) => {
      const canvas = state.canvases[k];
      const edges = canvas.edges.filter((e) => e.id !== edgeId);
      const canvases = {
        ...state.canvases,
        [k]: {
          ...canvas,
          edges,
          rootNodeId: computeRootNodeId(canvas.nodes, edges),
        },
      };
      return {
        dirty: true,
        testHighlight: null,
        canvases,
        journeyPath: computeJourneyPath(canvases[state.selected]),
      };
    }),

  onNodesChange: (k, changes) =>
    set((state) => {
      const canvas = state.canvases[k];
      const nodes = applyNodeChanges(
        changes,
        canvas.nodes as Node[],
      ) as RFNode[];
      // Pure selection/measurement ticks (drag-less clicks, dimension probes)
      // are not real edits — don't flip dirty for those (WS2).
      const mutated = changes.some(
        (c) => c.type !== "select" && c.type !== "dimensions",
      );
      const canvases = {
        ...state.canvases,
        [k]: {
          ...canvas,
          nodes,
          rootNodeId: computeRootNodeId(nodes, canvas.edges),
        },
      };
      // Only node add/remove can move journey membership — NOT position/select/
      // dimension ticks. Recompute only then so drag frames stay cheap.
      const structural = changes.some(
        (c) => c.type === "add" || c.type === "remove",
      );
      return {
        dirty: state.dirty || mutated,
        canvases,
        journeyPath: structural
          ? computeJourneyPath(canvases[state.selected])
          : state.journeyPath,
      };
    }),

  onEdgesChange: (k, changes) =>
    set((state) => {
      const canvas = state.canvases[k];
      const edges = applyEdgeChanges(changes, canvas.edges) as RFEdge[];
      const mutated = changes.some((c) => c.type !== "select");
      const canvases = {
        ...state.canvases,
        [k]: {
          ...canvas,
          edges,
          rootNodeId: computeRootNodeId(canvas.nodes, edges),
        },
      };
      return {
        dirty: state.dirty || mutated,
        canvases,
        // Edge add/remove changes reachability; selection ticks don't.
        journeyPath: mutated
          ? computeJourneyPath(canvases[state.selected])
          : state.journeyPath,
      };
    }),

  openNodeConfig: (nodeId) => set({ configNodeId: nodeId }),

  updateNodeProcessor: (k, nodeId, processor) =>
    set((state) => {
      const canvas = state.canvases[k];
      return {
        dirty: true,
        testHighlight: null,
        canvases: {
          ...state.canvases,
          [k]: {
            ...canvas,
            nodes: canvas.nodes.map((n) =>
              n.id === nodeId && n.type === "decisionNode"
                ? { ...n, data: { ...n.data, processor } }
                : n,
            ),
          },
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
  const current = hashRuleGraph(serializeRuleGraph(state.canvases));
  return current !== state.baselineHash;
}

export function selectCurrentRuleGraph(state: RuleBuilderState): RuleGraph {
  return serializeRuleGraph(state.canvases);
}

// --- always-on journey path (spec §4a / req 1) ---
//
// The set of node + edge ids on ANY START -> outcome path: nodes that are
// forward-reachable from the start node AND can reach an outcome, plus the
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

  const start = canvas.nodes.find((n) => n.type === "startNode");

  // Anchor forward-reachability at the computed ROOT (not strictly the start
  // node) since the frontend-only `edge_start` is only wired at seed time and
  // may not point at the current root after free-form editing. The start node
  // is still included as the entry marker, and the start->root edge if present.
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

  // can-reach-outcome: reverse-BFS from outcome nodes (over ALL edges).
  const rev = new Map<string, string[]>();
  for (const e of canvas.edges) {
    const list = rev.get(e.target);
    if (list) list.push(e.source);
    else rev.set(e.target, [e.source]);
  }
  const outcomeIds = canvas.nodes
    .filter((n) => n.type === "outcomeNode")
    .map((n) => n.id);
  const canReach = new Set<string>(outcomeIds);
  const stackR = [...outcomeIds];
  while (stackR.length > 0) {
    const cur = stackR.pop() as string;
    for (const prev of rev.get(cur) ?? []) {
      if (!canReach.has(prev)) {
        canReach.add(prev);
        stackR.push(prev);
      }
    }
  }

  // Journey nodes: reachable-from-root AND can-reach-outcome.
  for (const id of reachable) {
    if (canReach.has(id)) nodeIds.add(id);
  }
  // Always include the start node as the entry marker.
  if (start) nodeIds.add(start.id);

  // Journey edges: both endpoints on the journey. This naturally picks up the
  // start->root edge when the root is on the journey.
  for (const e of canvas.edges) {
    if (nodeIds.has(e.source) && nodeIds.has(e.target)) edgeIds.add(e.id);
  }

  return { nodeIds, edgeIds };
}

// Selector form for the currently-selected canvas. Returns the MEMOIZED field
// (recomputed inside the structural reducers) — it does NOT re-run the two BFS
// per call, so node/edge renderers can read it on every render frame cheaply.
export function selectJourneyPath(state: RuleBuilderState): JourneyPath {
  return state.journeyPath;
}

export { CANVAS_KEYS };
