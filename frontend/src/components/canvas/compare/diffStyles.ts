// Git-diff palette shared by the diff nodes, edges, and change list
// (version-diff-compare spec §10). added=green (status-live), removed=red
// (danger), modified=amber (status-staging), unchanged=neutral. No React.
import type { EdgeStatus, NodeStatus } from "@/lib/canvas/diff";

export const STATUS_SYMBOL: Record<NodeStatus, string> = {
  added: "+",
  removed: "−",
  modified: "~",
  unchanged: "·",
};

// Border + background tint for a node plate, keyed by diff status.
export function nodePlateClass(status: NodeStatus): string {
  switch (status) {
    case "added":
      return "border-status-live bg-status-liveBg ring-1 ring-status-live/40";
    case "removed":
      return "border-danger bg-danger-bg border-dashed opacity-70 ring-1 ring-danger/40";
    case "modified":
      return "border-status-staging bg-status-stagingBg ring-1 ring-status-staging/40";
    default:
      return "border-border bg-bg-elevated";
  }
}

// Marker text color for the change-list bullet / status glyph.
export function statusTextClass(status: NodeStatus | EdgeStatus): string {
  switch (status) {
    case "added":
      return "text-status-liveFg";
    case "removed":
      return "text-danger";
    case "modified":
      return "text-status-stagingFg";
    default:
      return "text-fg-muted";
  }
}

// Edge stroke color (hex — React Flow stroke is an inline style).
export function edgeStroke(status: EdgeStatus): string {
  switch (status) {
    case "added":
      return "#16a34a"; // status.live
    case "removed":
      return "#ef4444"; // danger-ish red
    default:
      return "#6b7280"; // node.no gray (neutral)
  }
}
