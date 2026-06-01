// Metadata card: Last Updated By / On (Task 11). Presentational (server-safe).
interface LastUpdatedCardProps {
  by: string;
  at: string; // ISO8601
}

function formatTimestamp(iso: string): string {
  const d = new Date(iso);
  if (Number.isNaN(d.getTime())) return iso;
  return d.toLocaleString(undefined, {
    year: "numeric",
    month: "short",
    day: "numeric",
    hour: "2-digit",
    minute: "2-digit",
  });
}

export function LastUpdatedCard({ by, at }: LastUpdatedCardProps) {
  return (
    <dl className="inline-flex flex-col gap-1 rounded-lg border border-status-prevBg bg-bg-elevated px-4 py-3 text-sm shadow-sm">
      <div className="flex items-center gap-2">
        <dt className="font-medium text-status-prev">Last Updated By</dt>
        <dd className="font-semibold text-nav">{by}</dd>
      </div>
      <div className="flex items-center gap-2">
        <dt className="font-medium text-status-prev">Last Updated On</dt>
        {/* Locale/timezone formatting differs between the server (UTC) and the
            client, so suppress the unavoidable text hydration mismatch. */}
        <dd className="text-status-prevFg" suppressHydrationWarning>
          {formatTimestamp(at)}
        </dd>
      </div>
    </dl>
  );
}
