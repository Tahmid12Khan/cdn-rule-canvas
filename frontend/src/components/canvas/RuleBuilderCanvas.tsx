"use client";

// The React Flow canvas (Tasks 11/12). Controlled mode: nodes/edges come from
// canvases[selected] in the Zustand store; all RF callbacks dispatch to store
// setters. Loaded via next/dynamic({ ssr:false }) by RuleBuilderClient to avoid
// hydration mismatch. Drag-drop from the palette + edge connect land here.
import { useCallback, useEffect, useRef } from "react";
import {
  ReactFlow,
  Background,
  BackgroundVariant,
  ConnectionMode,
  Controls,
  ReactFlowProvider,
  useReactFlow,
  type Connection,
  type EdgeChange,
  type Node as RFLibNode,
  type NodeChange,
  type NodeTypes,
  type EdgeTypes,
} from "@xyflow/react";

import { CanvasFallbackList } from "@/components/canvas/CanvasFallbackList";
import { FullScreenToggle } from "@/components/canvas/FullScreenToggle";
import { TemplateLibraryButton } from "@/components/canvas/TemplateLibraryButton";
import { DecisionNode } from "@/components/canvas/nodes/DecisionNode";
import { EndNode } from "@/components/canvas/nodes/EndNode";
import { ExpressionNode } from "@/components/canvas/nodes/ExpressionNode";
import { StartNode } from "@/components/canvas/nodes/StartNode";
import { LabeledEdge } from "@/components/canvas/edges/LabeledEdge";
import { autoLayout, needsLayout } from "@/lib/canvas/layout";
import { CHIP_MIME, type ChipPayload } from "@/lib/canvas/nodeTemplates";
import { CANVAS_KEYS, useRuleBuilderStore } from "@/state/ruleBuilderStore";
import {
  END_NODE_ID,
  START_NODE_ID,
  type Branch,
  type CanvasKey,
  type RFEdge,
  type RFNode,
} from "@/lib/canvas/types";

// Module-level constants — MUST NOT be inlined (prevents React Flow re-mounting
// every node each render — FRONTEND CONTRACT §8).
const NODE_TYPES: NodeTypes = {
  startNode: StartNode,
  decisionNode: DecisionNode,
  expressionNode: ExpressionNode,
  endNode: EndNode,
};
const EDGE_TYPES: EdgeTypes = {
  labeledEdge: LabeledEdge,
};

let idSeq = 0;
function nextId(prefix: string): string {
  idSeq += 1;
  return `${prefix}_${Date.now().toString(36)}_${idSeq}`;
}

interface RuleBuilderCanvasProps {
  canvasKey: CanvasKey;
  editable: boolean;
}

function CanvasInner({ canvasKey, editable }: RuleBuilderCanvasProps) {
  const wrapperRef = useRef<HTMLDivElement>(null);
  const { screenToFlowPosition, fitView } = useReactFlow();

  const canvas = useRuleBuilderStore((s) => s.canvases[canvasKey]);
  const onNodesChange = useRuleBuilderStore((s) => s.onNodesChange);
  const onEdgesChange = useRuleBuilderStore((s) => s.onEdgesChange);
  const addNode = useRuleBuilderStore((s) => s.addNode);
  const addEdge = useRuleBuilderStore((s) => s.addEdge);
  const openNodeConfig = useRuleBuilderStore((s) => s.openNodeConfig);
  const setNodePositions = useRuleBuilderStore((s) => s.setNodePositions);
  const applySeedLayout = useRuleBuilderStore((s) => s.applySeedLayout);

  // Auto-layout the seeded graph ONCE per (fid/vnum) seed when its nodes lack a
  // meaningful layout (overlapping positions from a template / older save). Read
  // the full store via getState so this doesn't churn on every drag frame.
  // Keyed off baselineHash so it re-arms after each fresh seed but never re-runs
  // for the same baseline. Uses applySeedLayout so it doesn't flip dirty.
  const laidOutForBaseline = useRef<string | null>(null);
  const baselineHash = useRuleBuilderStore((s) => s.baselineHash);
  useEffect(() => {
    if (laidOutForBaseline.current === baselineHash) return;
    const { canvases } = useRuleBuilderStore.getState();
    if (!CANVAS_KEYS.some((k) => needsLayout(canvases[k].nodes))) {
      laidOutForBaseline.current = baselineHash;
      return;
    }
    const positionsByCanvas = {} as Record<
      (typeof CANVAS_KEYS)[number],
      Map<string, { x: number; y: number }>
    >;
    for (const k of CANVAS_KEYS) {
      const c = canvases[k];
      positionsByCanvas[k] = autoLayout(c.nodes, c.edges);
    }
    applySeedLayout(positionsByCanvas);
    laidOutForBaseline.current =
      useRuleBuilderStore.getState().baselineHash;
    window.requestAnimationFrame(() => fitView({ duration: 200 }));
  }, [baselineHash, applySeedLayout, fitView]);

  // "Tidy layout" control: re-run dagre on the CURRENT canvas (a deliberate
  // user edit, so it flips dirty via setNodePositions) and re-fit the view.
  const handleTidyLayout = useCallback(() => {
    const positions = autoLayout(canvas.nodes, canvas.edges);
    setNodePositions(canvasKey, positions);
    window.requestAnimationFrame(() => fitView({ duration: 200 }));
  }, [canvas.nodes, canvas.edges, setNodePositions, canvasKey, fitView]);

  const handleNodesChange = useCallback(
    (changes: NodeChange[]) => {
      // The start + end nodes are non-deletable: drop any remove change
      // targeting them (belt-and-suspenders alongside node-level
      // deletable:false + the store removeNode guard).
      const filtered = changes.filter(
        (c) =>
          !(
            c.type === "remove" &&
            (c.id === START_NODE_ID || c.id === END_NODE_ID)
          ),
      );
      onNodesChange(canvasKey, filtered);
    },
    [onNodesChange, canvasKey],
  );
  const handleEdgesChange = useCallback(
    (changes: EdgeChange[]) => onEdgesChange(canvasKey, changes),
    [onEdgesChange, canvasKey],
  );

  const onConnect = useCallback(
    (conn: Connection) => {
      if (!editable || !conn.source || !conn.target) return;
      // Keep the actual dragged handle id as sourceHandle ("yes"/"no" on a
      // decision, "out" on start/expression) so React Flow v12 renders the edge
      // (error #008 otherwise). The wire branch is yes/no only.
      const handle = conn.sourceHandle ?? "out";
      const branch: Branch = handle === "no" ? "no" : "yes";
      const edge: RFEdge = {
        id: nextId("e"),
        source: conn.source,
        target: conn.target,
        sourceHandle: handle,
        type: "labeledEdge",
        data: { branch },
      };
      addEdge(canvasKey, edge);
    },
    [editable, addEdge, canvasKey],
  );

  const onDragOver = useCallback((e: React.DragEvent) => {
    e.preventDefault();
    e.dataTransfer.dropEffect = "copy";
  }, []);

  const onDrop = useCallback(
    (e: React.DragEvent) => {
      e.preventDefault();
      if (!editable) return;
      const raw = e.dataTransfer.getData(CHIP_MIME);
      if (!raw) return;
      let payload: ChipPayload;
      try {
        payload = JSON.parse(raw) as ChipPayload;
      } catch {
        return;
      }
      const position = screenToFlowPosition({ x: e.clientX, y: e.clientY });

      const node: RFNode =
        payload.kind === "decision"
          ? {
              id: nextId("n"),
              type: "decisionNode",
              position,
              data: { processor: payload.processor },
            }
          : {
              id: nextId("n"),
              type: "expressionNode",
              position,
              data: { action: payload.action },
            };
      addNode(canvasKey, node);
    },
    [editable, screenToFlowPosition, addNode, canvasKey],
  );

  // Double-click opens an editable config (edit mode) for nodes that carry a
  // config form (decision / expression).
  const onNodeDoubleClick = useCallback(
    (_e: React.MouseEvent, node: RFLibNode) => {
      if (node.type === "decisionNode" || node.type === "expressionNode") {
        openNodeConfig(node.id);
      }
    },
    [openNodeConfig],
  );

  // Single click opens the config/inspection drawer for any inspectable node
  // (start / decision / expression / end). In read-only mode this is the way to
  // inspect a node's contents; the drawer renders view-only when not editing.
  // Mutation is still gated by `editable` elsewhere, so a click here never edits.
  const onNodeClick = useCallback(
    (_e: React.MouseEvent, node: RFLibNode) => {
      if (
        node.type === "startNode" ||
        node.type === "decisionNode" ||
        node.type === "expressionNode" ||
        node.type === "endNode"
      ) {
        openNodeConfig(node.id);
      }
    },
    [openNodeConfig],
  );

  return (
    <div
      ref={wrapperRef}
      role="region"
      aria-label={`Rule canvas, ${canvas.nodes.length} nodes`}
      className="relative h-[520px] w-full overflow-hidden rounded-lg border border-border bg-bg"
      onDrop={onDrop}
      onDragOver={onDragOver}
    >
      <ReactFlow
        nodes={canvas.nodes}
        edges={canvas.edges}
        nodeTypes={NODE_TYPES}
        edgeTypes={EDGE_TYPES}
        onNodesChange={handleNodesChange}
        onEdgesChange={handleEdgesChange}
        onConnect={onConnect}
        onNodeClick={onNodeClick}
        onNodeDoubleClick={onNodeDoubleClick}
        // Loose so a drag can complete onto a node from any side; the edge then
        // re-renders floated to the closest border (LabeledEdge), so the single
        // top target handle never has to be aimed at precisely (req 5).
        connectionMode={ConnectionMode.Loose}
        nodesDraggable={editable}
        nodesConnectable={editable}
        // Always selectable so clicks fire onNodeClick even in read-only mode
        // (inspection). Selection alone mutates nothing; `editable` gates every
        // real mutation (drag/connect/drop).
        elementsSelectable
        // Delete a selected node/edge with either key while editing. React Flow
        // defaults to Backspace only, so the Delete key silently did nothing.
        // `null` in read-only mode keeps inspection non-destructive.
        deleteKeyCode={editable ? ["Backspace", "Delete"] : null}
        fitView
        proOptions={{ hideAttribution: true }}
      >
        <Background
          variant={BackgroundVariant.Dots}
          gap={20}
          size={1}
          // Theme-aware: reads --dot so dots adapt to light/dark canvas surfaces
          // (RF Background accepts any CSS color string).
          color="var(--dot)"
        />
        <Controls
          position="bottom-right"
          showInteractive={false}
          className="!shadow-sm"
        />
      </ReactFlow>

      {/* bottom-right cluster: tidy layout + full-screen + template library */}
      <div className="pointer-events-none absolute bottom-4 right-16 z-10 flex items-end gap-2">
        <div className="pointer-events-auto">
          <button
            type="button"
            onClick={handleTidyLayout}
            title="Tidy layout (auto-arrange top-down)"
            aria-label="Tidy layout"
            className="flex h-9 items-center gap-1.5 rounded-md border border-border bg-bg-elevated px-3 text-xs font-semibold text-nav shadow-sm hover:bg-brand-50 hover:text-brand-700"
          >
            <span aria-hidden>⤓</span>
            Tidy layout
          </button>
        </div>
        <div className="pointer-events-auto">
          <FullScreenToggle targetRef={wrapperRef} />
        </div>
        <div className="pointer-events-auto">
          <TemplateLibraryButton />
        </div>
      </div>

      <CanvasFallbackList canvas={canvas} />
    </div>
  );
}

export default function RuleBuilderCanvas(props: RuleBuilderCanvasProps) {
  return (
    <ReactFlowProvider>
      <CanvasInner {...props} />
    </ReactFlowProvider>
  );
}
