"use client";

// Start node (Phase 1, Task C): a frontend-only visual entry marker. Black
// rectangle with white text in BOTH themes (fixed node-start hex, mirrors
// OutcomeNode). It has ONLY a SOURCE handle (bottom) — no target handle — so
// nothing can connect INTO it. Never persisted (stripped on serialize) and
// non-deletable (guarded in store + canvas). It never affects root detection.
import { memo } from "react";
import { Handle, Position, type NodeProps } from "reactflow";

import { useRuleBuilderStore } from "@/state/ruleBuilderStore";
import type { StartNodeData } from "@/lib/canvas/types";

function StartNodeImpl({ id, data, selected }: NodeProps<StartNodeData>) {
  const hasError = useRuleBuilderStore((s) => Boolean(s.nodeErrors[id]));
  // Dim with the rest of the canvas while a test result is active (the start
  // marker is never "on path" itself).
  const testActive = useRuleBuilderStore((s) => s.testHighlight !== null);

  return (
    <div
      className={[
        "min-w-[80px] rounded-full border-2 bg-node-start px-4 py-1.5 text-center shadow-md transition-opacity",
        // Teal border so the black entry node stays visible against the dark
        // canvas (black fill blends otherwise); also reads as the entry point.
        hasError ? "border-danger ring-2 ring-danger" : "border-brand-500",
        selected ? "ring-2 ring-brand-500" : "",
        testActive ? "opacity-30" : "",
      ].join(" ")}
      data-testid="start-node"
    >
      <span className="flex items-center justify-center gap-1.5 text-xs font-semibold uppercase tracking-wide text-white">
        {/* Teal entry marker differentiates the start node from the otherwise
            identically-black Outcome node (LOW-6). Bg/text stay black/white. */}
        <span aria-hidden className="text-[10px] leading-none text-brand-400">
          ▶
        </span>
        {data.label || "Start"}
      </span>
      {/* source-only: nothing can connect INTO the start node */}
      <Handle
        id="out"
        type="source"
        position={Position.Bottom}
        className="!h-2.5 !w-2.5 !border-2 !border-white !bg-brand-500"
      />
    </div>
  );
}

export const StartNode = memo(StartNodeImpl);
