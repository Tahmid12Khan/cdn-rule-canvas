// Floating-edge geometry (spec §4 req 5). Pure, no React — given a target node's
// bounding rect and the source anchor point, pick the CLOSEST side of the target
// and return the border attach point + the corresponding React Flow Position.
//
// This is unit-testable in isolation: the React layer (LabeledEdge) reads the
// node geometry from the RF store and feeds it here, then passes the result to
// getBezierPath so the edge meets the nearest side of the destination.
import { Position } from "reactflow";

export interface NodeRect {
  // Top-left origin (React Flow positionAbsolute) + measured size.
  x: number;
  y: number;
  w: number;
  h: number;
}

export interface FloatingTarget {
  tx: number;
  ty: number;
  tPos: Position;
}

// Choose the target border point closest to (sourceX, sourceY). Uses the
// ratio-of-half-extents test: if the horizontal offset dominates relative to
// the box width, attach to the left/right side, otherwise top/bottom. The
// attach point sits at the midpoint of the chosen side, and `tPos` matches so
// the bezier curve leaves/enters perpendicular to that border.
export function closestSide(
  target: NodeRect,
  sourceX: number,
  sourceY: number,
): FloatingTarget {
  const cx = target.x + target.w / 2;
  const cy = target.y + target.h / 2;
  const dx = sourceX - cx;
  const dy = sourceY - cy;

  // Guard against a zero-size rect (unmeasured): fall back to top-center.
  const w = target.w || 1;
  const h = target.h || 1;

  // Compare normalized offsets: |dx|/w vs |dy|/h decides the dominant axis.
  if (Math.abs(dx) / w >= Math.abs(dy) / h) {
    // Left/right side.
    if (dx > 0) {
      return { tx: target.x + target.w, ty: cy, tPos: Position.Right };
    }
    return { tx: target.x, ty: cy, tPos: Position.Left };
  }
  // Top/bottom side.
  if (dy > 0) {
    return { tx: cx, ty: target.y + target.h, tPos: Position.Bottom };
  }
  return { tx: cx, ty: target.y, tPos: Position.Top };
}
