"use client";

// Client component: owns the test-presets list interactivity — TanStack Query
// for the list, the create/edit modal, delete confirmation, and pagination.
// Mirrors SitesListClient.
import { useState } from "react";
import { keepPreviousData, useQuery } from "@tanstack/react-query";

import { TestPresetCard } from "@/components/test-presets/TestPresetCard";
import { TestPresetDeleteDialog } from "@/components/test-presets/TestPresetDeleteDialog";
import {
  TestPresetExamples,
  type TestPresetExample,
} from "@/components/test-presets/TestPresetExamples";
import { TestPresetFormModal } from "@/components/test-presets/TestPresetFormModal";
import { CardGridSkeleton } from "@/components/ui/CardGridSkeleton";
import { EmptyState } from "@/components/ui/EmptyState";
import { ErrorBanner } from "@/components/ui/ErrorBanner";
import { Pagination } from "@/components/ui/Pagination";
import { listTestPresets, type TestPresetRead } from "@/lib/api/test-presets";
import { toUserError } from "@/lib/errors/userError";

const PAGE_SIZE = 20;

type PresetsPage = Awaited<ReturnType<typeof listTestPresets>>;

interface TestPresetsListClientProps {
  // SSR-prefetched first page. Seeds initialData on the page-1 key.
  initialPresets?: PresetsPage;
}

const addButton = (
  <button
    type="button"
    className="inline-flex items-center rounded-md bg-action-600 px-4 py-2 text-sm font-medium text-white shadow-sm transition hover:bg-action-700"
  >
    + Add A Test Preset
  </button>
);

export function TestPresetsListClient({
  initialPresets,
}: TestPresetsListClientProps = {}) {
  const [page, setPage] = useState(1);
  const [editing, setEditing] = useState<TestPresetRead | null>(null);
  const [deleting, setDeleting] = useState<TestPresetRead | null>(null);
  // The starter example a user chose via "Use this template" — opens the full
  // create modal pre-filled from it.
  const [usingExample, setUsingExample] = useState<TestPresetExample | null>(
    null,
  );

  const { data, isPending, isError, error, refetch, isFetching } = useQuery({
    queryKey: ["test-presets", { page, page_size: PAGE_SIZE }],
    queryFn: () => listTestPresets({ page, page_size: PAGE_SIZE }),
    placeholderData: keepPreviousData,
    initialData: page === 1 ? initialPresets : undefined,
  });

  return (
    <div className="flex flex-col gap-6">
      <div className="flex items-center justify-between gap-4">
        <div>
          <h1 className="text-2xl font-bold text-nav">Test Presets</h1>
          <p className="mt-1 text-sm text-status-prevFg">
            Reusable inputs for the rule-builder Test panels. Save a synthetic
            rule context or a live-URL test once, then load it from any version.
          </p>
        </div>
        <TestPresetFormModal trigger={addButton} />
      </div>

      {/* Onboarding helper: one-click starter templates. Shown for both new and
          existing users so the "how do I make one?" path is always visible. */}
      {!isError && (
        <TestPresetExamples onUse={(example) => setUsingExample(example)} />
      )}

      {isError && (
        <ErrorBanner
          error={toUserError(error, { surface: "load" })}
          onRetry={() => void refetch()}
        />
      )}

      {isPending && !isError && <CardGridSkeleton />}

      {!isPending && !isError && data.items.length === 0 && (
        <EmptyState
          title="No test presets yet"
          description="Save a test from the rule builder, pick a starter template above, or add one here to reuse across versions."
          action={<TestPresetFormModal trigger={addButton} />}
        />
      )}

      {!isPending && !isError && data.items.length > 0 && (
        <>
          <div
            className="grid grid-cols-1 gap-4 sm:grid-cols-2 lg:grid-cols-3"
            aria-busy={isFetching}
          >
            {data.items.map((preset) => (
              <TestPresetCard
                key={preset.slug}
                preset={preset}
                onEdit={setEditing}
                onDelete={setDeleting}
              />
            ))}
          </div>
          <Pagination
            page={data.page}
            pageSize={data.page_size}
            total={data.total}
            onPageChange={setPage}
          />
        </>
      )}

      {editing && (
        <TestPresetFormModal
          key={editing.slug}
          preset={editing}
          open
          onOpenChange={(open) => {
            if (!open) setEditing(null);
          }}
        />
      )}

      {usingExample && (
        <TestPresetFormModal
          key={usingExample.slug}
          initialExample={usingExample}
          open
          onOpenChange={(open) => {
            if (!open) setUsingExample(null);
          }}
        />
      )}

      <TestPresetDeleteDialog
        preset={deleting}
        open={deleting !== null}
        onOpenChange={(open) => {
          if (!open) setDeleting(null);
        }}
      />
    </div>
  );
}
