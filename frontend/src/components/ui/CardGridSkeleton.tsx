// Server-safe component: pure presentational loading skeleton grid.
export function CardGridSkeleton({ count = 6 }: { count?: number }) {
  return (
    <div
      className="grid grid-cols-1 gap-4 sm:grid-cols-2 lg:grid-cols-3"
      aria-hidden
    >
      {Array.from({ length: count }).map((_, idx) => (
        <div
          key={idx}
          className="h-32 animate-pulse rounded-lg border border-status-prevBg bg-status-prevBg/40"
        />
      ))}
    </div>
  );
}
