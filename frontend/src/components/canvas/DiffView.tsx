"use client";

// Reusable git-like diff editor (spec item 3). Self-contained so any consumer
// (the Transformation Journey's per-node + combined diffs today; a Phase-2
// reuse tomorrow) gets the same UX: an Expanded/Collapsed toggle and
// previous/next-change navigation across hunks. Owns LOCAL state per instance —
// two DiffViews on screen navigate independently.
//
// Client component: owns collapsed-mode + current-hunk-index (useState) and a
// scoped keyboard handler. Keyboard hunk-nav is Ctrl+ArrowUp/Down so it does
// NOT collide with the journey's ArrowLeft/Right step-nav.
import { useMemo, useRef, useState } from "react";

import {
  collapseContext,
  diffLines,
  groupHunks,
  type DiffLine,
  type DiffRow,
} from "@/lib/canvas/diffRenderer";

interface DiffViewProps {
  before: string;
  after: string;
  title?: string;
}

const PAD = 3;

// A render row carrying the original DiffLine index (for gap rows, the index of
// the next visible line) so we can map a hunk → its first rendered row for
// scrollIntoView. Built alongside the rows so collapsed + expanded share code.
type RenderRow = { row: DiffRow; lineIndex: number };

// Collapsed render: collapseContext output, re-annotated with original indices.
function collapsedRows(lines: DiffLine[]): RenderRow[] {
  const rows = collapseContext(lines, PAD);
  const out: RenderRow[] = [];
  let cursor = 0;
  for (const row of rows) {
    if (row.type === "gap") {
      // The gap sits before the next visible line; point it there.
      out.push({ row, lineIndex: cursor });
      cursor += row.hidden;
    } else {
      out.push({ row, lineIndex: cursor });
      cursor += 1;
    }
  }
  return out;
}

// Expanded render: every line, no gaps.
function expandedRows(lines: DiffLine[]): RenderRow[] {
  return lines.map((row, lineIndex) => ({ row, lineIndex }));
}

export function DiffView({ before, after, title }: DiffViewProps) {
  // Memoize the O(mn) diff + hunk grouping on the endpoint strings.
  const lines = useMemo(() => diffLines(before, after), [before, after]);
  const hunks = useMemo(() => groupHunks(lines, PAD), [lines]);
  const changed = useMemo(() => lines.some((l) => l.type !== "ctx"), [lines]);

  const [collapsedMode, setCollapsedMode] = useState(true);
  const [currentHunkIndex, setCurrentHunkIndex] = useState(0);

  const containerRef = useRef<HTMLDivElement>(null);
  // Row index (within the rendered list) → DOM node, so we can scroll the row
  // whose original line index starts the targeted hunk into view.
  const rowRefs = useRef<Map<number, HTMLDivElement>>(new Map());

  const rows = useMemo(
    () => (collapsedMode ? collapsedRows(lines) : expandedRows(lines)),
    [collapsedMode, lines],
  );

  const hunkCount = hunks.length;

  function scrollToHunk(hunkIndex: number) {
    const hunk = hunks[hunkIndex];
    if (!hunk) return;
    // First rendered row at or after the hunk's start line.
    let targetRowIdx = rows.findIndex((r) => r.lineIndex >= hunk.startLine);
    if (targetRowIdx < 0) targetRowIdx = rows.length - 1;
    rowRefs.current
      .get(targetRowIdx)
      ?.scrollIntoView({ block: "nearest" });
  }

  function step(delta: number) {
    if (hunkCount === 0) return;
    const next = Math.min(Math.max(currentHunkIndex + delta, 0), hunkCount - 1);
    setCurrentHunkIndex(next);
    scrollToHunk(next);
  }

  function handleKeyDown(e: React.KeyboardEvent<HTMLDivElement>) {
    // Ctrl-scoped so it doesn't collide with the journey's ArrowLeft/Right.
    if (!e.ctrlKey) return;
    if (e.key === "ArrowDown") {
      e.preventDefault();
      step(1);
    } else if (e.key === "ArrowUp") {
      e.preventDefault();
      step(-1);
    }
  }

  if (!changed) {
    return (
      <p
        data-testid="diff-view"
        className="rounded-md border border-status-prevBg bg-bg p-3 text-[11px] text-status-prevFg"
      >
        No changes.
      </p>
    );
  }

  return (
    <div
      ref={containerRef}
      data-testid="diff-view"
      tabIndex={0}
      onKeyDown={handleKeyDown}
      className="rounded-md border border-status-prevBg bg-bg focus:outline-none focus-visible:ring-2 focus-visible:ring-brand-500"
    >
      <div className="flex flex-wrap items-center justify-between gap-2 border-b border-status-prevBg px-3 py-1.5">
        <span className="truncate text-xs font-medium text-nav">
          {title ?? "Diff"}
        </span>
        <div className="flex items-center gap-3">
          {hunkCount > 0 && (
            <div className="flex items-center gap-1">
              <button
                type="button"
                onClick={() => step(-1)}
                disabled={currentHunkIndex <= 0}
                aria-label="Previous change"
                className="flex h-6 w-6 items-center justify-center rounded border border-status-prevBg text-sm font-bold text-nav hover:bg-status-prevBg disabled:cursor-not-allowed disabled:opacity-40"
              >
                ◀
              </button>
              <span
                data-testid="diff-hunk-counter"
                className="select-none text-[11px] tabular-nums text-status-prevFg"
              >
                change {currentHunkIndex + 1} of {hunkCount}
              </span>
              <button
                type="button"
                onClick={() => step(1)}
                disabled={currentHunkIndex >= hunkCount - 1}
                aria-label="Next change"
                className="flex h-6 w-6 items-center justify-center rounded border border-status-prevBg text-sm font-bold text-nav hover:bg-status-prevBg disabled:cursor-not-allowed disabled:opacity-40"
              >
                ▶
              </button>
            </div>
          )}
          <button
            type="button"
            onClick={() => setCollapsedMode((c) => !c)}
            aria-pressed={!collapsedMode}
            className="text-[11px] font-medium text-action-600 hover:text-action-700"
          >
            {collapsedMode ? "Expanded" : "Collapsed"}
          </button>
        </div>
      </div>

      <div className="max-h-64 overflow-auto font-mono text-[11px] leading-relaxed">
        {rows.map(({ row }, i) =>
          row.type === "gap" ? (
            <div
              key={i}
              ref={(el) => {
                if (el) rowRefs.current.set(i, el);
                else rowRefs.current.delete(i);
              }}
              data-testid="diff-gap"
              className="select-none bg-bg-elevated px-3 py-0.5 text-center text-fg-muted"
            >
              ⋯ {row.hidden} unchanged {row.hidden === 1 ? "line" : "lines"}
            </div>
          ) : (
            <div
              key={i}
              ref={(el) => {
                if (el) rowRefs.current.set(i, el);
                else rowRefs.current.delete(i);
              }}
              className={
                row.type === "add"
                  ? "whitespace-pre-wrap break-words bg-status-liveBg px-3 text-status-liveFg"
                  : row.type === "del"
                    ? "whitespace-pre-wrap break-words bg-danger-bg px-3 text-danger"
                    : "whitespace-pre-wrap break-words px-3 text-fg-muted"
              }
            >
              <span className="select-none">
                {row.type === "add" ? "+ " : row.type === "del" ? "− " : "  "}
              </span>
              {row.text}
            </div>
          ),
        )}
      </div>
    </div>
  );
}
