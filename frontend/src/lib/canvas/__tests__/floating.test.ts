import { Position } from "@xyflow/react";
import { describe, expect, it } from "vitest";

import { closestSide, type NodeRect } from "@/lib/canvas/floating";

// A 100x100 target node whose top-left is at (100, 100) -> center (150, 150).
const rect: NodeRect = { x: 100, y: 100, w: 100, h: 100 };

describe("closestSide (floating edge geometry)", () => {
  it("attaches to the RIGHT side when the source is to the right", () => {
    const r = closestSide(rect, 400, 150);
    expect(r.tPos).toBe(Position.Right);
    expect(r.tx).toBe(200); // x + w
    expect(r.ty).toBe(150); // center y
  });

  it("attaches to the LEFT side when the source is to the left", () => {
    const r = closestSide(rect, -100, 150);
    expect(r.tPos).toBe(Position.Left);
    expect(r.tx).toBe(100); // x
    expect(r.ty).toBe(150);
  });

  it("attaches to the TOP side when the source is above", () => {
    const r = closestSide(rect, 150, -100);
    expect(r.tPos).toBe(Position.Top);
    expect(r.tx).toBe(150); // center x
    expect(r.ty).toBe(100); // y
  });

  it("attaches to the BOTTOM side when the source is below", () => {
    const r = closestSide(rect, 150, 500);
    expect(r.tPos).toBe(Position.Bottom);
    expect(r.tx).toBe(150);
    expect(r.ty).toBe(200); // y + h
  });

  it("picks the dominant axis on a diagonal (mostly-horizontal -> a side)", () => {
    // Far to the right, only slightly down -> horizontal dominates -> Right.
    const r = closestSide(rect, 600, 180);
    expect(r.tPos).toBe(Position.Right);
  });

  it("does not crash on a zero-size (unmeasured) rect", () => {
    const r = closestSide({ x: 0, y: 0, w: 0, h: 0 }, 50, 0);
    expect(Object.values(Position)).toContain(r.tPos);
    expect(Number.isFinite(r.tx)).toBe(true);
    expect(Number.isFinite(r.ty)).toBe(true);
  });

  it("attaches to the DIAMOND region, not the title pill, for a decision node", () => {
    // A decision node's full measured box is wider/taller than the diamond: a
    // 24px title pill sits above the 112px diamond (mirrors LabeledEdge's
    // DIAMOND_SIZE / TITLE_PILL_H inset). Box at (0,0), 140 wide x 160 tall.
    const DIAMOND_SIZE = 112;
    const TITLE_PILL_H = 24;
    const box: NodeRect = { x: 0, y: 0, w: 140, h: 160 };
    const inset: NodeRect = {
      x: box.x + (box.w - DIAMOND_SIZE) / 2, // 14
      y: box.y + TITLE_PILL_H, // 24
      w: DIAMOND_SIZE,
      h: DIAMOND_SIZE,
    };
    // Source far above the node -> TOP side of the DIAMOND (y=24), NOT the box
    // top (y=0) which would land in the title-pill gap above the diamond.
    const r = closestSide(inset, 70, -400);
    expect(r.tPos).toBe(Position.Top);
    expect(r.ty).toBe(24); // diamond top, below the title pill
    expect(r.tx).toBe(70); // diamond center x (14 + 112/2)
  });
});
