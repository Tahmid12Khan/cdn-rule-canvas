"use client";

// Expression (action) node (expression-nodes-spec §1/§5): performs ONE body
// action and passes through (single in + single out handle). The title is the
// manifest `label` for the action kind; the one-line summary is built from the
// server display rules for trim_json/add_attribute, and is the outcome title for
// apply_outcome. Purple rounded rectangle to distinguish action nodes from the
// teal decision diamond.
import { memo, useState } from "react";
import { Handle, Position, type NodeProps } from "reactflow";

import { useNodeTypes } from "@/hooks/useNodeTypes";
import { fieldDisplayValue, nodeSummary, nodeTitle } from "@/lib/canvas/manifest";
import { useRuleBuilderStore } from "@/state/ruleBuilderStore";
import type { ExpressionNodeData } from "@/lib/canvas/types";

function ExpressionNodeImpl({ id, data, selected }: NodeProps<ExpressionNodeData>) {
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
  const action = data.action;
  const spec = specByKind(action.type);
  const title = nodeTitle(spec, action);

  // apply_outcome shows the saved outcome's title (resolved on deserialize);
  // other actions show the generic manifest summary (json_path/length, etc.).
  const summary =
    action.type === "apply_outcome"
      ? data.outcomeTitle || "Pick an outcome…"
      : spec
        ? nodeSummary(spec, action, manifest?.display)
        : "";

  const [hovered, setHovered] = useState(false);

  return (
    <div
      className={["relative flex flex-col items-center", dimmed ? "opacity-30" : ""].join(
        " ",
      )}
      data-testid="expression-node"
      onMouseEnter={() => setHovered(true)}
      onMouseLeave={() => setHovered(false)}
    >
      <div
        className={[
          "relative z-10 min-w-[110px] max-w-[160px] rounded-md border-2 bg-node-action px-3 py-2 text-center shadow-md transition-opacity",
          hasError ? "border-danger ring-2 ring-danger" : "border-white/30",
          selected ? "ring-2 ring-brand-500" : "",
          onPath
            ? "ring-4 ring-brand-400 ring-offset-2"
            : journeyBaseline
              ? "ring-1 ring-brand-400/40"
              : "",
        ].join(" ")}
      >
        {/* single in handle (top) */}
        <Handle
          id="in"
          type="target"
          position={Position.Top}
          className="!h-2.5 !w-2.5 !border-2 !border-white !bg-action"
        />

        <span
          className="block truncate text-xs font-semibold uppercase tracking-wide text-white"
          title={title}
          data-testid="expression-title"
        >
          {title}
        </span>
        {summary && (
          <span
            className="mt-0.5 block truncate font-mono text-[10px] text-white/80"
            title={summary}
            data-testid="expression-summary"
          >
            {summary}
          </span>
        )}

        {/* single out handle (bottom) — passthrough */}
        <Handle
          id="out"
          type="source"
          position={Position.Bottom}
          className="!h-2.5 !w-2.5 !border-2 !border-white !bg-action"
        />
      </div>

      {hasError && (
        <span className="mt-1 max-w-[160px] truncate text-[10px] font-medium text-danger">
          {errorMsg}
        </span>
      )}

      {hovered && spec && (
        <div
          role="tooltip"
          data-testid="expression-tooltip"
          className="pointer-events-none absolute left-1/2 top-full z-50 mt-2 w-56 -translate-x-1/2 rounded-md border border-status-prevBg bg-bg-elevated p-3 text-left shadow-lg"
        >
          <p className="mb-1 text-xs font-semibold text-nav">{title}</p>
          <p className="mb-2 text-[11px] text-status-prevFg">{spec.summary}</p>
          <dl className="space-y-0.5">
            {spec.fields.map((field) => (
              <div key={field.name} className="flex justify-between gap-2">
                <dt className="text-[11px] text-status-prevFg">{field.label}:</dt>
                <dd className="truncate text-[11px] font-medium text-nav">
                  {field.control === "outcome_select"
                    ? data.outcomeTitle || "—"
                    : fieldDisplayValue(field, action)}
                </dd>
              </div>
            ))}
          </dl>
        </div>
      )}
    </div>
  );
}

export const ExpressionNode = memo(ExpressionNodeImpl);
