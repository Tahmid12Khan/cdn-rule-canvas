"use client";

// Action node: orange/amber rectangle. Render-only (post-MVP) — registered so
// existing graphs render, but not creatable in the MVP palette. (FRONTEND
// CONTRACT §3)
import { memo } from "react";
import { Handle, Position, type NodeProps } from "reactflow";

import type { ActionNodeData } from "@/lib/canvas/types";

function ActionNodeImpl({ data }: NodeProps<ActionNodeData>) {
  return (
    <div
      className="min-w-[90px] rounded-md border-2 border-node-action bg-node-action px-3 py-2 text-center shadow-md"
      data-testid="action-node"
    >
      <Handle
        id="in"
        type="target"
        position={Position.Top}
        className="!h-2.5 !w-2.5 !border-2 !border-white !bg-bg-elevated"
      />
      <span className="block truncate text-xs font-semibold text-white">
        {data.label}
      </span>
      <Handle
        id="out"
        type="source"
        position={Position.Bottom}
        className="!h-2.5 !w-2.5 !border-2 !border-white !bg-bg-elevated"
      />
    </div>
  );
}

export const ActionNode = memo(ActionNodeImpl);
