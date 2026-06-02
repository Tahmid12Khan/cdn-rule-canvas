"use client";

// End node (expression-nodes-spec §1): the terminal that stops the flow. An
// "END" pill with a TARGET handle only — zero outgoing edges (end_terminal).
// Auto-injected (≥1 per non-empty canvas), non-deletable. Persisted on the wire
// as an `end` node. Visually mirrors the StartNode (black pill) so the
// start/end pair reads as the journey's bookends.
import { memo } from "react";
import { Handle, Position, type NodeProps } from "@xyflow/react";

import { useRuleBuilderStore } from "@/state/ruleBuilderStore";
import type { RFEndNode } from "@/lib/canvas/types";

function EndNodeImpl({ id, data, selected }: NodeProps<RFEndNode>) {
  const hasError = useRuleBuilderStore((s) => Boolean(s.nodeErrors[id]));
  const onPath = useRuleBuilderStore(
    (s) => s.testHighlight?.nodeIds.has(id) ?? false,
  );
  const testActive = useRuleBuilderStore((s) => s.testHighlight !== null);
  const dimmed = testActive && !onPath;
  const onJourney = useRuleBuilderStore((s) => s.journeyPath.nodeIds.has(id));
  const journeyBaseline = onJourney && !testActive;

  return (
    <div
      className={[
        "relative z-10 min-w-[80px] rounded-full border-2 bg-node-start px-4 py-1.5 text-center shadow-md transition-opacity",
        hasError ? "border-danger ring-2 ring-danger" : "border-border",
        selected ? "ring-2 ring-brand-500" : "",
        onPath
          ? "ring-4 ring-brand-400 ring-offset-2"
          : journeyBaseline
            ? "ring-1 ring-brand-400/40"
            : "",
        dimmed ? "opacity-30" : "",
      ].join(" ")}
      data-testid="end-node"
    >
      {/* target-only: the flow terminates here */}
      <Handle
        id="in"
        type="target"
        position={Position.Top}
        className="!h-2.5 !w-2.5 !border-2 !border-white !bg-brand-500"
      />
      <span className="flex items-center justify-center gap-1.5 text-xs font-semibold uppercase tracking-wide text-white">
        <span aria-hidden className="text-[10px] leading-none text-brand-400">
          ■
        </span>
        {data.label || "END"}
      </span>
    </div>
  );
}

export const EndNode = memo(EndNodeImpl);
