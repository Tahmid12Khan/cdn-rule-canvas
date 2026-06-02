"use client";

// One read-only React Flow instance rendering a single canvas's diff (union of
// both versions' nodes/edges). Store-free: nodes/edges are derived purely from
// the CanvasDiff. Registers its ReactFlowInstance via onReady so the parent
// dialog can pan/centre it when a change-list item is clicked. Each instance
// gets its OWN ReactFlowProvider (isolated RF store per canvas).
import { useCallback, useMemo } from "react";
import ReactFlow, {
  Background,
  BackgroundVariant,
  Controls,
  ReactFlowProvider,
  type EdgeTypes,
  type NodeTypes,
  type ReactFlowInstance,
} from "reactflow";

import { DiffEdge } from "@/components/canvas/compare/DiffEdge";
import { DiffDecisionNode } from "@/components/canvas/compare/diffNodes/DiffDecisionNode";
import { DiffExpressionNode } from "@/components/canvas/compare/diffNodes/DiffExpressionNode";
import { DiffTerminalNode } from "@/components/canvas/compare/diffNodes/DiffTerminalNode";
import type { CanvasDiff } from "@/lib/canvas/diff";
import type { Branch } from "@/lib/api/ruleGraph";

// Module-level so React Flow never remounts every node each render.
const NODE_TYPES: NodeTypes = {
  diffDecisionNode: DiffDecisionNode,
  diffExpressionNode: DiffExpressionNode,
  diffTerminalNode: DiffTerminalNode,
};
const EDGE_TYPES: EdgeTypes = { diffEdge: DiffEdge };

interface DiffCanvasProps {
  diff: CanvasDiff;
  focusedNodeId: string | null;
  onReady: (instance: ReactFlowInstance) => void;
}

function DiffCanvasInner({ diff, focusedNodeId, onReady }: DiffCanvasProps) {
  const rfNodes = useMemo(
    () =>
      diff.nodes.map((nd) => ({
        id: nd.id,
        type:
          nd.node.kind === "decision"
            ? "diffDecisionNode"
            : nd.node.kind === "expression"
              ? "diffExpressionNode"
              : "diffTerminalNode",
        position: nd.node.position,
        data: {
          graphNode: nd.node,
          status: nd.status,
          focused: nd.id === focusedNodeId,
        },
        draggable: false,
        selectable: false,
        connectable: false,
        deletable: false,
      })),
    [diff.nodes, focusedNodeId],
  );

  const kindById = useMemo(() => {
    const m = new Map<string, string>();
    for (const nd of diff.nodes) m.set(nd.id, nd.node.kind);
    return m;
  }, [diff.nodes]);

  const rfEdges = useMemo(
    () =>
      diff.edges.map((ed) => ({
        id: ed.key,
        source: ed.source,
        target: ed.target,
        // Decision sources route per branch (yes=right / no=bottom); start &
        // expression sources have a single "out" handle.
        sourceHandle:
          kindById.get(ed.source) === "decision" ? (ed.branch as Branch) : "out",
        targetHandle: "in",
        type: "diffEdge",
        data: { branch: ed.branch, status: ed.status },
      })),
    [diff.edges, kindById],
  );

  const handleInit = useCallback(
    (instance: ReactFlowInstance) => {
      instance.fitView({ padding: 0.2 });
      onReady(instance);
    },
    [onReady],
  );

  if (diff.nodes.length === 0) {
    return (
      <div className="flex h-[320px] w-full items-center justify-center rounded-lg border border-border bg-bg text-sm text-fg-muted">
        Empty canvas — no rules in either version.
      </div>
    );
  }

  return (
    <div className="h-[320px] w-full overflow-hidden rounded-lg border border-border bg-bg">
      <ReactFlow
        nodes={rfNodes}
        edges={rfEdges}
        nodeTypes={NODE_TYPES}
        edgeTypes={EDGE_TYPES}
        onInit={handleInit}
        nodesDraggable={false}
        nodesConnectable={false}
        elementsSelectable={false}
        deleteKeyCode={null}
        fitView
        proOptions={{ hideAttribution: true }}
      >
        <Background variant={BackgroundVariant.Dots} gap={20} size={1} color="var(--dot)" />
        <Controls position="bottom-right" showInteractive={false} className="!shadow-sm" />
      </ReactFlow>
    </div>
  );
}

export function DiffCanvas(props: DiffCanvasProps) {
  return (
    <ReactFlowProvider>
      <DiffCanvasInner {...props} />
    </ReactFlowProvider>
  );
}
