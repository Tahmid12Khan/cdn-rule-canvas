"use client";

// Screen-reader-accessible equivalent of the opaque React Flow SVG canvas
// (FRONTEND CONTRACT §7): an ordered, keyboard-navigable list of nodes/edges
// with an aria-live region announcing the node count. (Task 11)
import type { CanvasWorkingState } from "@/lib/canvas/serialize";

function nodeLabel(canvas: CanvasWorkingState["nodes"][number]): string {
  if (canvas.type === "decisionNode") {
    const p = canvas.data.processor;
    if (p.type === "meta_tags") {
      return `Decision · Meta Tags (${p.tag_name || "unconfigured"})`;
    }
    return `Decision · Device Type (${p.operator} ${p.value})`;
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
