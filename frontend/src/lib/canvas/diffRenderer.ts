// Pure, dependency-free git-style line diff (spec items 2 & 3). Extracted from
// TransformationJourney so the diff engine + hunk grouping are reusable (e.g.
// the combined Start→End diff, and any future DiffView consumer) and trivially
// unit-testable: no React, no store, no manifest. Mirrors the canvas graph
// diff's pure-JS LCS approach (lib/canvas/diff.ts) — deliberately NO npm dep.

// One line of a git-style diff: `ctx` = unchanged context, `add`/`del` = a line
// present only in the new/old body.
export type DiffLine = { type: "add" | "del" | "ctx"; text: string };

// A collapsed run of `hidden` unchanged lines (git's "@@ … @@" gap).
export type DiffRow = DiffLine | { type: "gap"; hidden: number };

// A maximal run of changed (add/del) lines plus up to `pad` context lines on
// either side, addressed by its position in the FULL DiffLine list. `startLine`
// /`endLine` are inclusive indices into the `lines` array passed to groupHunks,
// so a consumer can map a hunk back to rendered rows for scroll/nav.
export type DiffHunk = {
  startLine: number;
  endLine: number;
  lines: DiffLine[];
};

// LCS line diff of two strings — no external dep.
export function diffLines(before: string, after: string): DiffLine[] {
  const a = before.split("\n");
  const b = after.split("\n");
  const m = a.length;
  const k = b.length;
  const lcs: number[][] = Array.from({ length: m + 1 }, () =>
    new Array<number>(k + 1).fill(0),
  );
  for (let i = m - 1; i >= 0; i--) {
    for (let j = k - 1; j >= 0; j--) {
      lcs[i][j] =
        a[i] === b[j]
          ? lcs[i + 1][j + 1] + 1
          : Math.max(lcs[i + 1][j], lcs[i][j + 1]);
    }
  }
  const out: DiffLine[] = [];
  let i = 0;
  let j = 0;
  while (i < m && j < k) {
    if (a[i] === b[j]) {
      out.push({ type: "ctx", text: a[i] });
      i++;
      j++;
    } else if (lcs[i + 1][j] >= lcs[i][j + 1]) {
      out.push({ type: "del", text: a[i] });
      i++;
    } else {
      out.push({ type: "add", text: b[j] });
      j++;
    }
  }
  while (i < m) out.push({ type: "del", text: a[i++] });
  while (j < k) out.push({ type: "add", text: b[j++] });
  return out;
}

// Collapse runs of unchanged context to `pad` lines around each change, so a
// one-line edit in a big body reads like a git hunk, not a wall of text.
export function collapseContext(lines: DiffLine[], pad = 3): DiffRow[] {
  const keep = new Array<boolean>(lines.length).fill(false);
  lines.forEach((line, idx) => {
    if (line.type === "ctx") return;
    const lo = Math.max(0, idx - pad);
    const hi = Math.min(lines.length - 1, idx + pad);
    for (let p = lo; p <= hi; p++) keep[p] = true;
  });
  const rows: DiffRow[] = [];
  let hidden = 0;
  lines.forEach((line, idx) => {
    if (keep[idx]) {
      if (hidden > 0) {
        rows.push({ type: "gap", hidden });
        hidden = 0;
      }
      rows.push(line);
    } else {
      hidden++;
    }
  });
  if (hidden > 0) rows.push({ type: "gap", hidden });
  return rows;
}

// Group a diff into git-style hunks: each hunk is a maximal run of changed
// (add/del) lines plus up to `pad` context lines on either side. Adjacent
// changes whose padded windows touch or overlap MERGE into one hunk (matching
// `collapseContext`'s gap behaviour). An all-context diff has zero hunks; an
// all-changed diff is a single hunk. Used to drive previous/next-change
// navigation independent of the collapsed/expanded render mode.
export function groupHunks(lines: DiffLine[], pad = 3): DiffHunk[] {
  // Indices of changed lines.
  const changed: number[] = [];
  lines.forEach((line, idx) => {
    if (line.type !== "ctx") changed.push(idx);
  });
  if (changed.length === 0) return [];

  const hunks: DiffHunk[] = [];
  let runStart = changed[0];
  let runEnd = changed[0];
  for (let c = 1; c < changed.length; c++) {
    const idx = changed[c];
    // Two changes belong to the same hunk if their padded windows touch: the
    // gap of context between them is <= 2*pad (pad trailing + pad leading).
    if (idx - runEnd <= 2 * pad + 1) {
      runEnd = idx;
    } else {
      hunks.push(makeHunk(lines, runStart, runEnd, pad));
      runStart = idx;
      runEnd = idx;
    }
  }
  hunks.push(makeHunk(lines, runStart, runEnd, pad));
  return hunks;
}

function makeHunk(
  lines: DiffLine[],
  changeStart: number,
  changeEnd: number,
  pad: number,
): DiffHunk {
  const startLine = Math.max(0, changeStart - pad);
  const endLine = Math.min(lines.length - 1, changeEnd + pad);
  return { startLine, endLine, lines: lines.slice(startLine, endLine + 1) };
}
