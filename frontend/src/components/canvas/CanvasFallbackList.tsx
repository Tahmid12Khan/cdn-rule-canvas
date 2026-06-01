"use client";

// Screen-reader-accessible equivalent of the opaque React Flow SVG canvas
// (FRONTEND CONTRACT §7): an ordered, keyboard-navigable list of nodes/edges
// with an aria-live region announcing the node count. (Task 11)
import type { CanvasWorkingState } from "@/lib/canvas/serialize";

// Compact, manifest-agnostic summary of a generic processor config: the kind
// plus its (non-type) field values. Stays correct for any node type without
// per-type code (the manifest labels are not available in this pure a11y path).
function processorSummary(processor: {
  type: string;
  [field: string]: unknown;
}): string {
  const parts = Object.entries(processor)
    .filter(([key]) => key !== "type")
    .map(([, value]) =>
      value === undefined || value === null || value === ""
        ? "unconfigured"
        : String(value),
    );
  const detail = parts.length > 0 ? ` (${parts.join(" ")})` : "";
  return `${processor.type}${detail}`;
}

function nodeLabel(canvas: CanvasWorkingState["nodes"][number]): string {
  if (canvas.type === "decisionNode") {
    return `Decision · ${processorSummary(canvas.data.processor)}`;
  }
  if (canvas.type === "outcomeNode") {
    return `Outcome · ${canvas.data.title || "untitled"}`;
  }
  return "Node";
}

interface CanvasFallbackListProps {
  canvas: CanvasWorkingState;
}

export function CanvasFallbackList({ canvas }: CanvasFallbackListProps) {
  return (
    <div className="sr-only" aria-live="polite">
      <p>{`Rule canvas contains ${canvas.nodes.length} nodes and ${canvas.edges.length} connections.`}</p>
      <ol>
        {canvas.nodes.map((n) => (
          <li key={n.id}>{nodeLabel(n)}</li>
        ))}
      </ol>
      <ul>
        {canvas.edges.map((e) => (
          <li key={e.id}>{`Connection ${e.source} → ${e.target} on ${
            e.sourceHandle ?? e.data?.branch ?? "yes"
          } branch`}</li>
        ))}
      </ul>
    </div>
  );
}
