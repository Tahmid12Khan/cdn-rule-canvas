"use client";

// Client component: owns the sites list interactivity — TanStack Query for the
// list, the create/edit modal, delete confirmation, and pagination (spec §6).
// Mirrors FeaturesListClient.
import { useState } from "react";
import { keepPreviousData, useQuery } from "@tanstack/react-query";

import { SiteCard } from "@/components/sites/SiteCard";
import { SiteDeleteDialog } from "@/components/sites/SiteDeleteDialog";
import { SiteFormModal } from "@/components/sites/SiteFormModal";
import { CardGridSkeleton } from "@/components/ui/CardGridSkeleton";
import { EmptyState } from "@/components/ui/EmptyState";
import { ErrorBanner } from "@/components/ui/ErrorBanner";
import { Pagination } from "@/components/ui/Pagination";
import { listSites, type SiteRead } from "@/lib/api/sites";
import { toUserError } from "@/lib/errors/userError";

const PAGE_SIZE = 20;

type SitesPage = Awaited<ReturnType<typeof listSites>>;

interface SitesListClientProps {
  // SSR-prefetched first page. Seeds initialData on the page-1 key so the list
  // hydrates without a client-side fetch waterfall.
  initialSites?: SitesPage;
}

const addButton = (
  <button
    type="button"
    className="inline-flex items-center rounded-md bg-action-600 px-4 py-2 text-sm font-medium text-white shadow-sm transition hover:bg-action-700"
  >
    + Add A New Site
  </button>
);

export function SitesListClient({ initialSites }: SitesListClientProps = {}) {
  const [page, setPage] = useState(1);
  const [editing, setEditing] = useState<SiteRead | null>(null);
  const [deleting, setDeleting] = useState<SiteRead | null>(null);

  const { data, isPending, isError, error, refetch, isFetching } = useQuery({
    queryKey: ["sites", { page, page_size: PAGE_SIZE }],
    queryFn: () => listSites({ page, page_size: PAGE_SIZE }),
    placeholderData: keepPreviousData,
    initialData: page === 1 ? initialSites : undefined,
  });

  return (
    <div className="flex flex-col gap-6">
      <div className="flex items-center justify-between gap-4">
        <div>
          <h1 className="text-2xl font-bold text-nav">Sites</h1>
          <p className="mt-1 text-sm text-status-prevFg">
            Route an incoming source host to a destination upstream. Rules apply
            to every request; scope a rule to a site with a Site Match node.
          </p>
        </div>
        <SiteFormModal trigger={addButton} />
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
          title="No sites yet"
          description="Add your first site to route an incoming host to a destination upstream."
          action={<SiteFormModal trigger={addButton} />}
        />
      )}

      {!isPending && !isError && data.items.length > 0 && (
        <>
          <div
            className="grid grid-cols-1 gap-4 sm:grid-cols-2 lg:grid-cols-3"
            aria-busy={isFetching}
          >
            {data.items.map((site) => (
              <SiteCard
                key={site.slug}
                site={site}
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

      {/* Edit modal — keyed by slug so opening a different card re-seeds it.
          Controlled by the list's `editing` state (no trigger). */}
      {editing && (
        <SiteFormModal
          key={editing.slug}
          site={editing}
          open
          onOpenChange={(open) => {
            if (!open) setEditing(null);
          }}
        />
      )}

      <SiteDeleteDialog
        site={deleting}
        open={deleting !== null}
        onOpenChange={(open) => {
          if (!open) setDeleting(null);
        }}
      />
    </div>
  );
}
