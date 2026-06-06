// Shared eval-result logic for the two test panels (TestPanel + UrlTestPanel).
// Both run a canvas through the proxy evaluator and must highlight the path +
// name the applied outcome IDENTICALLY — so the bug-prone bits (full-path
// highlight, matched-outcome resolution) live here, not duplicated per panel.
import { useCallback, useMemo, useRef } from "react";

import type { EvalResponse } from "@/lib/api/evalTest";
import type { RFEdge } from "@/lib/canvas/types";
import {
  useRuleBuilderStore,
  type TestHighlight,
} from "@/state/ruleBuilderStore";

// The FULL start→END highlight for a run (features-matched-spec §7). Built from
// the journey node sequence — every journey node id (incl. start + end), plus,
// for each consecutive journey pair, the live-canvas edge whose
// (source,target) matches. Falls back to the proxy's traversed_* sets when the
// journey is empty (older proxies / no journey).
export function fullPathHighlight(
  res: EvalResponse,
  edges: RFEdge[],
): TestHighlight {
  const reachedEnd =
    res.journey.length > 0 &&
    res.journey[res.journey.length - 1].kind === "end";

  if (res.journey.length === 0) {
    return {
      nodeIds: new Set(res.traversed_node_ids),
      edgeIds: new Set(res.traversed_edge_ids),
      outcomeNodeId: res.matched_node_id,
      deadEnd: !reachedEnd,
    };
  }

  const nodeIds = new Set(res.journey.map((s) => s.node_id));
  const edgeIds = new Set<string>();
  for (let i = 0; i + 1 < res.journey.length; i++) {
    const source = res.journey[i].node_id;
    const target = res.journey[i + 1].node_id;
    const edge = edges.find((e) => e.source === source && e.target === target);
    if (edge) edgeIds.add(edge.id);
  }
  return {
    nodeIds,
    edgeIds,
    outcomeNodeId: res.matched_node_id,
    deadEnd: !reachedEnd,
  };
}

// Owns the per-run highlight lifecycle: applies the full start→END highlight on
// a successful run, restores it when the Transformation Journey collapses
// (features-matched-spec §7), and clears it. The "full path" is remembered so
// stepping through the journey (which sets a single-node highlight) can restore
// it.
export function useEvalHighlight() {
  const setTestHighlight = useRuleBuilderStore((s) => s.setTestHighlight);
  const clearTestHighlight = useRuleBuilderStore((s) => s.clearTestHighlight);
  const fullPathRef = useRef<TestHighlight | null>(null);

  const applyResult = useCallback(
    (res: EvalResponse, edges: RFEdge[]) => {
      const hl = fullPathHighlight(res, edges);
      fullPathRef.current = hl;
      setTestHighlight(hl);
    },
    [setTestHighlight],
  );

  const restoreFullPath = useCallback(() => {
    if (fullPathRef.current) setTestHighlight(fullPathRef.current);
  }, [setTestHighlight]);

  const clear = useCallback(() => {
    clearTestHighlight();
    fullPathRef.current = null;
  }, [clearTestHighlight]);

  return { applyResult, restoreFullPath, clear };
}

// Resolve the result banner facts for a run: whether the matched path reached an
// END node (success — the terminal body IS the output, even with no
// apply_outcome) and the friendly title of the applied outcome (recovered from
// the terminal expression node's apply_outcome action on the live canvas).
export function useMatchedOutcome(
  data: EvalResponse | undefined,
  outcomeTitleById: (id: string) => string,
): { matchedTitle: string | null; reachedEnd: boolean } {
  const selected = useRuleBuilderStore((s) => s.selected);

  const reachedEnd = useMemo(() => {
    const j = data?.journey;
    return Boolean(j && j.length > 0 && j[j.length - 1].kind === "end");
  }, [data]);

  const matchedTitle = useMemo(() => {
    const matchedNodeId = data?.matched_node_id;
    if (!matchedNodeId) return null;
    const { canvases } = useRuleBuilderStore.getState();
    const node = canvases[selected].nodes.find((n) => n.id === matchedNodeId);
    if (node?.type !== "expressionNode") return null;
    const { action, outcomeTitle } = node.data;
    if (action.type !== "apply_outcome") return null;
    if (outcomeTitle) return outcomeTitle;
    const outcomeId = action.outcome_id;
    if (typeof outcomeId !== "string" || !outcomeId) return null;
    return outcomeTitleById(outcomeId);
  }, [data, selected, outcomeTitleById]);

  return { matchedTitle, reachedEnd };
}
