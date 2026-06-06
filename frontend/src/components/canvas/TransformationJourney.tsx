"use client";

// Transformation Journey (expression-nodes-spec §5 + features-matched-spec
// §7/§8). COLLAPSIBLE, default COLLAPSED:
//   - Collapsed → the canvas shows the FULL start→end highlight (owned by
//     TestPanel); the body is hidden (header + step count + Expand affordance).
//   - Expanded → a node-by-node stepper. Big clickable Prev/Next arrows (≥40px
//     target) and ArrowLeft/ArrowRight keys — the keys only move the step when
//     the journey container (or a child) is focused, so we don't hijack arrows
//     globally. As you step, the current node glows on the canvas via
//     setTestHighlight. Collapsing again restores the full-path highlight.
// Each step shows BOTH technical info (label, kind + node_id, the node's inputs
// as "Field label = value", the result body, and per-node time) AND a plain-
// English "what was done" sentence (describeStep).
//
// Client component: owns the current-step index + collapsed flag (useState) +
// keyboard handler.
import { useEffect, useState } from "react";

import { useNodeTypes } from "@/hooks/useNodeTypes";
import type { JourneyStep } from "@/lib/api/evalTest";
import { describeStep } from "@/lib/canvas/describeStep";
import { fieldDisplayValue } from "@/lib/canvas/manifest";
import type { ProcessorConfig, RFNode } from "@/lib/canvas/types";
import { useRuleBuilderStore } from "@/state/ruleBuilderStore";

interface TransformationJourneyProps {
  journey: JourneyStep[];
  // Feature content kind: "json" pretty-prints body_after as JSON; "html" shows
  // the raw string.
  featureType: "html" | "json";
  // The live (selected) canvas nodes, so each step can look up the node's
  // config (decision processor / expression action) by node_id for the inputs +
  // plain-English description.
  canvasNodes: RFNode[];
  // Restore the FULL start→end highlight (called when collapsing). Set by
  // TestPanel from the journey node sequence.
  onRestoreFullPath: () => void;
}

// Pretty-print a step's body_after. JSON features stringify the value (objects,
// arrays, primitives) with 2-space indent; HTML features show the raw string
// as-is (or stringify a non-string fallback defensively).
function formatBody(value: unknown, isJson: boolean): string {
  if (isJson) {
    try {
      return JSON.stringify(value, null, 2);
    } catch {
      return String(value);
    }
  }
  return typeof value === "string" ? value : JSON.stringify(value, null, 2);
}

// One line of a git-style diff: `ctx` = unchanged context, `add`/`del` = a line
// present only in the new/old body.
type DiffLine = { type: "add" | "del" | "ctx"; text: string };
// A collapsed run of `hidden` unchanged lines (git's "@@ … @@" gap).
type DiffRow = DiffLine | { type: "gap"; hidden: number };

// LCS line diff of two strings — no external dep, mirroring the canvas graph
// diff's pure-JS approach (lib/canvas/diff.ts).
function diffLines(before: string, after: string): DiffLine[] {
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
function collapseContext(lines: DiffLine[], pad = 3): DiffRow[] {
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

// The live canvas node's config for a step: a decision's processor or an
// expression's action (both ProcessorConfig); undefined for start/end or an
// unknown node id.
function configForNode(node: RFNode | undefined): ProcessorConfig | undefined {
  if (node?.type === "decisionNode") return node.data.processor;
  if (node?.type === "expressionNode") return node.data.action;
  return undefined;
}

export function TransformationJourney({
  journey,
  featureType,
  canvasNodes,
  onRestoreFullPath,
}: TransformationJourneyProps) {
  const isJson = featureType === "json";
  const setTestHighlight = useRuleBuilderStore((s) => s.setTestHighlight);
  const { specByKind } = useNodeTypes();

  const [collapsed, setCollapsed] = useState(true);
  const [index, setIndex] = useState(0);

  // A fresh journey (new run) resets to the first step and re-collapses so the
  // canvas shows the full start→end highlight again.
  useEffect(() => {
    setIndex(0);
    setCollapsed(true);
  }, [journey]);

  // Glow the current step's node on the canvas while EXPANDED; when collapsed,
  // restore the full start→end highlight (owned by TestPanel).
  const currentNodeId = journey[index]?.node_id ?? null;
  useEffect(() => {
    if (collapsed) {
      onRestoreFullPath();
      return;
    }
    if (!currentNodeId) return;
    setTestHighlight({
      nodeIds: new Set([currentNodeId]),
      edgeIds: new Set(),
      outcomeNodeId: currentNodeId,
      deadEnd: false,
    });
  }, [collapsed, currentNodeId, setTestHighlight, onRestoreFullPath]);

  if (journey.length === 0) return null;

  const n = journey.length;
  // Clamp defensively — index can lag a step behind a shrinking journey.
  const clamped = Math.min(Math.max(index, 0), n - 1);
  const step = journey[clamped];
  const atStart = clamped <= 0;
  const atEnd = clamped >= n - 1;

  function go(delta: number) {
    setIndex((i) => Math.min(Math.max(i + delta, 0), n - 1));
  }

  function handleKeyDown(e: React.KeyboardEvent<HTMLDivElement>) {
    if (collapsed) return;
    if (e.key === "ArrowRight") {
      e.preventDefault();
      go(1);
    } else if (e.key === "ArrowLeft") {
      e.preventDefault();
      go(-1);
    }
  }

  const branchLabel =
    step.branch === null ? null : step.branch ? "yes" : "no";

  // Look up the live canvas node + its config for the inputs + description.
  const node = canvasNodes.find((c) => c.id === step.node_id);
  const config = configForNode(node);
  const spec = config ? specByKind(config.type) : undefined;
  // apply_outcome shows the resolved outcome title (the action stores only the
  // outcome_id); the title is re-resolved on deserialize and cached on the node.
  const outcomeTitle =
    node?.type === "expressionNode" ? node.data.outcomeTitle : undefined;
  const description = describeStep(
    step.kind,
    config,
    step.branch,
    outcomeTitle ?? step.label,
  );

  // Git-style diff of the body vs the previous step (skipped on the first step,
  // which has no predecessor). Decisions/start/end don't mutate the body, so
  // their diff is all context → we show a "no change" note instead.
  const prevStep = clamped > 0 ? journey[clamped - 1] : undefined;
  const bodyDiff = prevStep
    ? diffLines(
        formatBody(prevStep.body_after, isJson),
        formatBody(step.body_after, isJson),
      )
    : null;
  const bodyChanged = bodyDiff?.some((l) => l.type !== "ctx") ?? false;
  const diffRows = bodyDiff ? collapseContext(bodyDiff) : [];

  return (
    <section
      aria-label="Transformation Journey"
      tabIndex={0}
      onKeyDown={handleKeyDown}
      className="mt-4 rounded-lg border border-status-prevBg bg-bg-elevated p-4 shadow-sm focus:outline-none focus-visible:ring-2 focus-visible:ring-brand-500"
    >
      <div className="flex items-center justify-between">
        <h4 className="text-sm font-semibold text-nav">
          Transformation Journey
        </h4>
        <div className="flex items-center gap-3">
          <span className="text-xs font-medium text-status-prevFg">
            {collapsed
              ? `${n} ${n === 1 ? "step" : "steps"}`
              : `Step ${clamped + 1} of ${n}`}
          </span>
          <button
            type="button"
            onClick={() => setCollapsed((c) => !c)}
            aria-expanded={!collapsed}
            className="text-xs font-medium text-action-600 hover:text-action-700"
          >
            {collapsed ? "Expand" : "Collapse"}
          </button>
        </div>
      </div>

      {collapsed ? (
        <p className="mt-1 text-xs text-status-prevFg">
          The full path from Start to END is highlighted on the canvas. Expand
          to walk it node by node.
        </p>
      ) : (
        <>
          <p className="mt-1 text-xs text-status-prevFg">
            Walk the matched path with ← / → or the arrows. The current node
            glows on the canvas.
          </p>

          <div className="mt-3 flex items-center gap-3">
            <button
              type="button"
              onClick={() => go(-1)}
              disabled={atStart}
              aria-label="Previous node"
              className="flex h-10 w-10 shrink-0 items-center justify-center rounded-md border border-status-prevBg text-lg font-bold text-nav hover:bg-status-prevBg disabled:cursor-not-allowed disabled:opacity-40"
            >
              ←
            </button>

            <div className="min-w-0 flex-1 text-center">
              <p
                className="truncate text-sm font-semibold text-brand-700"
                title={step.label}
              >
                {step.label}
              </p>
              <p className="truncate font-mono text-[11px] text-status-prevFg">
                <span className="uppercase tracking-wide">{step.kind}</span>
                {" · "}
                <span title={step.node_id}>{step.node_id}</span>
                {branchLabel !== null && (
                  <span className="ml-1 font-semibold text-nav">
                    → {branchLabel}
                  </span>
                )}
                {" · "}
                <span data-testid="journey-time">{step.time_ms} ms</span>
              </p>
            </div>

            <button
              type="button"
              onClick={() => go(1)}
              disabled={atEnd}
              aria-label="Next node"
              className="flex h-10 w-10 shrink-0 items-center justify-center rounded-md border border-status-prevBg text-lg font-bold text-nav hover:bg-status-prevBg disabled:cursor-not-allowed disabled:opacity-40"
            >
              →
            </button>
          </div>

          {/* Non-technical: plain-English "what was done". */}
          <p
            data-testid="journey-description"
            className="mt-3 rounded-md bg-bg p-2 text-xs text-nav"
          >
            {description}
          </p>

          {/* Technical: the node's inputs as "Field label = value". */}
          {config && spec && spec.fields.length > 0 && (
            <div className="mt-3">
              <p className="mb-1 text-xs font-medium text-nav">Inputs</p>
              <dl
                data-testid="journey-inputs"
                className="space-y-0.5 rounded-md border border-status-prevBg bg-bg p-2"
              >
                {spec.fields.map((field) => (
                  <div
                    key={field.name}
                    className="flex justify-between gap-2 text-[11px]"
                  >
                    <dt className="text-status-prevFg">{field.label}</dt>
                    <dd className="truncate font-mono text-nav">
                      {field.control === "outcome_select"
                        ? outcomeTitle || "—"
                        : fieldDisplayValue(field, config)}
                    </dd>
                  </div>
                ))}
              </dl>
            </div>
          )}

          {prevStep && (
            <div className="mt-3">
              <p className="mb-1 text-xs font-medium text-nav">
                Changes from previous step
              </p>
              {bodyChanged ? (
                <div
                  data-testid="journey-diff"
                  className="max-h-64 overflow-auto rounded-md border border-status-prevBg bg-bg font-mono text-[11px] leading-relaxed"
                >
                  {diffRows.map((row, i) =>
                    row.type === "gap" ? (
                      <div
                        key={i}
                        className="select-none bg-bg-elevated px-3 py-0.5 text-center text-fg-muted"
                      >
                        ⋯ {row.hidden} unchanged{" "}
                        {row.hidden === 1 ? "line" : "lines"}
                      </div>
                    ) : (
                      <div
                        key={i}
                        className={
                          row.type === "add"
                            ? "whitespace-pre-wrap break-words bg-status-liveBg px-3 text-status-liveFg"
                            : row.type === "del"
                              ? "whitespace-pre-wrap break-words bg-danger-bg px-3 text-danger"
                              : "whitespace-pre-wrap break-words px-3 text-fg-muted"
                        }
                      >
                        <span className="select-none">
                          {row.type === "add"
                            ? "+ "
                            : row.type === "del"
                              ? "− "
                              : "  "}
                        </span>
                        {row.text}
                      </div>
                    ),
                  )}
                </div>
              ) : (
                <p
                  data-testid="journey-diff"
                  className="rounded-md border border-status-prevBg bg-bg p-3 text-[11px] text-status-prevFg"
                >
                  This node didn&apos;t change the body.
                </p>
              )}
            </div>
          )}

          <div className="mt-3">
            <p className="mb-1 text-xs font-medium text-nav">
              Body after this node
            </p>
            <pre
              data-testid="journey-body"
              className="max-h-64 overflow-auto whitespace-pre-wrap break-words rounded-md border border-status-prevBg bg-bg p-3 font-mono text-[11px] leading-relaxed text-nav"
            >
              {formatBody(step.body_after, isJson)}
            </pre>
          </div>
        </>
      )}
    </section>
  );
}
