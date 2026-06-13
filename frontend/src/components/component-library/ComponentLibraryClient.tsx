"use client";

// Component library list (design §5.1). Owns the list interactivity — TanStack
// Query for the page, the create dialog, delete confirmation, and pagination.
// Mirrors SitesListClient / TestPresetsListClient.
import { useState } from "react";
import { keepPreviousData, useQuery } from "@tanstack/react-query";

import { ComponentCard } from "@/components/component-library/ComponentCard";
import { ComponentCreateModal } from "@/components/component-library/ComponentCreateModal";
import { ComponentDeleteDialog } from "@/components/component-library/ComponentDeleteDialog";
import { CardGridSkeleton } from "@/components/ui/CardGridSkeleton";
import { EmptyState } from "@/components/ui/EmptyState";
import { ErrorBanner } from "@/components/ui/ErrorBanner";
import { Pagination } from "@/components/ui/Pagination";
import {
  componentKeys,
  listComponentTemplates,
} from "@/lib/api/componentTemplates";
import type { ComponentTemplateSummary } from "@/lib/schemas/componentTemplates";
import { toUserError } from "@/lib/errors/userError";

const PAGE_SIZE = 20;

type ComponentsPage = Awaited<ReturnType<typeof listComponentTemplates>>;

interface ComponentLibraryClientProps {
  initialComponents?: ComponentsPage;
}

const addButton = (
  <button
    type="button"
    className="inline-flex items-center rounded-md bg-accent px-4 py-2 text-sm font-medium text-accent-fg shadow-sm transition hover:opacity-90"
  >
    + Create a Component
  </button>
);

export function ComponentLibraryClient({
  initialComponents,
}: ComponentLibraryClientProps = {}) {
  const [page, setPage] = useState(1);
  const [deleting, setDeleting] = useState<ComponentTemplateSummary | null>(
    null,
  );

  const { data, isPending, isError, error, refetch, isFetching } = useQuery({
    queryKey: componentKeys.list({ page, page_size: PAGE_SIZE }),
    queryFn: () => listComponentTemplates({ page, page_size: PAGE_SIZE }),
    placeholderData: keepPreviousData,
    initialData: page === 1 ? initialComponents : undefined,
  });

  return (
    <div className="flex flex-col gap-6">
      <div className="flex items-center justify-between gap-4">
        <div>
          <h1 className="text-2xl font-bold text-fg">Components</h1>
          <p className="mt-1 text-sm text-fg-muted">
            A global library of reusable, independently-versioned HTML templates.
            Rules pick a component, a version, and fill in its variables.
          </p>
        </div>
        <ComponentCreateModal trigger={addButton} />
      </div>

      {isError && (
        <ErrorBanner
          error={toUserError(error, { surface: "load" })}
          onRetry={() => void refetch()}
        />
      )}

      {isPending && !isError && <CardGridSkeleton />}

      {!isPending && !isError && data.items.length === 0 && (
        <EmptyState
          title="No components yet"
          description="Create a reusable HTML template to inject from your rules."
          action={<ComponentCreateModal trigger={addButton} />}
        />
      )}

      {!isPending && !isError && data.items.length > 0 && (
        <>
          <div
            className="grid grid-cols-1 gap-4 sm:grid-cols-2 lg:grid-cols-3"
            aria-busy={isFetching}
          >
            {data.items.map((component) => (
              <ComponentCard
                key={component.id}
                component={component}
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

      <ComponentDeleteDialog
        component={deleting}
        open={deleting !== null}
        onOpenChange={(open) => {
          if (!open) setDeleting(null);
        }}
      />
    </div>
  );
}
