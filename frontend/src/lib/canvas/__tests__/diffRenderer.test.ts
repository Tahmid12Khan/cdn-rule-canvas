import { describe, expect, it } from "vitest";

import {
  collapseContext,
  diffLines,
  groupHunks,
  type DiffLine,
} from "@/lib/canvas/diffRenderer";

// Build a DiffLine[] tersely: "c"=ctx, "+"=add, "-"=del followed by text.
function lines(...specs: string[]): DiffLine[] {
  return specs.map((s) => {
    const tag = s[0];
    const text = s.slice(1);
    if (tag === "+") return { type: "add", text };
    if (tag === "-") return { type: "del", text };
    return { type: "ctx", text };
  });
}

describe("diffLines", () => {
  it("marks added, removed and unchanged lines via LCS", () => {
    const out = diffLines("a\nb\nc", "a\nB\nc");
    expect(out).toEqual([
      { type: "ctx", text: "a" },
      { type: "del", text: "b" },
      { type: "add", text: "B" },
      { type: "ctx", text: "c" },
    ]);
  });

  it("an unchanged string is all context", () => {
    const out = diffLines("x\ny", "x\ny");
    expect(out.every((l) => l.type === "ctx")).toBe(true);
  });
});

describe("groupHunks", () => {
  it("all-context diff produces zero hunks", () => {
    const ls = lines("ca", "cb", "cc");
    expect(groupHunks(ls, 3)).toEqual([]);
  });

  it("all-changed diff is a single hunk spanning every line", () => {
    const ls = lines("-a", "+A", "-b", "+B");
    const hunks = groupHunks(ls, 3);
    expect(hunks).toHaveLength(1);
    expect(hunks[0].startLine).toBe(0);
    expect(hunks[0].endLine).toBe(ls.length - 1);
    expect(hunks[0].lines).toEqual(ls);
  });

  it("a single isolated change yields one padded hunk", () => {
    // 0..9 context with a single change at index 5.
    const ls = lines(
      "c0",
      "c1",
      "c2",
      "c3",
      "c4",
      "+x",
      "c6",
      "c7",
      "c8",
      "c9",
    );
    const hunks = groupHunks(ls, 2);
    expect(hunks).toHaveLength(1);
    // pad=2 → window [3, 7].
    expect(hunks[0].startLine).toBe(3);
    expect(hunks[0].endLine).toBe(7);
    expect(hunks[0].lines).toHaveLength(5);
  });

  it("two changes far apart produce two separate hunks", () => {
    const ls = lines(
      "+a", // 0
      "c1",
      "c2",
      "c3",
      "c4",
      "c5",
      "c6",
      "c7",
      "c8",
      "c9",
      "+b", // 10
    );
    const hunks = groupHunks(ls, 2);
    expect(hunks).toHaveLength(2);
    expect(hunks[0].startLine).toBe(0);
    expect(hunks[1].endLine).toBe(10);
  });

  it("two changes whose padded windows touch merge into one hunk", () => {
    // changes at 0 and 4, pad=2 → gap (3 ctx lines) <= 2*pad+1 → merge.
    const ls = lines("+a", "c1", "c2", "c3", "+b");
    const hunks = groupHunks(ls, 2);
    expect(hunks).toHaveLength(1);
    expect(hunks[0].startLine).toBe(0);
    expect(hunks[0].endLine).toBe(4);
  });

  it("hunk windows clamp to the array bounds", () => {
    const ls = lines("+a", "c1", "c2");
    const hunks = groupHunks(ls, 5);
    expect(hunks[0].startLine).toBe(0);
    expect(hunks[0].endLine).toBe(2);
  });
});

describe("collapseContext", () => {
  it("collapses far context into a gap row carrying the hidden count", () => {
    const ls = lines(
      "c0",
      "c1",
      "c2",
      "c3",
      "c4",
      "c5",
      "+x",
      "c7",
      "c8",
      "c9",
      "c10",
      "c11",
    );
    const rows = collapseContext(ls, 1);
    const gap = rows.find((r) => r.type === "gap");
    expect(gap).toBeDefined();
    if (gap?.type === "gap") expect(gap.hidden).toBeGreaterThan(0);
  });

  it("keeps everything when there is no far context", () => {
    const ls = lines("ca", "+b", "cc");
    const rows = collapseContext(ls, 3);
    expect(rows.some((r) => r.type === "gap")).toBe(false);
  });
});
