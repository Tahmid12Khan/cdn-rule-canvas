"use client";

// Left-panel change list (version-diff-compare spec §7). Numbered globally
// 1..N in display order (canvas order anonymous→registered→customer; within a
// canvas: node changes then edge changes). Each item is git-diff coloured and,
// when clicked, asks the parent to pan/centre the matching canvas on the node /
// edge midpoint. Modified nodes list their changed fields with old→new values.
import { useNodeTypes } from "@/hooks/useNodeTypes";
import { nodeTitle } from "@/lib/canvas/manifest";
import { statusTextClass, STATUS_SYMBOL } from "@/components/canvas/compare/diffStyles";
import { CANVAS_KEYS } from "@/state/ruleBuilderStore";
import type { CanvasDiff, FieldChange, NodeDiff, RuleGraphDiff } from "@/lib/canvas/diff";
import type { GraphNode } from "@/lib/api/ruleGraph";
import type { NodeTypeSpec } from "@/lib/api/nodeTypes";
import type { CanvasKey } from "@/lib/canvas/types";

const CANVAS_LABELS: Record<CanvasKey, string> = {
  anonymous: "Anonymous",
  registered: "Registered",
  customer: "Customer",
};

// Rough node-centre in flow coords for setCenter (node wrappers are ~100×60).
function nodeCenter(node: GraphNode): { x: number; y: number } {
  return { x: node.position.x + 50, y: node.position.y + 30 };
}

function fmtValue(field: string, value: unknown, spec: NodeTypeSpec | undefined): string {
  if (value === null || value === undefined || value === "") return "(none)";
  const f = spec?.fields.find((x) => x.name === field);
  if (f?.control === "select") {
    const opt = f.options?.find((o) => o.value === value);
    if (opt) return opt.label;
  }
  return String(value);
}

function fieldLabel(field: string, spec: NodeTypeSpec | undefined): string {
  if (field === "type") return "Type";
  if (field === "custom_label") return "Custom label";
  if (field === "kind") return "Kind";
  return spec?.fields.find((x) => x.name === field)?.label ?? field;
}

interface ChangeListProps {
  diff: RuleGraphDiff;
  onSelect: (
    canvasKey: CanvasKey,
    nodeId: string | null,
    point: { x: number; y: number },
  ) => void;
}

export function ChangeList({ diff, onSelect }: ChangeListProps) {
  const { specByKind } = useNodeTypes();

  const labelOf = (node: GraphNode): string => {
    if (node.kind === "start") return "Start";
    if (node.kind === "end") return "END";
    const cfg = node.kind === "decision" ? node.processor : node.action;
    const custom = node.kind === "expression" ? node.custom_label?.trim() : undefined;
    return custom || nodeTitle(specByKind(cfg.type), cfg);
  };

  const totalChanges = CANVAS_KEYS.reduce((sum, k) => sum + diff[k].changeCount, 0);

  // Global running counter assigned while rendering, in display order.
  let counter = 0;

  return (
    <aside
      className="w-72 shrink-0 overflow-y-auto border-r border-border bg-bg-elevated p-4"
      aria-label="Change list"
    >
      <h3 className="mb-1 text-sm font-semibold text-nav">Changes</h3>
      <p className="mb-3 text-xs text-fg-muted">
        {totalChanges === 0
          ? "No differences (positions ignored)."
          : `${totalChanges} change${totalChanges === 1 ? "" : "s"} across canvases.`}
      </p>

      {CANVAS_KEYS.map((key) => {
        const cd: CanvasDiff = diff[key];
        const nodeChanges = cd.nodes.filter((n) => n.status !== "unchanged");
        const edgeChanges = cd.edges.filter((e) => e.status !== "unchanged");
        const posById = new Map(cd.nodes.map((n) => [n.id, n.node.position]));

        return (
          <div key={key} className="mb-4">
            <h4 className="mb-1.5 text-[11px] font-bold uppercase tracking-wide text-fg-subtle">
              {CANVAS_LABELS[key]}
            </h4>

            {nodeChanges.length === 0 && edgeChanges.length === 0 && (
              <p className="text-xs italic text-fg-muted">No changes</p>
            )}

            <ul className="space-y-1.5">
              {nodeChanges.map((nd: NodeDiff) => {
                counter += 1;
                const n = counter;
                const spec =
                  nd.node.kind === "decision"
                    ? specByKind(nd.node.processor.type)
                    : nd.node.kind === "expression"
                      ? specByKind(nd.node.action.type)
                      : undefined;
                return (
                  <li key={nd.id}>
                    <button
                      type="button"
                      onClick={() => onSelect(key, nd.id, nodeCenter(nd.node))}
                      className="group flex w-full items-start gap-2 rounded-md px-2 py-1.5 text-left hover:bg-bg"
                    >
                      <span className="mt-0.5 inline-flex h-5 w-5 shrink-0 items-center justify-center rounded-full border border-border text-[10px] font-bold text-fg-muted group-hover:border-action group-hover:text-action">
                        {n}
                      </span>
                      <span className="min-w-0 flex-1">
                        <span className={`text-sm font-medium ${statusTextClass(nd.status)}`}>
                          <span aria-hidden className="mr-1 font-bold">
                            {STATUS_SYMBOL[nd.status]}
                          </span>
                          {labelOf(nd.node)}
                        </span>
                        {nd.status === "modified" && nd.changes && (
                          <ul className="mt-0.5 space-y-0.5">
                            {nd.changes.map((c: FieldChange) => (
                              <li
                                key={c.field}
                                className="truncate font-mono text-[11px] text-fg-muted"
                                title={`${fieldLabel(c.field, spec)}: ${fmtValue(c.field, c.old, spec)} → ${fmtValue(c.field, c.new, spec)}`}
                              >
                                {fieldLabel(c.field, spec)}: {fmtValue(c.field, c.old, spec)} → {fmtValue(c.field, c.new, spec)}
                              </li>
                            ))}
                          </ul>
                        )}
                      </span>
                    </button>
                  </li>
                );
              })}

              {edgeChanges.map((ed) => {
                counter += 1;
                const n = counter;
                const srcNode = cd.nodes.find((x) => x.id === ed.source)?.node;
                const tgtNode = cd.nodes.find((x) => x.id === ed.target)?.node;
                const srcPos = posById.get(ed.source);
                const tgtPos = posById.get(ed.target);
                const point =
                  srcPos && tgtPos
                    ? { x: (srcPos.x + tgtPos.x) / 2 + 50, y: (srcPos.y + tgtPos.y) / 2 + 30 }
                    : srcPos
                      ? { x: srcPos.x + 50, y: srcPos.y + 30 }
                      : { x: 0, y: 0 };
                const srcLabel = srcNode ? labelOf(srcNode) : ed.source;
                const tgtLabel = tgtNode ? labelOf(tgtNode) : ed.target;
                return (
                  <li key={ed.key}>
                    <button
                      type="button"
                      onClick={() => onSelect(key, null, point)}
                      className="group flex w-full items-start gap-2 rounded-md px-2 py-1.5 text-left hover:bg-bg"
                    >
                      <span className="mt-0.5 inline-flex h-5 w-5 shrink-0 items-center justify-center rounded-full border border-border text-[10px] font-bold text-fg-muted group-hover:border-action group-hover:text-action">
                        {n}
                      </span>
                      <span className={`min-w-0 flex-1 text-sm font-medium ${statusTextClass(ed.status)}`}>
                        <span aria-hidden className="mr-1 font-bold">
                          {STATUS_SYMBOL[ed.status]}
                        </span>
                        <span className="text-xs">
                          {srcLabel} → {tgtLabel}{" "}
                          <span className="uppercase opacity-70">({ed.branch})</span>
                        </span>
                      </span>
                    </button>
                  </li>
                );
              })}
            </ul>
          </div>
        );
      })}
    </aside>
  );
}
