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
  const edges = root
    ? [...canvas.edges, startEdge(root)]
    : canvas.edges;
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

  setSelected: (k) => set({ selected: k }),

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
      return {
        dirty: true,
        testHighlight: null,
        canvases: {
          ...state.canvases,
          [k]: {
            ...canvas,
            nodes,
            rootNodeId: computeRootNodeId(nodes, canvas.edges),
          },
        },
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
      return {
        dirty: true,
        testHighlight: null,
        canvases: {
          ...state.canvases,
          [k]: { nodes, edges, rootNodeId: computeRootNodeId(nodes, edges) },
        },
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
    set((state) => ({
      dirty: true,
      testHighlight: null,
      canvases: {
        ...state.canvases,
        [k]: {
          ...state.canvases[k],
          edges,
          rootNodeId: computeRootNodeId(state.canvases[k].nodes, edges),
        },
      },
    }));
    return true;
  },

  removeEdge: (k, edgeId) =>
    set((state) => {
      const canvas = state.canvases[k];
      const edges = canvas.edges.filter((e) => e.id !== edgeId);
      return {
        dirty: true,
        testHighlight: null,
        canvases: {
          ...state.canvases,
          [k]: {
            ...canvas,
            edges,
            rootNodeId: computeRootNodeId(canvas.nodes, edges),
          },
        },
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
      return {
        dirty: state.dirty || mutated,
        canvases: {
          ...state.canvases,
          [k]: {
            ...canvas,
            nodes,
            rootNodeId: computeRootNodeId(nodes, canvas.edges),
          },
        },
      };
    }),

  onEdgesChange: (k, changes) =>
    set((state) => {
      const canvas = state.canvases[k];
      const edges = applyEdgeChanges(changes, canvas.edges) as RFEdge[];
      const mutated = changes.some((c) => c.type !== "select");
      return {
        dirty: state.dirty || mutated,
        canvases: {
          ...state.canvases,
          [k]: {
            ...canvas,
            edges,
            rootNodeId: computeRootNodeId(canvas.nodes, edges),
          },
        },
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

export { CANVAS_KEYS };
