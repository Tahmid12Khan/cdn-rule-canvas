"use client";

// Full-screen Compare modal (version-diff-compare spec §6). Base = the version
// the user is on; they pick another version to compare against. Computes a pure
// rule-graph diff and renders the left change list + the three diff canvases.
// Clicking a change-list item pans/centres the matching canvas on the node.
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import * as Dialog from "@radix-ui/react-dialog";
import { useQuery } from "@tanstack/react-query";
import type { ReactFlowInstance } from "reactflow";

import { ChangeList } from "@/components/canvas/compare/ChangeList";
import { DiffCanvas } from "@/components/canvas/compare/DiffCanvas";
import { ErrorBanner } from "@/components/ui/ErrorBanner";
import { getVersion, type VersionRead } from "@/lib/api/canvasVersions";
import { listVersions } from "@/lib/api/versions";
import { diffRuleGraph } from "@/lib/canvas/diff";
import { CANVAS_KEYS } from "@/state/ruleBuilderStore";
import type { CanvasKey } from "@/lib/canvas/types";

const CANVAS_LABELS: Record<CanvasKey, string> = {
  anonymous: "Anonymous",
  registered: "Registered",
  customer: "Customer",
};

interface CompareDialogProps {
  fid: string;
  currentVersion: VersionRead;
  open: boolean;
  onOpenChange: (open: boolean) => void;
}

export function CompareDialog({
  fid,
  currentVersion,
  open,
  onOpenChange,
}: CompareDialogProps) {
  const [compareVnum, setCompareVnum] = useState<number | null>(null);
  const [swap, setSwap] = useState(false);
  const [focused, setFocused] = useState<{ canvas: CanvasKey; nodeId: string | null }>(
    { canvas: "anonymous", nodeId: null },
  );

  const rfInstances = useRef<Partial<Record<CanvasKey, ReactFlowInstance>>>({});
  const sectionRefs = useRef<Partial<Record<CanvasKey, HTMLElement | null>>>({});

  // All other versions of this feature, newest first.
  const { data: versionsPage, isLoading: versionsLoading } = useQuery({
    queryKey: ["versions", fid, "compare-picker"],
    queryFn: () => listVersions(fid, { page_size: 100 }),
    enabled: open,
  });

  const otherVersions = useMemo(() => {
    const items = (versionsPage?.items ?? []).filter(
      (v) => v.version_number !== currentVersion.version_number,
    );
    return items.sort((a, b) => b.version_number - a.version_number);
  }, [versionsPage, currentVersion.version_number]);

  // Default the picker to the previous version (largest vnum < current) once.
  useEffect(() => {
    if (compareVnum !== null || otherVersions.length === 0) return;
    const prev = otherVersions.find(
      (v) => v.version_number < currentVersion.version_number,
    );
    setCompareVnum((prev ?? otherVersions[0]).version_number);
  }, [otherVersions, compareVnum, currentVersion.version_number]);

  const {
    data: otherVersion,
    isLoading: otherLoading,
    isError: otherError,
  } = useQuery({
    queryKey: ["version", fid, compareVnum],
    queryFn: () => getVersion(fid, compareVnum as number),
    enabled: open && compareVnum !== null,
  });

  // swap=false → base(current) is NEW, picked is OLD (green = added since picked).
  const newVersion = swap ? otherVersion : currentVersion;
  const oldVersion = swap ? currentVersion : otherVersion;

  const diff = useMemo(
    () =>
      oldVersion && newVersion
        ? diffRuleGraph(oldVersion.rule_graph, newVersion.rule_graph)
        : null,
    [oldVersion, newVersion],
  );

  const onSelect = useCallback(
    (canvasKey: CanvasKey, nodeId: string | null, point: { x: number; y: number }) => {
      setFocused({ canvas: canvasKey, nodeId });
      sectionRefs.current[canvasKey]?.scrollIntoView({ behavior: "smooth", block: "start" });
      rfInstances.current[canvasKey]?.setCenter(point.x, point.y, {
        zoom: 1.2,
        duration: 400,
      });
    },
    [],
  );

  const pairKey = `${oldVersion?.version_number}-${newVersion?.version_number}`;

  return (
    <Dialog.Root open={open} onOpenChange={onOpenChange}>
      <Dialog.Portal>
        <Dialog.Overlay className="fixed inset-0 z-40 bg-black/60 backdrop-blur-sm" />
        <Dialog.Content className="fixed inset-3 z-50 flex flex-col overflow-hidden rounded-xl bg-bg shadow-xl focus:outline-none">
          {/* Header */}
          <div className="flex flex-wrap items-center justify-between gap-3 border-b border-border bg-bg-elevated px-5 py-3">
            <div className="flex items-center gap-3">
              <Dialog.Title className="text-base font-semibold text-nav">
                Compare versions
              </Dialog.Title>
              <span className="text-sm text-fg-muted">
                {oldVersion && newVersion
                  ? `v${oldVersion.version_number} (${oldVersion.status}) → v${newVersion.version_number} (${newVersion.status})`
                  : "Pick a version to compare"}
              </span>
            </div>

            <div className="flex items-center gap-2">
              <label className="text-xs text-fg-muted" htmlFor="compare-with">
                Base v{currentVersion.version_number} · compare against
              </label>
              <select
                id="compare-with"
                value={compareVnum ?? ""}
                onChange={(e) => setCompareVnum(Number(e.target.value))}
                disabled={versionsLoading || otherVersions.length === 0}
                className="rounded-md border border-border bg-bg px-2 py-1 text-sm text-fg focus:outline-none focus:ring-2 focus:ring-action"
              >
                {compareVnum === null && <option value="">Select…</option>}
                {otherVersions.map((v) => (
                  <option key={v.version_number} value={v.version_number}>
                    v{v.version_number} · {v.status}
                  </option>
                ))}
              </select>
              <button
                type="button"
                onClick={() => setSwap((s) => !s)}
                title="Swap which version is old vs new"
                className="rounded-md border border-border px-2 py-1 text-sm text-fg hover:bg-bg-elevated"
              >
                ⇄ Swap
              </button>

              {/* Legend */}
              <div className="ml-2 hidden items-center gap-3 text-xs sm:flex">
                <span className="text-status-liveFg">+ added</span>
                <span className="text-danger">− removed</span>
                <span className="text-status-stagingFg">~ modified</span>
              </div>

              <Dialog.Close asChild>
                <button
                  type="button"
                  aria-label="Close compare"
                  className="ml-2 rounded-md border border-border px-2.5 py-1 text-sm font-medium text-fg hover:bg-bg-elevated"
                >
                  Close
                </button>
              </Dialog.Close>
            </div>
          </div>

          <Dialog.Description className="sr-only">
            Side-by-side rule-graph comparison of two feature versions.
          </Dialog.Description>

          {/* Body */}
          <div className="flex min-h-0 flex-1">
            {versionsLoading && (
              <div className="flex flex-1 items-center justify-center text-sm text-fg-muted">
                Loading versions…
              </div>
            )}

            {!versionsLoading && otherVersions.length === 0 && (
              <div className="flex flex-1 items-center justify-center text-sm text-fg-muted">
                No other versions to compare against.
              </div>
            )}

            {!versionsLoading && otherVersions.length > 0 && (
              <>
                {diff && <ChangeList diff={diff} onSelect={onSelect} />}

                <div className="min-h-0 flex-1 overflow-y-auto p-5">
                  {otherError && (
                    <ErrorBanner message={`Could not load version ${compareVnum}.`} />
                  )}
                  {otherLoading && !diff && (
                    <div className="flex h-40 items-center justify-center text-sm text-fg-muted">
                      Loading version {compareVnum}…
                    </div>
                  )}

                  {diff &&
                    CANVAS_KEYS.map((key) => (
                      <section
                        key={key}
                        ref={(el) => {
                          sectionRefs.current[key] = el;
                        }}
                        className="mb-6 scroll-mt-4"
                      >
                        <h3 className="mb-2 flex items-center gap-2 text-sm font-bold uppercase tracking-wide text-nav">
                          {CANVAS_LABELS[key]}
                          <span className="rounded-full bg-bg-elevated px-2 py-0.5 text-[11px] font-medium text-fg-muted">
                            {diff[key].changeCount} change
                            {diff[key].changeCount === 1 ? "" : "s"}
                          </span>
                        </h3>
                        <DiffCanvas
                          key={`${key}-${pairKey}`}
                          diff={diff[key]}
                          focusedNodeId={focused.canvas === key ? focused.nodeId : null}
                          onReady={(inst) => {
                            rfInstances.current[key] = inst;
                          }}
                        />
                      </section>
                    ))}
                </div>
              </>
            )}
          </div>
        </Dialog.Content>
      </Dialog.Portal>
    </Dialog.Root>
  );
}
