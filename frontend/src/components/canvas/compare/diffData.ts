// Data payloads carried on the read-only diff React Flow nodes/edges
// (version-diff-compare spec §8). Shared by DiffCanvas + the diff node/edge
// renderers.
import type { Branch, GraphNode } from "@/lib/api/ruleGraph";
import type { EdgeStatus, NodeStatus } from "@/lib/canvas/diff";

export interface DiffNodeData {
  graphNode: GraphNode;
  status: NodeStatus;
  focused: boolean;
}

export interface DiffEdgeData {
  branch: Branch;
  status: EdgeStatus;
}
