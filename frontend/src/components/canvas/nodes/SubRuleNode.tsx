"use client";

// Sub Rule node: hot-pink rectangle with a "SUB RULE" superscript. Render-only
// (post-MVP) — registered so existing graphs that contain this node type render,
// but not creatable in the MVP palette. (FRONTEND CONTRACT §3)
import { memo } from "react";
import { Handle, Position, type NodeProps } from "reactflow";

import type { SubRuleNodeData } from "@/lib/canvas/types";

function SubRuleNodeImpl({ data }: NodeProps<SubRuleNodeData>) {
  return (
    <div
      className="relative min-w-[90px] rounded-md border-2 border-node-subrule bg-node-subrule px-3 py-2 text-center shadow-md"
      data-testid="subrule-node"
    >
      <span className="absolute -top-2 left-2 rounded bg-bg-elevated px-1 text-[9px] font-bold uppercase tracking-wide text-node-subrule">
        Sub Rule
      </span>
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

export const SubRuleNode = memo(SubRuleNodeImpl);
