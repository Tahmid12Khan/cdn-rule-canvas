"use client";

// Decision node: blue diamond rendered via CSS rotation. The outer square is
// rotate(45deg); the diamond label content is counter-rotated (-45deg) so text
// stays upright. Two source handles (id="yes" / id="no") + one target handle.
//
// Backend-driven (spec Part E): the title shown ABOVE the diamond is the node
// type's manifest `label` (e.g. article_url -> "URL"), truncated with …
// and shown in full on hover. The hover tooltip lists every field's current
// value, the spec summary, and the output branches. All metadata comes from the
// node-type manifest (useNodeTypes) — there is NO per-type code here.
import { memo, useState } from "react";
import { Handle, Position, type NodeProps } from "reactflow";

import { useNodeTypes } from "@/hooks/useNodeTypes";
import { fieldDisplayValue, nodeTitle } from "@/lib/canvas/manifest";
import { useRuleBuilderStore } from "@/state/ruleBuilderStore";
import type { DecisionNodeData } from "@/lib/canvas/types";

function DecisionNodeImpl({ id, data, selected }: NodeProps<DecisionNodeData>) {
  const hasError = useRuleBuilderStore((s) => Boolean(s.nodeErrors[id]));
  const errorMsg = useRuleBuilderStore((s) => s.nodeErrors[id]);
  // Test-a-rule highlight (WS4): glow if on the traversed path, dim otherwise
  // while a test result is active.
  const onPath = useRuleBuilderStore(
    (s) => s.testHighlight?.nodeIds.has(id) ?? false,
  );
  const testActive = useRuleBuilderStore((s) => s.testHighlight !== null);
  const dimmed = testActive && !onPath;

  // Always-on journey baseline (req 1): nodes on any START -> outcome path get a
  // faint steady ring. Suppressed while a test result is active (the brighter
  // test highlight + dim-the-rest takes over). Reads the MEMOIZED journeyPath
  // (recomputed in the structural reducers) — a stable boolean, NOT per-node
  // BFS on every render/drag frame.
  const onJourney = useRuleBuilderStore((s) => s.journeyPath.nodeIds.has(id));
  const journeyBaseline = onJourney && !testActive;

  const { specByKind } = useNodeTypes();
  const processor = data.processor;
  const spec = specByKind(processor.type);
  const title = nodeTitle(spec, processor);

  // Lightweight hover popover (does NOT wrap the node in a portal/trigger that
  // would intercept React Flow drag/handle interactions — it is a sibling
  // overlay toggled purely by mouse enter/leave on the container).
  const [hovered, setHovered] = useState(false);

  return (
    <div
      className={["relative flex flex-col items-center", dimmed ? "opacity-30" : ""].join(
        " ",
      )}
      data-testid="decision-node"
      onMouseEnter={() => setHovered(true)}
      onMouseLeave={() => setHovered(false)}
    >
      {/* manifest label at the TOP of the node, IN FRONT of the diamond. The
          `relative z-20` is load-bearing: it lifts the title pill above the
          later-painted diamond body so a bigger diamond can't occlude the name
          ("name on top, in front" — req 4). */}
      <span
        className="relative z-20 mb-1 max-w-[120px] truncate rounded bg-bg-elevated/90 px-1.5 py-0.5 text-[10px] font-semibold uppercase tracking-wide text-action-700 shadow-sm"
        title={title}
        data-testid="decision-title"
      >
        {title}
      </span>

      <div className="relative z-0 h-28 w-28">
        {/* target handle (top). Kept connectable for ConnectionMode.Loose drag
            completion; visually de-emphasized since the edge floats to the
            closest border post-connect regardless of which handle it nominally
            attaches to. */}
        <Handle
          id="in"
          type="target"
          position={Position.Top}
          className="!h-2.5 !w-2.5 !border-2 !border-white !bg-action"
        />

        {/* rotated square = diamond (background plate, z-0) */}
        <div
          className={[
            "absolute inset-0 flex items-center justify-center rounded-md border-2 bg-action shadow-md transition-shadow",
            hasError ? "border-danger ring-2 ring-danger" : "border-action-700",
            selected ? "ring-2 ring-action-600" : "",
            onPath
              ? "ring-4 ring-brand-400 ring-offset-2"
              : journeyBaseline
                ? "ring-1 ring-brand-400/40"
                : "",
          ].join(" ")}
          style={{ transform: "rotate(45deg)" }}
        >
          {/* counter-rotated content column: each input field value on its own
              line, centered + truncated so it stays inside the diamond. The
              inscribed upright square of a 112px diamond is ~79px wide; we cap
              the column at 72px to keep lines off the corners. (req 4) */}
          <div
            className="absolute inset-0 flex items-center justify-center"
            style={{ transform: "rotate(45deg)" }}
          >
            <div
              className="flex max-w-[72px] flex-col items-center gap-0.5 text-center"
              style={{ transform: "rotate(-45deg)" }}
              data-testid="decision-values"
            >
              {spec?.fields.map((field) => (
                <span
                  key={field.name}
                  className="block max-w-[72px] truncate text-[9px] font-medium leading-tight text-white/85"
                  title={`${field.label}: ${fieldDisplayValue(field, processor)}`}
                >
                  {fieldDisplayValue(field, processor)}
                </span>
              ))}
            </div>
          </div>
        </div>

        {/* YES source handle (right) */}
        <Handle
          id="yes"
          type="source"
          position={Position.Right}
          className="!h-2.5 !w-2.5 !border-2 !border-white !bg-node-yes"
        />
        {/* NO source handle (bottom) */}
        <Handle
          id="no"
          type="source"
          position={Position.Bottom}
          className="!h-2.5 !w-2.5 !border-2 !border-white !bg-node-no"
        />
      </div>

      {hasError && (
        <span className="mt-1 max-w-[140px] truncate text-[10px] font-medium text-danger">
          {errorMsg}
        </span>
      )}

      {/* Hover tooltip. pointer-events-none so it never blocks drag/handles. */}
      {hovered && spec && (
        <div
          role="tooltip"
          data-testid="decision-tooltip"
          className="pointer-events-none absolute left-1/2 top-full z-50 mt-2 w-56 -translate-x-1/2 rounded-md border border-status-prevBg bg-bg-elevated p-3 text-left shadow-lg"
        >
          <p className="mb-1 text-xs font-semibold text-nav">{title}</p>
          <p className="mb-2 text-[11px] text-status-prevFg">{spec.summary}</p>
          <dl className="space-y-0.5">
            {spec.fields.map((field) => (
              <div key={field.name} className="flex justify-between gap-2">
                <dt className="text-[11px] text-status-prevFg">{field.label}:</dt>
                <dd className="truncate text-[11px] font-medium text-nav">
                  {fieldDisplayValue(field, processor)}
                </dd>
              </div>
            ))}
          </dl>
          <div className="mt-2 border-t border-status-prevBg pt-1.5">
            <p className="text-[10px] uppercase tracking-wide text-status-prev">
              Branches
            </p>
            <p className="text-[11px] text-nav">
              {spec.output.branches.map((b) => b.label).join(" · ")}
            </p>
          </div>
        </div>
      )}
    </div>
  );
}

export const DecisionNode = memo(DecisionNodeImpl);
