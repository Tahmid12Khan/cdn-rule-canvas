"use client";

// Transformation Journey (expression-nodes-spec §5). A node-by-node stepper shown
// after a successful "Test a rule" run. It walks the matched path and, at each
// step, renders the body AFTER that node's action (JSON.stringify(…, null, 2) in a
// <pre> for JSON features; the raw string for HTML). Navigation: big clickable
// Prev/Next arrows (≥40px target) and ArrowLeft/ArrowRight keys — but the keys
// only move the step when the journey container (or a child) is focused, so we
// don't hijack arrows globally. As you step, the current node glows on the canvas
// via setTestHighlight.
//
// Client component: owns the current-step index (useState) + keyboard handler.
import { useEffect, useState } from "react";

import type { JourneyStep } from "@/lib/api/evalTest";
import { useRuleBuilderStore } from "@/state/ruleBuilderStore";

interface TransformationJourneyProps {
  journey: JourneyStep[];
  // Feature content kind: "json" pretty-prints body_after as JSON; "html" shows
  // the raw string.
  featureType: "html" | "json";
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

export function TransformationJourney({
  journey,
  featureType,
}: TransformationJourneyProps) {
  const isJson = featureType === "json";
  const setTestHighlight = useRuleBuilderStore((s) => s.setTestHighlight);

  const [index, setIndex] = useState(0);

  // A fresh journey (new run) resets to the first step. Keyed on length +
  // identity of the first node so a re-run of the same path still resets.
  useEffect(() => {
    setIndex(0);
  }, [journey]);

  // Glow the current step's node on the canvas as the user steps through.
  const currentNodeId = journey[index]?.node_id ?? null;
  useEffect(() => {
    if (!currentNodeId) return;
    setTestHighlight({
      nodeIds: new Set([currentNodeId]),
      edgeIds: new Set(),
      outcomeNodeId: currentNodeId,
      deadEnd: false,
    });
  }, [currentNodeId, setTestHighlight]);

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

  return (
    <section
      aria-label="Transformation Journey"
      tabIndex={0}
      onKeyDown={handleKeyDown}
      className="mt-4 rounded-lg border border-status-prevBg bg-bg-elevated p-4 shadow-sm focus:outline-none focus-visible:ring-2 focus-visible:ring-brand-500"
    >
      <div className="flex items-center justify-between">
        <h4 className="text-sm font-semibold text-nav">Transformation Journey</h4>
        <span className="text-xs font-medium text-status-prevFg">
          Step {clamped + 1} of {n}
        </span>
      </div>

      <p className="mt-1 text-xs text-status-prevFg">
        Walk the matched path with ← / → or the arrows. The current node glows on
        the canvas.
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
          <p className="truncate text-sm font-semibold text-brand-700" title={step.label}>
            {step.label}
          </p>
          <p className="truncate font-mono text-[11px] text-status-prevFg">
            <span className="uppercase tracking-wide">{step.kind}</span>
            {" · "}
            <span title={step.node_id}>{step.node_id}</span>
            {branchLabel !== null && (
              <span className="ml-1 font-semibold text-nav">→ {branchLabel}</span>
            )}
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
    </section>
  );
}
