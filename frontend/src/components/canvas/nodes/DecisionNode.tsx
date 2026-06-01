"use client";

// Decision node: blue diamond rendered via CSS rotation. The outer square is
// rotate(45deg); the label content is counter-rotated (-45deg) so text stays
// upright. Two source handles (id="yes" / id="no") + one target handle. The
// processor sub-label (OPERATOR / value) renders above the diamond. (Tasks
// 11/12/13)
import { memo } from "react";
import { Handle, Position, type NodeProps } from "reactflow";

import { useRuleBuilderStore } from "@/state/ruleBuilderStore";
import type { DecisionNodeData, ProcessorConfig } from "@/lib/canvas/types";

// Human sub-label: "CONTAINS / true" (meta_tags) or "EQUALS / mobile" (device).
function processorSubLabel(p: ProcessorConfig): string {
  if (p.type === "meta_tags") {
    const op = p.operator.toUpperCase();
    return p.operator === "exists" ? op : `${op} / ${p.value ?? ""}`;
  }
  return `${p.operator.toUpperCase()} / ${p.value}`;
}

// Short title shown inside the diamond.
function processorTitle(p: ProcessorConfig): string {
  if (p.type === "meta_tags") return p.tag_name?.trim() ? p.tag_name : "Meta Tags";
  return "Device";
}

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
  const subLabel = processorSubLabel(data.processor);
  const title = processorTitle(data.processor);

  return (
    <div
      className={["flex flex-col items-center", dimmed ? "opacity-30" : ""].join(
        " ",
      )}
      data-testid="decision-node"
    >
      {/* processor sub-label above the diamond */}
      <span className="mb-1 max-w-[140px] truncate rounded bg-bg-elevated/90 px-1.5 py-0.5 text-[10px] font-semibold uppercase tracking-wide text-action-700 shadow-sm">
        {subLabel}
      </span>

      <div className="relative h-16 w-16">
        {/* target handle (top) */}
        <Handle
          id="in"
          type="target"
          position={Position.Top}
          className="!h-2.5 !w-2.5 !border-2 !border-white !bg-action"
        />

        {/* rotated square = diamond */}
        <div
          className={[
            "absolute inset-0 flex items-center justify-center rounded-md border-2 bg-action shadow-md transition-shadow",
            hasError ? "border-danger ring-2 ring-danger" : "border-action-700",
            selected ? "ring-2 ring-action-600" : "",
            onPath ? "ring-4 ring-brand-400 ring-offset-2" : "",
          ].join(" ")}
          style={{ transform: "rotate(45deg)" }}
        >
          {/* counter-rotated label so text is upright */}
          <span
            className="max-w-[48px] truncate px-1 text-center text-[10px] font-semibold leading-tight text-white"
            style={{ transform: "rotate(-45deg)" }}
          >
            {title}
          </span>
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
    </div>
  );
}

export const DecisionNode = memo(DecisionNodeImpl);
