"use client";

// Read-only diff edge: a bezier coloured by diff status (added=green,
// removed=red dashed, unchanged=gray) with a small YES/NO branch pill at the
// midpoint. Store-free, no delete affordance, no floating recompute — positions
// are real so the default handle routing is good enough for a read-only diff.
import { memo } from "react";
import {
  BaseEdge,
  EdgeLabelRenderer,
  getBezierPath,
  type EdgeProps,
} from "@xyflow/react";

import { edgeStroke } from "@/components/canvas/compare/diffStyles";
import { START_NODE_ID } from "@/lib/canvas/types";
import type { DiffRFEdge } from "@/components/canvas/compare/diffData";

function DiffEdgeImpl({
  id,
  source,
  sourceX,
  sourceY,
  targetX,
  targetY,
  sourcePosition,
  targetPosition,
  markerEnd,
  data,
}: EdgeProps<DiffRFEdge>) {
  const [edgePath, labelX, labelY] = getBezierPath({
    sourceX,
    sourceY,
    targetX,
    targetY,
    sourcePosition,
    targetPosition,
  });

  const status = data?.status ?? "unchanged";
  const branch = data?.branch ?? "yes";
  const isYes = branch === "yes";
  const isStartEdge = source === START_NODE_ID;
  const stroke = edgeStroke(status);

  return (
    <>
      <BaseEdge
        id={id}
        path={edgePath}
        markerEnd={markerEnd}
        style={{
          stroke,
          strokeWidth: status === "unchanged" ? 2 : 3,
          strokeDasharray: status === "removed" ? "6 4" : undefined,
          opacity: status === "removed" ? 0.8 : 1,
        }}
      />
      {!isStartEdge && (
        <EdgeLabelRenderer>
          <div
            className={[
              "nodrag nopan pointer-events-none absolute flex items-center gap-1 rounded-full border bg-bg-elevated px-1.5 py-0.5 text-[10px] font-bold uppercase tracking-wide shadow-sm",
              isYes ? "border-node-yes text-node-yes" : "border-node-no text-node-no",
            ].join(" ")}
            style={{
              transform: `translate(-50%, -50%) translate(${labelX}px, ${labelY}px)`,
            }}
          >
            {isYes ? "YES" : "NO"}
          </div>
        </EdgeLabelRenderer>
      )}
    </>
  );
}

export const DiffEdge = memo(DiffEdgeImpl);
