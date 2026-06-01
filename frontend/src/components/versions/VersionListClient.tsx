"use client";

// Client component: orchestrates the version list page (FRONTEND CONTRACT
// §2.3). Owns search term (debounced inside SearchInput), pagination, and the
// two TanStack queries (feature + versions). Mutations live in the leaf dialogs
// / RowActionsMenu and invalidate ['versions', fid] / ['feature', fid].
import { useCallback, useMemo, useState } from "react";
import * as Tooltip from "@radix-ui/react-tooltip";
import { useQuery } from "@tanstack/react-query";

import { CardGridSkeleton } from "@/components/ui/CardGridSkeleton";
import { ErrorBanner } from "@/components/ui/ErrorBanner";
import { Pagination } from "@/components/ui/Pagination";
import { AddVersionDialog } from "@/components/versions/AddVersionDialog";
import { DeploymentStatusRow } from "@/components/versions/DeploymentStatusRow";
import { SearchInput } from "@/components/versions/SearchInput";
import { VersionsTable } from "@/components/versions/VersionsTable";
import { getFeature } from "@/lib/api/features";
import { listVersions } from "@/lib/api/versions";
import { toUserError } from "@/lib/errors/userError";

const PAGE_SIZE = 20;

interface VersionListClientProps {
  featureId: string;
  featureType: string;
}

export function VersionListClient({
  featureId,
  featureType,
}: VersionListClientProps) {
  const [search, setSearch] = useState("");
  const [page, setPage] = useState(1);

  // Reset to page 1 whenever the search term changes.
  const handleSearch = useCallback((term: string) => {
    setSearch(term);
    setPage(1);
  }, []);

  const featureQuery = useQuery({
    queryKey: ["feature", featureId],
    queryFn: () => getFeature(featureId),
  });

  const versionsQuery = useQuery({
    queryKey: ["versions", featureId, { search, page, page_size: PAGE_SIZE }],
    queryFn: () =>
      listVersions(featureId, { search, page, page_size: PAGE_SIZE }),
  });

  // Resolve a deployment version UUID -> its version_number using the loaded
  // version summaries (best-effort; "—" if not on the current page).
  const versionNumberById = useMemo(() => {
    const map = new Map<string, number>();
    for (const v of versionsQuery.data?.items ?? []) {
      map.set(v.id, v.version_number);
    }
    return (id: string) => map.get(id);
  }, [versionsQuery.data]);

  const feature = featureQuery.data;
  const versionsPage = versionsQuery.data;

  return (
    <Tooltip.Provider delayDuration={200}>
      <div>
        {featureQuery.isError ? (
          <ErrorBanner
            error={toUserError(featureQuery.error, { surface: "load" })}
            onRetry={() => void featureQuery.refetch()}
          />
        ) : (
          <header className="space-y-4">
            <div className="flex flex-wrap items-center gap-3">
              <h1 className="text-2xl font-bold text-nav">
                {feature?.name ?? featureId}
              </h1>
            </div>
            {feature && (
              <DeploymentStatusRow
                stagingVersionId={feature.staging_version_id}
                liveVersionId={feature.live_version_id}
                versionNumberById={versionNumberById}
              />
            )}
          </header>
        )}

        <section className="mt-8">
          <div className="flex flex-wrap items-center justify-between gap-3">
            <h2 className="text-lg font-semibold text-nav">Versions</h2>
            <div className="flex flex-wrap items-center gap-2">
              <Tooltip.Root>
                <Tooltip.Trigger asChild>
                  <button
                    type="button"
                    aria-disabled="true"
                    onClick={(e) => e.preventDefault()}
                    className="cursor-not-allowed rounded-lg border border-status-prevBg px-3.5 py-2 text-sm font-medium text-status-prev"
                  >
                    Access Permissions
                  </button>
                </Tooltip.Trigger>
                <Tooltip.Portal>
                  <Tooltip.Content
                    sideOffset={4}
                    className="rounded-md bg-nav px-2.5 py-1.5 text-xs text-nav-fg shadow-md"
                  >
                    Coming soon
                    <Tooltip.Arrow className="fill-nav" />
                  </Tooltip.Content>
                </Tooltip.Portal>
              </Tooltip.Root>
              <AddVersionDialog featureId={featureId} />
            </div>
          </div>

          <div className="mt-4 flex justify-end">
            <SearchInput onSearch={handleSearch} />
          </div>

          <div className="mt-4">
            {versionsQuery.isLoading ? (
              <CardGridSkeleton count={4} />
            ) : versionsQuery.isError ? (
              <ErrorBanner
                error={toUserError(versionsQuery.error, { surface: "load" })}
                onRetry={() => void versionsQuery.refetch()}
              />
            ) : (
              <>
                {versionsPage &&
                  versionsPage.total === 0 &&
                  search.length === 0 && (
                    <p className="mb-3 rounded-lg border border-dashed border-status-prevBg bg-status-prevBg/30 px-4 py-3 text-sm text-status-prevFg">
                      Step 2 of 5: Add a draft version to start building rules. A
                      new draft is seeded from the current live rules.
                    </p>
                  )}
                <VersionsTable
                  featureId={featureId}
                  featureType={featureType}
                  versions={versionsPage?.items ?? []}
                />
                {versionsPage && versionsPage.total > 0 && (
                  <Pagination
                    page={versionsPage.page}
                    pageSize={versionsPage.page_size}
                    total={versionsPage.total}
                    onPageChange={setPage}
                  />
                )}
              </>
            )}
          </div>
        </section>
      </div>
    </Tooltip.Provider>
  );
}
