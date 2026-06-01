"use client";

// The React Flow canvas (Tasks 11/12). Controlled mode: nodes/edges come from
// canvases[selected] in the Zustand store; all RF callbacks dispatch to store
// setters. Loaded via next/dynamic({ ssr:false }) by RuleBuilderClient to avoid
// hydration mismatch. Drag-drop from the palette + edge connect land here.
import { useCallback, useRef } from "react";
import ReactFlow, {
  Background,
  BackgroundVariant,
  Controls,
  ReactFlowProvider,
  useReactFlow,
  type Connection,
  type EdgeChange,
  type Node as RFLibNode,
  type NodeChange,
  type NodeTypes,
  type EdgeTypes,
} from "reactflow";

import { CanvasFallbackList } from "@/components/canvas/CanvasFallbackList";
import { FullScreenToggle } from "@/components/canvas/FullScreenToggle";
import { TemplateLibraryButton } from "@/components/canvas/TemplateLibraryButton";
import { DecisionNode } from "@/components/canvas/nodes/DecisionNode";
import { OutcomeNode } from "@/components/canvas/nodes/OutcomeNode";
import { StartNode } from "@/components/canvas/nodes/StartNode";
import { SubRuleNode } from "@/components/canvas/nodes/SubRuleNode";
import { ActionNode } from "@/components/canvas/nodes/ActionNode";
import { LabeledEdge } from "@/components/canvas/edges/LabeledEdge";
import { CHIP_MIME, type ChipPayload } from "@/lib/canvas/nodeTemplates";
import { useRuleBuilderStore } from "@/state/ruleBuilderStore";
import {
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
  outcomeNode: OutcomeNode,
  subRuleNode: SubRuleNode,
  actionNode: ActionNode,
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
  const { screenToFlowPosition } = useReactFlow();

  const canvas = useRuleBuilderStore((s) => s.canvases[canvasKey]);
  const onNodesChange = useRuleBuilderStore((s) => s.onNodesChange);
  const onEdgesChange = useRuleBuilderStore((s) => s.onEdgesChange);
  const addNode = useRuleBuilderStore((s) => s.addNode);
  const addEdge = useRuleBuilderStore((s) => s.addEdge);
  const openNodeConfig = useRuleBuilderStore((s) => s.openNodeConfig);

  const handleNodesChange = useCallback(
    (changes: NodeChange[]) => {
      // The start node is non-deletable: drop any remove change targeting it
      // (belt-and-suspenders alongside node-level deletable:false + the store
      // removeNode guard).
      const filtered = changes.filter(
        (c) => !(c.type === "remove" && c.id === START_NODE_ID),
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
      const branch = (conn.sourceHandle ?? "yes") as Branch;
      const edge: RFEdge = {
        id: nextId("e"),
        source: conn.source,
        target: conn.target,
        sourceHandle: branch,
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
              type: "outcomeNode",
              position,
              data: { outcomeId: payload.outcomeId, title: payload.title },
            };
      addNode(canvasKey, node);
    },
    [editable, screenToFlowPosition, addNode, canvasKey],
  );

  // Double-click opens a decision node's editable config (edit mode).
  const onNodeDoubleClick = useCallback(
    (_e: React.MouseEvent, node: RFLibNode) => {
      if (node.type === "decisionNode") openNodeConfig(node.id);
    },
    [openNodeConfig],
  );

  // Single click opens the config/inspection drawer for any inspectable node
  // (decision / outcome / start). In read-only mode this is the way to inspect
  // a node's contents; the drawer renders view-only when not editing. Mutation
  // is still gated by `editable` elsewhere, so a click here never edits.
  const onNodeClick = useCallback(
    (_e: React.MouseEvent, node: RFLibNode) => {
      if (
        node.type === "decisionNode" ||
        node.type === "outcomeNode" ||
        node.type === "startNode"
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

      {/* bottom-right cluster: full-screen + template library */}
      <div className="pointer-events-none absolute bottom-4 right-16 z-10 flex items-end gap-2">
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
