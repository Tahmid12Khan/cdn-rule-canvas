"use client";

// Custom edge rendering a YES (teal) / NO (gray) outlined oval pill at the edge
// midpoint. The branch is read from edge.data.branch. (Task 12)
//
// FLOATING TARGET ATTACH (spec §4 req 5): the edge meets the CLOSEST side of the
// destination node. We read the target node's geometry from the RF store
// (nodeInternals.get(target)) and recompute the target attach point + side via
// the pure `closestSide` helper, then feed it to getBezierPath. The SOURCE end
// is untouched — it still leaves whichever branch handle (yes=Right / no=Bottom)
// the edge belongs to, so branch semantics + the YES/NO pill are preserved.
// When the target hasn't been measured yet (width/height null on first paint),
// we fall back to the props RF passed in so the edge never jumps to (0,0).
//
// HIGHLIGHT PRECEDENCE: test trace (bright + animated dash) > always-on journey
// baseline (subtle steady) > neutral. The journey baseline is suppressed while a
// test result is active so the dim-the-rest behavior reads cleanly.
import { memo, useCallback } from "react";
import {
  BaseEdge,
  EdgeLabelRenderer,
  getBezierPath,
  useStore,
  type EdgeProps,
  type ReactFlowState,
} from "@xyflow/react";

import { closestSide, type NodeRect } from "@/lib/canvas/floating";
import { useRuleBuilderStore } from "@/state/ruleBuilderStore";
import { START_NODE_ID, type RFEdge } from "@/lib/canvas/types";

// Decision-node geometry (mirrors DecisionNode.tsx): the diamond plate is
// `h-28 w-28` = 112px and sits in a `flex flex-col items-center` column UNDER a
// title pill (`text-[10px]` + `py-0.5` + `mb-1` ≈ 24px tall). The node's FULL
// measured bounding box therefore includes the title pill above and the error
// label below the diamond. When floating an edge INTO a decision node we must
// attach to the diamond, so we inset the measured rect to the diamond region
// before picking the closest side — otherwise edges land in the gap above it.
const DIAMOND_SIZE = 112; // h-28 / w-28
const TITLE_PILL_H = 24; // title pill height incl. mb-1

function LabeledEdgeImpl({
  id,
  source,
  target,
  sourceX,
  sourceY,
  targetX,
  targetY,
  sourcePosition,
  targetPosition,
  data,
  markerEnd,
  selected,
}: EdgeProps<RFEdge>) {
  // Subscribe to ONLY the target node's internal record (re-floats the edge when
  // that node is dragged). useCallback keeps the selector identity stable so it
  // doesn't churn every render. v12: nodeLookup -> InternalNode, whose absolute
  // position lives at `.internals.positionAbsolute` and measured size at
  // `.measured`.
  const targetNode = useStore(
    useCallback((s: ReactFlowState) => s.nodeLookup.get(target), [target]),
  );

  // Float the target end to the closest border when the node is measured;
  // otherwise fall back to the coords RF already computed (first-paint safety).
  let tx = targetX;
  let ty = targetY;
  let tPos = targetPosition;
  if (
    targetNode?.internals.positionAbsolute &&
    targetNode.measured.width != null &&
    targetNode.measured.height != null
  ) {
    let rect: NodeRect = {
      x: targetNode.internals.positionAbsolute.x,
      y: targetNode.internals.positionAbsolute.y,
      w: targetNode.measured.width,
      h: targetNode.measured.height,
    };
    // For a decision node, inset the measured box to the 112px diamond region
    // (title pill above + error label below are excluded) so the edge attaches
    // to the diamond border, not the gap above it. The diamond is horizontally
    // centered in the flex-col column, so center the inset within the box too.
    if (targetNode.type === "decisionNode") {
      rect = {
        x: rect.x + (rect.w - DIAMOND_SIZE) / 2,
        y: rect.y + TITLE_PILL_H,
        w: DIAMOND_SIZE,
        h: DIAMOND_SIZE,
      };
    }
    const floated = closestSide(rect, sourceX, sourceY);
    tx = floated.tx;
    ty = floated.ty;
    tPos = floated.tPos;
  }

  const [edgePath, labelX, labelY] = getBezierPath({
    sourceX,
    sourceY,
    targetX: tx,
    targetY: ty,
    sourcePosition,
    targetPosition: tPos,
  });

  const branch = data?.branch ?? "yes";
  const isYes = branch === "yes";
  // The frontend-only START -> root entry edge is not a decision branch, so it
  // must NOT show a YES/NO pill or a delete button (it is non-deletable). We
  // still render its floating stroke so "Start" visibly connects to the root.
  const isStartEdge = source === START_NODE_ID;

  // Test-a-rule highlight (WS4): chosen edges get a thick animated brand stroke;
  // the rest fade while a result is active.
  const onPath = useRuleBuilderStore(
    (s) => s.testHighlight?.edgeIds.has(id) ?? false,
  );
  const testActive = useRuleBuilderStore((s) => s.testHighlight !== null);
  const dimmed = testActive && !onPath;

  // Always-on journey baseline (req 1): edges on any START -> outcome path get a
  // slightly stronger steady stroke. Suppressed while a test result is active so
  // the two highlight tiers never visually fight. Reads the MEMOIZED journeyPath
  // (recomputed in the structural reducers) — a stable boolean, NOT per-edge
  // BFS on every render/drag frame.
  const onJourney = useRuleBuilderStore((s) => s.journeyPath.edgeIds.has(id));
  const journeyBaseline = onJourney && !testActive;

  // Inline delete affordance: only while editing.
  const isEditing = useRuleBuilderStore((s) => s.isEditing);
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
          // Precedence: test path > selection > journey baseline > neutral.
          stroke: onPath
            ? "#0ea5a4"
            : selected
              ? "#0ea5a4"
              : journeyBaseline
                ? "#14b8a6"
                : baseStroke,
          strokeWidth: onPath ? 4 : selected ? 3.5 : journeyBaseline ? 2.5 : 2,
          opacity: dimmed ? 0.25 : journeyBaseline ? 0.9 : 1,
          strokeDasharray: onPath ? "6 4" : undefined,
          // Animate ONLY the test path; the always-on journey stays calm.
          animation: onPath ? "rre-edge-dash 0.6s linear infinite" : undefined,
          filter: selected
            ? "drop-shadow(0 0 3px rgba(14,165,164,0.85))"
            : undefined,
        }}
      />
      {!isStartEdge && (
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
                  removeEdge(id);
                }}
              >
                ×
              </button>
            )}
          </div>
        </EdgeLabelRenderer>
      )}
    </>
  );
}

export const LabeledEdge = memo(LabeledEdgeImpl);
