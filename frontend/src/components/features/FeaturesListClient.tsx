"use client";

// Client component: owns the features list interactivity — TanStack Query for
// the list, the create modal, and pagination. FRONTEND CONTRACT §2.2, Task 06.
import { useState } from "react";
import { keepPreviousData, useQuery } from "@tanstack/react-query";

import { FeatureCard } from "@/components/features/FeatureCard";
import { FeatureCreateModal } from "@/components/features/FeatureCreateModal";
import { CardGridSkeleton } from "@/components/ui/CardGridSkeleton";
import { EmptyState } from "@/components/ui/EmptyState";
import { ErrorBanner } from "@/components/ui/ErrorBanner";
import { Pagination } from "@/components/ui/Pagination";
import { toUserError } from "@/lib/errors/userError";
import { listFeatures } from "@/lib/api/features";

const PAGE_SIZE = 20;

export function FeaturesListClient() {
  const [page, setPage] = useState(1);

  const { data, isPending, isError, error, refetch, isFetching } = useQuery({
    queryKey: ["features", { page, page_size: PAGE_SIZE }],
    queryFn: () => listFeatures({ page, page_size: PAGE_SIZE }),
    placeholderData: keepPreviousData,
  });

  return (
    <div className="flex flex-col gap-6">
      <div className="flex items-center justify-between gap-4">
        <div>
          <h1 className="text-2xl font-bold text-nav">Features</h1>
          <p className="mt-1 text-sm text-status-prevFg">
            Author, version, and deploy response transformation rules.
          </p>
        </div>
        <FeatureCreateModal />
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
          title="No features yet"
          description="Step 1 of 5: Add your first feature. A feature groups the rule versions for a single response transformation."
          action={<FeatureCreateModal />}
        />
      )}

      {!isPending && !isError && data.items.length > 0 && (
        <>
          <div
            className="grid grid-cols-1 gap-4 sm:grid-cols-2 lg:grid-cols-3"
            aria-busy={isFetching}
          >
            {data.items.map((feature) => (
              <FeatureCard key={feature.id} feature={feature} />
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
    </div>
  );
}
