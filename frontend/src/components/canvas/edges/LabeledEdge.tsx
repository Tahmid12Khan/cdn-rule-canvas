"use client";

// Custom edge rendering a YES (teal) / NO (gray) outlined oval pill at the edge
// midpoint. The branch is read from edge.data.branch. (Task 12)
//
// When the edge is selected it thickens + glows so the selection is visible,
// and — while the canvas is in edit mode — a small × button appears next to the
// branch pill to delete the edge directly (no reliance on the Delete/Backspace
// key, which is easy to miss).
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
  selected,
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

  // Inline delete affordance: only while editing the currently-rendered canvas.
  const isEditing = useRuleBuilderStore((s) => s.isEditing);
  const activeCanvas = useRuleBuilderStore((s) => s.selected);
  const removeEdge = useRuleBuilderStore((s) => s.removeEdge);
  const showDelete = selected && isEditing;

  const baseStroke = isYes ? "#0ea5a4" : "#6b7280";

  return (
    <>
      <BaseEdge
        id={id}
        path={edgePath}
        markerEnd={markerEnd}
        style={{
          stroke: onPath ? "#0ea5a4" : selected ? "#0ea5a4" : baseStroke,
          strokeWidth: onPath ? 4 : selected ? 3.5 : 2,
          opacity: dimmed ? 0.25 : 1,
          strokeDasharray: onPath ? "6 4" : undefined,
          animation: onPath ? "rre-edge-dash 0.6s linear infinite" : undefined,
          filter: selected
            ? "drop-shadow(0 0 3px rgba(14,165,164,0.85))"
            : undefined,
        }}
      />
      <EdgeLabelRenderer>
        <div
          // nodrag/nopan keep the pill from hijacking canvas gestures.
          className={[
            "nodrag nopan pointer-events-none absolute flex items-center gap-1 rounded-full border bg-bg-elevated px-1.5 py-0.5 text-[10px] font-bold uppercase tracking-wide shadow-sm",
            isYes
              ? "border-node-yes text-node-yes"
              : "border-node-no text-node-no",
            selected ? "ring-2 ring-action" : "",
          ].join(" ")}
          style={{
            transform: `translate(-50%, -50%) translate(${labelX}px, ${labelY}px)`,
            opacity: dimmed ? 0.25 : 1,
          }}
          data-testid={`edge-label-${branch}`}
        >
          {isYes ? "YES" : "NO"}
          {showDelete && (
            <button
              type="button"
              // pointer-events re-enabled just for the button so it is clickable.
              className="nodrag nopan pointer-events-auto flex h-3.5 w-3.5 items-center justify-center rounded-full bg-danger text-[9px] leading-none text-white hover:opacity-90"
              aria-label={`Delete ${branch} edge`}
              title="Delete this connection"
              onClick={(e) => {
                e.stopPropagation();
                removeEdge(activeCanvas, id);
              }}
            >
              ×
            </button>
          )}
        </div>
      </EdgeLabelRenderer>
    </>
  );
}

export const LabeledEdge = memo(LabeledEdgeImpl);
