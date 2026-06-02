"use client";

// Read-only diff expression (action) node: rounded rectangle, single in/out
// handle, coloured by diff status. Mirrors the editor's ExpressionNode visually
// but is store-free. For apply_outcome the summary falls back to the manifest
// field token (the outcome's title isn't resolvable across versions here); other
// actions use the generic manifest summary. A non-empty custom_label is shown.
import { memo } from "react";
import { Handle, Position, type NodeProps } from "reactflow";

import { useNodeTypes } from "@/hooks/useNodeTypes";
import { nodeSummary, nodeTitle } from "@/lib/canvas/manifest";
import { nodePlateClass } from "@/components/canvas/compare/diffStyles";
import type { DiffNodeData } from "@/components/canvas/compare/diffData";

function DiffExpressionNodeImpl({ data }: NodeProps<DiffNodeData>) {
  const { manifest, specByKind } = useNodeTypes();
  const node = data.graphNode;
  const action = node.kind === "expression" ? node.action : { type: "" };
  const customLabel =
    node.kind === "expression" ? node.custom_label?.trim() : undefined;
  const spec = specByKind(action.type);
  const title = customLabel || nodeTitle(spec, action);
  const summary = spec ? nodeSummary(spec, action, manifest?.display) : "";

  return (
    <div className="relative flex flex-col items-center" data-testid="diff-expression-node">
      <div
        className={[
          "relative z-10 min-w-[110px] max-w-[160px] rounded-md border-2 px-3 py-2 text-center shadow-md",
          nodePlateClass(data.status),
          data.focused ? "ring-4 ring-action ring-offset-2" : "",
        ].join(" ")}
      >
        <Handle id="in" type="target" position={Position.Top} className="!h-2 !w-2 !border !border-white !bg-fg-muted" />
        <span className="block truncate text-xs font-semibold uppercase tracking-wide text-fg" title={title}>
          {title}
        </span>
        {summary && (
          <span className="mt-0.5 block truncate font-mono text-[10px] text-fg-muted" title={summary}>
            {summary}
          </span>
        )}
        <Handle id="out" type="source" position={Position.Bottom} className="!h-2 !w-2 !border !border-white !bg-fg-muted" />
      </div>
    </div>
  );
}

export const DiffExpressionNode = memo(DiffExpressionNodeImpl);
