// Data payloads carried on the read-only diff React Flow nodes/edges
// (version-diff-compare spec §8). Shared by DiffCanvas + the diff node/edge
// renderers. Must be `type` aliases (React Flow v12 constrains node/edge data to
// `Record<string, unknown>`, which interfaces don't satisfy).
import type { Edge, Node } from "@xyflow/react";

import type { Branch, GraphNode } from "@/lib/api/ruleGraph";
import type { EdgeStatus, NodeStatus } from "@/lib/canvas/diff";

export type DiffNodeData = {
  graphNode: GraphNode;
  status: NodeStatus;
  focused: boolean;
};

export type DiffEdgeData = {
  branch: Branch;
  status: EdgeStatus;
};

export type DiffRFNode = Node<DiffNodeData>;
export type DiffRFEdge = Edge<DiffEdgeData>;
