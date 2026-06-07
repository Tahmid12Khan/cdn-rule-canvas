"use client";

// Client component: owns the features list interactivity — TanStack Query for
// the list and the create modal. FRONTEND CONTRACT §2.2, Task 06.
//
// Spec item 8: the list is split into two labeled sections — "HTML Rules" and
// "JSON Rules" — each ordered by execution_order ASC, with the execution number
// shown per card (see FeatureCard). We fetch a large page so both sections show
// all features on one page (no pager).
import { useQuery } from "@tanstack/react-query";

import { FeatureCard } from "@/components/features/FeatureCard";
import { FeatureCreateModal } from "@/components/features/FeatureCreateModal";
import { CardGridSkeleton } from "@/components/ui/CardGridSkeleton";
import { EmptyState } from "@/components/ui/EmptyState";
import { ErrorBanner } from "@/components/ui/ErrorBanner";
import { toUserError } from "@/lib/errors/userError";
import type { FeatureType } from "@/lib/api/enums";
import { type FeatureRead, listFeatures } from "@/lib/api/features";

// High limit so both ordered sections show every feature on a single page.
const PAGE_SIZE = 100;

type FeaturesPage = Awaited<ReturnType<typeof listFeatures>>;

interface FeaturesListClientProps {
  // SSR-prefetched first page. Seeds initialData on the list key so the list
  // hydrates without a client-side fetch waterfall.
  initialFeatures?: FeaturesPage;
}

const SECTIONS: { type: FeatureType; label: string; emptyHint: string }[] = [
  {
    type: "html",
    label: "HTML Rules",
    emptyHint: "No HTML rules yet. Add one to transform HTML responses.",
  },
  {
    type: "json",
    label: "JSON Rules",
    emptyHint: "No JSON rules yet. Add one to transform JSON responses.",
  },
];

// Sort defensively by execution_order ASC (the API already orders, but the
// section view depends on it so we don't trust ordering implicitly).
function byExecutionOrder(a: FeatureRead, b: FeatureRead): number {
  return a.execution_order - b.execution_order;
}

export function FeaturesListClient({
  initialFeatures,
}: FeaturesListClientProps = {}) {
  const { data, isPending, isError, error, refetch, isFetching } = useQuery({
    queryKey: ["features", { page: 1, page_size: PAGE_SIZE }],
    queryFn: () => listFeatures({ page: 1, page_size: PAGE_SIZE }),
    initialData: initialFeatures,
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
        <div className="flex flex-col gap-8" aria-busy={isFetching}>
          {SECTIONS.map(({ type, label, emptyHint }) => {
            const items = data.items
              .filter((f) => f.type === type)
              .sort(byExecutionOrder);
            return (
              <section key={type} className="flex flex-col gap-4">
                <div className="flex items-center gap-3 border-b border-border pb-2">
                  <h2 className="text-lg font-semibold text-nav">{label}</h2>
                  <span className="rounded-full bg-brand-50 px-2 py-0.5 text-xs font-semibold text-accent-onMuted">
                    {items.length}
                  </span>
                </div>
                {items.length === 0 ? (
                  <p className="text-sm text-fg-muted">{emptyHint}</p>
                ) : (
                  <div className="grid grid-cols-1 gap-4 sm:grid-cols-2 lg:grid-cols-3">
                    {items.map((feature) => (
                      <FeatureCard key={feature.id} feature={feature} />
                    ))}
                  </div>
                )}
              </section>
            );
          })}
        </div>
      )}
    </div>
  );
}
