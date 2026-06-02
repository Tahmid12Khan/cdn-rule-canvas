"use client";

// Inline outcome list on the version detail page (FRONTEND CONTRACT §2.4). Lists
// the version's outcomes with Edit (→ transformation route) and, for
// non-builtin outcomes, Clone / Delete. Add Outcome creates a new outcome.
// Reuses the canvas-subtree's minimal outcomes API to avoid a collision with
// Task 15's `lib/api/outcomes.ts`. (Task 11/15 reuse)
import { useState } from "react";
import Link from "next/link";
import {
  useMutation,
  useQuery,
  useQueryClient,
} from "@tanstack/react-query";
import { z } from "zod";

import { ErrorBanner } from "@/components/ui/ErrorBanner";
import { apiSend } from "@/lib/api/client";
import {
  CanvasOutcome,
  listCanvasOutcomes,
} from "@/lib/api/canvasOutcomes";
import { toUserError, type UserError } from "@/lib/errors/userError";
import { useOnboardingStore } from "@/state/onboardingStore";

interface OutcomeListSectionProps {
  versionId: string;
  // Route base for the Edit Outcome link: /products/features/{type}/{slug}/{vnum}
  routeBase: string;
  // Outcome mutations are only allowed on DRAFT versions.
  editable: boolean;
}

const createOutcome = (vid: string, title: string) =>
  apiSend("POST", `/api/v1/versions/${vid}/outcomes`, CanvasOutcome, { title });
const cloneOutcome = (oid: string) =>
  apiSend("POST", `/api/v1/outcomes/${oid}/clone`, CanvasOutcome);
const deleteOutcome = (oid: string) =>
  apiSend("DELETE", `/api/v1/outcomes/${oid}`, z.void());

export function OutcomeListSection({
  versionId,
  routeBase,
  editable,
}: OutcomeListSectionProps) {
  const queryClient = useQueryClient();
  const completeOnboarding = useOnboardingStore((s) => s.complete);
  const queryKey = ["outcomes", versionId] as const;
  // Surface add/clone/remove failures (409/5xx/network) instead of failing
  // silently (mirrors ApplicabilityForm's toUserError + ErrorBanner pattern).
  const [mutationError, setMutationError] = useState<UserError | null>(null);

  const { data: outcomes = [], isLoading } = useQuery({
    queryKey,
    queryFn: () => listCanvasOutcomes(versionId),
  });

  const invalidate = () =>
    queryClient.invalidateQueries({ queryKey });

  const add = useMutation({
    mutationFn: () => createOutcome(versionId, "New Outcome"),
    onSuccess: () => {
      setMutationError(null);
      completeOnboarding("outcome");
      return invalidate();
    },
    onError: (err) =>
      setMutationError(toUserError(err, { surface: "create" })),
  });
  const clone = useMutation({
    mutationFn: (oid: string) => cloneOutcome(oid),
    onSuccess: () => {
      setMutationError(null);
      return invalidate();
    },
    onError: (err) =>
      setMutationError(toUserError(err, { surface: "create" })),
  });
  const remove = useMutation({
    mutationFn: (oid: string) => deleteOutcome(oid),
    onSuccess: () => {
      setMutationError(null);
      return invalidate();
    },
    onError: (err) =>
      setMutationError(toUserError(err, { surface: "delete" })),
  });

  return (
    <section className="space-y-3">
      <div className="flex items-center justify-between">
        <h2 className="text-lg font-semibold text-nav">Outcomes</h2>
        {editable && (
          <button
            type="button"
            onClick={() => add.mutate()}
            disabled={add.isPending}
            className="rounded-md bg-action-600 px-3 py-1.5 text-sm font-semibold text-white hover:bg-action-700 disabled:opacity-50"
          >
            + Add Outcome
          </button>
        )}
      </div>

      {mutationError && <ErrorBanner error={mutationError} />}

      {isLoading ? (
        <p className="text-sm text-status-prev">Loading outcomes…</p>
      ) : outcomes.length === 0 ? (
        <p className="text-sm text-status-prev">No outcomes yet.</p>
      ) : (
        <ul className="divide-y divide-status-prevBg rounded-lg border border-status-prevBg bg-bg-elevated">
          {outcomes.map((o) => (
            <li
              key={o.id}
              className="flex items-center justify-between px-4 py-3"
            >
              <div className="flex items-center gap-2">
                <span className="font-medium text-nav">{o.title}</span>
                {o.is_builtin && (
                  <span className="rounded bg-bg-overlay px-1.5 py-0.5 text-[10px] font-semibold uppercase text-status-prevFg">
                    Built-in
                  </span>
                )}
              </div>
              <div className="flex items-center gap-2 text-sm">
                <Link
                  href={`${routeBase}/transformation/${o.id}`}
                  className="font-medium text-action-600 hover:text-action-700"
                >
                  {editable ? "Edit" : "View"}
                </Link>
                {!o.is_builtin && editable && (
                  <>
                    <button
                      type="button"
                      onClick={() => clone.mutate(o.id)}
                      disabled={clone.isPending || remove.isPending}
                      className="text-status-prevFg hover:text-brand-600 disabled:cursor-not-allowed disabled:opacity-50"
                    >
                      Clone
                    </button>
                    <button
                      type="button"
                      onClick={() => remove.mutate(o.id)}
                      disabled={clone.isPending || remove.isPending}
                      className="text-danger hover:text-danger disabled:cursor-not-allowed disabled:opacity-50"
                    >
                      Delete
                    </button>
                  </>
                )}
              </div>
            </li>
          ))}
        </ul>
      )}
    </section>
  );
}
