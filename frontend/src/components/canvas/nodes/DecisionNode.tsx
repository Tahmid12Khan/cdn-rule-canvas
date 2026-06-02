"use client";

// Decision node: teal diamond (CSS rotate(45deg)). One target handle + two
// source handles (yes/no). Backend-driven (spec Part E): the title ABOVE the
// diamond is the manifest `label`; the one-line condition summary BELOW the
// diamond is built from server display rules — operator SYMBOLS and the value
// truncation length come from the manifest, the client only renders them. The
// hover tooltip still lists each field's full (untruncated) value.
import { memo, useState } from "react";
import { Handle, Position, type NodeProps } from "@xyflow/react";

import { useNodeTypes } from "@/hooks/useNodeTypes";
import { fieldDisplayValue, nodeSummary, nodeTitle } from "@/lib/canvas/manifest";
import { useRuleBuilderStore } from "@/state/ruleBuilderStore";
import type { RFDecisionNode } from "@/lib/canvas/types";

function DecisionNodeImpl({ id, data, selected }: NodeProps<RFDecisionNode>) {
  const hasError = useRuleBuilderStore((s) => Boolean(s.nodeErrors[id]));
  const errorMsg = useRuleBuilderStore((s) => s.nodeErrors[id]);
  const onPath = useRuleBuilderStore(
    (s) => s.testHighlight?.nodeIds.has(id) ?? false,
  );
  const testActive = useRuleBuilderStore((s) => s.testHighlight !== null);
  const dimmed = testActive && !onPath;
  const onJourney = useRuleBuilderStore((s) => s.journeyPath.nodeIds.has(id));
  const journeyBaseline = onJourney && !testActive;

  const { manifest, specByKind } = useNodeTypes();
  const processor = data.processor;
  const spec = specByKind(processor.type);
  const title = nodeTitle(spec, processor);
  const summary = spec ? nodeSummary(spec, processor, manifest?.display) : "";

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
      {/* manifest label ABOVE the diamond. Opaque, high-contrast pill (light text
          on an elevated surface) so the name never blends into the teal body
          ("name on top, in front" — req 4). z-20 keeps it above the diamond. */}
      <span
        className="relative z-20 mb-1 max-w-[120px] truncate rounded border border-border bg-bg-elevated px-1.5 py-0.5 text-[10px] font-semibold uppercase tracking-wide text-fg shadow-sm"
        title={title}
        data-testid="decision-title"
      >
        {title}
      </span>

      <div className="relative z-0 h-20 w-20">
        {/* target handle (top) */}
        <Handle
          id="in"
          type="target"
          position={Position.Top}
          className="!h-2.5 !w-2.5 !border-2 !border-white !bg-action"
        />

        {/* rotated square = diamond (clean shape; the condition renders below) */}
        <div
          className={[
            "absolute inset-0 rounded-md border-2 bg-action shadow-md transition-shadow",
            hasError ? "border-danger ring-2 ring-danger" : "border-action-700",
            selected ? "ring-2 ring-action-600" : "",
            onPath
              ? "ring-4 ring-brand-400 ring-offset-2"
              : journeyBaseline
                ? "ring-1 ring-brand-400/40"
                : "",
          ].join(" ")}
          style={{ transform: "rotate(45deg)" }}
        />

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

        {/* one-line condition summary CENTERED IN FRONT of the diamond: the
            operator as its server-defined symbol + the (truncated) value.
            z-30 keeps it above the diamond body; pointer-events-none so it
            never blocks dragging or the handles underneath. */}
        {summary && (
          <span className="pointer-events-none absolute inset-0 z-30 flex items-center justify-center px-3 text-center">
            <span
              className="max-w-[64px] truncate font-mono text-[10px] font-semibold leading-tight text-white drop-shadow"
              title={summary}
              data-testid="decision-summary"
            >
              {summary}
            </span>
          </span>
        )}
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
