"use client";

// Read-only diff terminal node — handles BOTH start and end (a black pill in the
// editor; here a status-coloured pill). Start = source handle only (bottom);
// End = target handle only (top). Store-free. Start/end almost always read as
// "unchanged" (only position can differ) but the union may still surface them as
// added/removed when a whole canvas appears or disappears between versions.
import { memo } from "react";
import { Handle, Position, type NodeProps } from "reactflow";

import { nodePlateClass } from "@/components/canvas/compare/diffStyles";
import type { DiffNodeData } from "@/components/canvas/compare/diffData";

function DiffTerminalNodeImpl({ data }: NodeProps<DiffNodeData>) {
  const isStart = data.graphNode.kind === "start";
  const label = isStart ? "Start" : "END";

  return (
    <div
      className={[
        "relative z-10 min-w-[80px] rounded-full border-2 px-4 py-1.5 text-center shadow-md",
        nodePlateClass(data.status),
        data.focused ? "ring-4 ring-action ring-offset-2" : "",
      ].join(" ")}
      data-testid="diff-terminal-node"
    >
      {!isStart && (
        <Handle id="in" type="target" position={Position.Top} className="!h-2 !w-2 !border !border-white !bg-fg-muted" />
      )}
      <span className="flex items-center justify-center gap-1.5 text-xs font-semibold uppercase tracking-wide text-fg">
        <span aria-hidden className="text-[10px] leading-none">
          {isStart ? "▶" : "■"}
        </span>
        {label}
      </span>
      {isStart && (
        <Handle id="out" type="source" position={Position.Bottom} className="!h-2 !w-2 !border !border-white !bg-fg-muted" />
      )}
    </div>
  );
}

export const DiffTerminalNode = memo(DiffTerminalNodeImpl);
