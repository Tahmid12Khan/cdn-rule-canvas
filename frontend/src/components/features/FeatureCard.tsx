"use client";

// Feature card (FRONTEND CONTRACT §2.2, Task 06). Now a client component
// (spec item 8): besides linking to the version list it shows the execution
// number and lets the user edit it directly. The backend uses a preserve-gaps
// model (auto-assign on create, reject duplicates), NOT auto-renumber — so a
// ±1 stepper would always collide with the contiguous neighbour. Instead we
// expose a direct number input the user types into and commits on Enter/blur,
// PATCHing the typed target order. The control lives inside the card link, so
// its handlers stopPropagation / preventDefault to avoid triggering navigation.
import { useEffect, useState } from "react";
import Link from "next/link";
import { useMutation, useQueryClient } from "@tanstack/react-query";

import { ApiError } from "@/lib/api/client";
import { type FeatureRead, updateFeature } from "@/lib/api/features";

const TYPE_BADGE: Record<FeatureRead["type"], string> = {
  html: "bg-brand-50 text-accent-onMuted",
  json: "bg-action-600/10 text-action-700",
};

const TYPE_LABEL: Record<FeatureRead["type"], string> = {
  html: "HTML",
  json: "JSON",
};

export function FeatureCard({ feature }: { feature: FeatureRead }) {
  const href = `/products/features/${feature.type}/${feature.id}`;
  const hasLive = feature.live_version_id !== null;
  const hasStaging = feature.staging_version_id !== null;

  const queryClient = useQueryClient();
  const [conflict, setConflict] = useState<string | null>(null);
  // Local draft of the order input. Kept as a string so the field can be
  // cleared mid-edit; resynced if the feature's order changes underneath us.
  const [draft, setDraft] = useState(String(feature.execution_order));

  useEffect(() => {
    setDraft(String(feature.execution_order));
  }, [feature.execution_order]);

  const reorder = useMutation({
    mutationFn: (execution_order: number) =>
      updateFeature(feature.id, { execution_order }),
    onMutate: () => setConflict(null),
    onSuccess: () => {
      void queryClient.invalidateQueries({ queryKey: ["features"] });
    },
    onError: (err: unknown, execution_order) => {
      if (err instanceof ApiError && err.code === "EXECUTION_ORDER_CONFLICT") {
        setConflict(
          `Order ${execution_order} is already used by another ${TYPE_LABEL[feature.type]} rule`,
        );
        return;
      }
      setConflict("Could not change the order. Try again.");
    },
  });

  // The order input lives inside the <Link>; suppress navigation on interaction.
  function stop(event: React.SyntheticEvent) {
    event.preventDefault();
    event.stopPropagation();
  }

  // Commit only on explicit Enter / blur — never per keystroke. No-op when the
  // value is unchanged, empty, or invalid.
  function commit() {
    if (reorder.isPending) return;
    const trimmed = draft.trim();
    const next = Number(trimmed);
    if (trimmed === "" || !Number.isInteger(next) || next < 0) {
      setDraft(String(feature.execution_order));
      setConflict(null);
      return;
    }
    if (next === feature.execution_order) {
      setConflict(null);
      return;
    }
    reorder.mutate(next);
  }

  return (
    <Link
      href={href}
      className="group flex flex-col gap-3 rounded-lg border border-border bg-bg-elevated p-5 transition-colors hover:border-accent"
    >
      <div className="flex items-start justify-between gap-3">
        <h3 className="text-base font-semibold text-nav group-hover:text-brand-700">
          {feature.name}
        </h3>
        <div className="flex shrink-0 items-center gap-2">
          <label
            className="inline-flex items-center gap-1 rounded-full bg-brand-50 pl-2 pr-1 text-xs font-semibold text-accent-onMuted"
            title="Execution order (lowest runs first). Type a number and press Enter."
            onClick={stop}
          >
            <span aria-hidden>#</span>
            <span className="sr-only">Execution order</span>
            <input
              type="number"
              min={0}
              inputMode="numeric"
              value={draft}
              disabled={reorder.isPending}
              aria-label="Execution order"
              className="w-10 rounded-full bg-transparent px-1 py-0.5 text-center text-xs font-semibold text-accent-onMuted focus:bg-bg-elevated focus:outline-none focus:ring-1 focus:ring-brand-500 disabled:opacity-60"
              onChange={(e) => setDraft(e.target.value)}
              onClick={stop}
              onKeyDown={(e) => {
                if (e.key === "Enter") {
                  stop(e);
                  commit();
                  e.currentTarget.blur();
                }
              }}
              onBlur={commit}
            />
          </label>
          <span
            className={`rounded-full px-2 py-0.5 text-xs font-semibold uppercase ${TYPE_BADGE[feature.type]}`}
          >
            {feature.type}
          </span>
        </div>
      </div>

      <code className="truncate font-mono text-xs text-fg-muted">
        {feature.id}
      </code>

      <div className="mt-1 flex items-center gap-2 text-xs">
        <span
          className={
            hasLive
              ? "inline-flex items-center gap-1 rounded-full bg-status-liveBg px-2 py-0.5 font-medium text-status-liveFg"
              : "inline-flex items-center gap-1 rounded-full border border-status-draftBorder px-2 py-0.5 font-medium text-status-draft"
          }
        >
          <span
            className={`inline-block h-1.5 w-1.5 rounded-full ${hasLive ? "bg-status-live" : "bg-status-draft"}`}
            aria-hidden
          />
          {hasLive ? "Live" : "No live version"}
        </span>
        {hasStaging && (
          <span className="inline-flex items-center gap-1 rounded-full bg-status-stagingBg px-2 py-0.5 font-medium text-status-stagingFg">
            <span
              className="inline-block h-1.5 w-1.5 rounded-full bg-status-staging"
              aria-hidden
            />
            Staging
          </span>
        )}
      </div>

      {conflict && (
        <p role="alert" className="text-xs font-medium text-danger">
          {conflict}
        </p>
      )}
    </Link>
  );
}
