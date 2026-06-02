"use client";

// Read-only diff decision node: the same rotated-square diamond as the editor's
// DecisionNode, but coloured by diff status (added/removed/modified/unchanged)
// instead of the live teal, and with NO store coupling (highlight/test/journey).
// Title + one-line summary come from the manifest helpers, identical to the
// editor, so a node reads the same in the diff as on the canvas.
import { memo } from "react";
import { Handle, Position, type NodeProps } from "reactflow";

import { useNodeTypes } from "@/hooks/useNodeTypes";
import { nodeSummary, nodeTitle } from "@/lib/canvas/manifest";
import { nodePlateClass } from "@/components/canvas/compare/diffStyles";
import type { DiffNodeData } from "@/components/canvas/compare/diffData";

function DiffDecisionNodeImpl({ data }: NodeProps<DiffNodeData>) {
  const { manifest, specByKind } = useNodeTypes();
  const node = data.graphNode;
  const processor = node.kind === "decision" ? node.processor : { type: "" };
  const spec = specByKind(processor.type);
  const title = nodeTitle(spec, processor);
  const summary = spec ? nodeSummary(spec, processor, manifest?.display) : "";

  return (
    <div className="relative flex flex-col items-center" data-testid="diff-decision-node">
      <span
        className="relative z-20 mb-1 max-w-[120px] truncate rounded border border-border bg-bg-elevated px-1.5 py-0.5 text-[10px] font-semibold uppercase tracking-wide text-fg shadow-sm"
        title={title}
      >
        {title}
      </span>

      <div className="relative z-0 h-20 w-20">
        <Handle id="in" type="target" position={Position.Top} className="!h-2 !w-2 !border !border-white !bg-fg-muted" />
        <div
          className={[
            "absolute inset-0 rounded-md border-2 shadow-md",
            nodePlateClass(data.status),
            data.focused ? "ring-4 ring-action ring-offset-2" : "",
          ].join(" ")}
          style={{ transform: "rotate(45deg)" }}
        />
        <Handle id="yes" type="source" position={Position.Right} className="!h-2 !w-2 !border !border-white !bg-node-yes" />
        <Handle id="no" type="source" position={Position.Bottom} className="!h-2 !w-2 !border !border-white !bg-node-no" />

        {summary && (
          <span className="pointer-events-none absolute inset-0 z-30 flex items-center justify-center px-3 text-center">
            <span className="max-w-[64px] truncate font-mono text-[10px] font-semibold leading-tight text-fg drop-shadow" title={summary}>
              {summary}
            </span>
          </span>
        )}
      </div>
    </div>
  );
}

export const DiffDecisionNode = memo(DiffDecisionNodeImpl);
