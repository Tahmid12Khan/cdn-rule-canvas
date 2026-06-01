"use client";

// Custom edge rendering a YES (teal) / NO (gray) outlined oval pill at the edge
// midpoint. The branch is read from edge.data.branch. (Task 12)
import { memo } from "react";
import {
  BaseEdge,
  EdgeLabelRenderer,
  getBezierPath,
  type EdgeProps,
} from "reactflow";

import { useRuleBuilderStore } from "@/state/ruleBuilderStore";
import type { RFEdgeData } from "@/lib/canvas/types";

function LabeledEdgeImpl({
  id,
  sourceX,
  sourceY,
  targetX,
  targetY,
  sourcePosition,
  targetPosition,
  data,
  markerEnd,
}: EdgeProps<RFEdgeData>) {
  const [edgePath, labelX, labelY] = getBezierPath({
    sourceX,
    sourceY,
    targetX,
    targetY,
    sourcePosition,
    targetPosition,
  });

  const branch = data?.branch ?? "yes";
  const isYes = branch === "yes";

  // Test-a-rule highlight (WS4): chosen edges get a thick animated brand
  // stroke; the rest fade while a result is active.
  const onPath = useRuleBuilderStore(
    (s) => s.testHighlight?.edgeIds.has(id) ?? false,
  );
  const testActive = useRuleBuilderStore((s) => s.testHighlight !== null);
  const dimmed = testActive && !onPath;

  return (
    <>
      <BaseEdge
        id={id}
        path={edgePath}
        markerEnd={markerEnd}
        style={{
          stroke: onPath ? "#0ea5a4" : isYes ? "#0ea5a4" : "#6b7280",
          strokeWidth: onPath ? 4 : 2,
          opacity: dimmed ? 0.25 : 1,
          strokeDasharray: onPath ? "6 4" : undefined,
          animation: onPath ? "rre-edge-dash 0.6s linear infinite" : undefined,
        }}
      />
      <EdgeLabelRenderer>
        <div
          // nodrag/nopan keep the pill from hijacking canvas gestures.
          className={[
            "nodrag nopan pointer-events-none absolute rounded-full border bg-bg-elevated px-1.5 py-0.5 text-[10px] font-bold uppercase tracking-wide shadow-sm",
            isYes
              ? "border-node-yes text-node-yes"
              : "border-node-no text-node-no",
          ].join(" ")}
          style={{
            transform: `translate(-50%, -50%) translate(${labelX}px, ${labelY}px)`,
            opacity: dimmed ? 0.25 : 1,
          }}
          data-testid={`edge-label-${branch}`}
        >
          {isYes ? "YES" : "NO"}
        </div>
      </EdgeLabelRenderer>
    </>
  );
}

export const LabeledEdge = memo(LabeledEdgeImpl);
