"use client";

// Outcome node: black rectangle, terminal (target handle only — no outgoing
// edges allowed). The title is a denormalized display cache re-resolved from
// the outcomes query; outcome_id is canonical. (Task 11)
import { memo } from "react";
import { Handle, Position, type NodeProps } from "reactflow";

import { useRuleBuilderStore } from "@/state/ruleBuilderStore";
import type { OutcomeNodeData } from "@/lib/canvas/types";

function OutcomeNodeImpl({ id, data, selected }: NodeProps<OutcomeNodeData>) {
  const hasError = useRuleBuilderStore((s) => Boolean(s.nodeErrors[id]));
  // Test-a-rule highlight (WS4). The matched outcome gets a stronger "winner"
  // ring; on-path outcomes glow; the rest dim while a result is active.
  const onPath = useRuleBuilderStore(
    (s) => s.testHighlight?.nodeIds.has(id) ?? false,
  );
  const isMatched = useRuleBuilderStore(
    (s) => s.testHighlight?.outcomeNodeId === id,
  );
  const testActive = useRuleBuilderStore((s) => s.testHighlight !== null);
  const dimmed = testActive && !onPath && !isMatched;
  // Always-on journey baseline (req 1): subtle steady ring when on a START ->
  // outcome path, suppressed while a test result is active. Reads the MEMOIZED
  // journeyPath (recomputed in the structural reducers) — a stable boolean, NOT
  // per-node BFS.
  const onJourney = useRuleBuilderStore((s) => s.journeyPath.nodeIds.has(id));
  const journeyBaseline = onJourney && !testActive;

  return (
    <div
      // `relative z-10` so the outcome's text/handles are never occluded by an
      // overlapping floated decision diamond (z-index parity — req 4).
      className={[
        "relative z-10 min-w-[90px] max-w-[140px] rounded-md border-2 bg-node-outcome px-3 py-2 text-center shadow-md transition-opacity",
        // Light neutral border so the black outcome node stays visible against
        // the dark canvas (black-on-black blends otherwise); subtle on light.
        hasError ? "border-danger ring-2 ring-danger" : "border-white/30",
        selected ? "ring-2 ring-brand-500" : "",
        isMatched
          ? "ring-4 ring-brand-500 ring-offset-2"
          : onPath
            ? "ring-4 ring-brand-400 ring-offset-2"
            : journeyBaseline
              ? "ring-1 ring-brand-400/40"
              : "",
        dimmed ? "opacity-30" : "",
      ].join(" ")}
      data-testid="outcome-node"
    >
      <Handle
        id="in"
        type="target"
        position={Position.Top}
        className="!h-2.5 !w-2.5 !border-2 !border-white !bg-brand-500"
      />
      <span className="block truncate text-xs font-semibold text-white">
        {data.title || "Outcome"}
      </span>
    </div>
  );
}

export const OutcomeNode = memo(OutcomeNodeImpl);
