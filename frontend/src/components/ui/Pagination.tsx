"use client";

// Client component: emits page-change events via onPageChange callback.
import clsx from "clsx";

interface PaginationProps {
  page: number; // 1-based
  pageSize: number;
  total: number;
  onPageChange: (page: number) => void;
}

export function Pagination({
  page,
  pageSize,
  total,
  onPageChange,
}: PaginationProps) {
  const from = total === 0 ? 0 : (page - 1) * pageSize + 1;
  const to = Math.min(page * pageSize, total);
  const lastPage = Math.max(1, Math.ceil(total / pageSize));

  return (
    <div className="flex items-center justify-between py-3 text-sm text-status-prevFg">
      <span>
        Results {from}–{to} of {total}
      </span>
      <div className="flex items-center gap-2">
        <button
          type="button"
          className={clsx(
            "rounded-md border border-status-prevBg px-3 py-1",
            page <= 1
              ? "cursor-not-allowed text-status-prev"
              : "hover:bg-status-prevBg",
          )}
          disabled={page <= 1}
          onClick={() => onPageChange(page - 1)}
        >
          Previous
        </button>
        <button
          type="button"
          className={clsx(
            "rounded-md border border-status-prevBg px-3 py-1",
            page >= lastPage
              ? "cursor-not-allowed text-status-prev"
              : "hover:bg-status-prevBg",
          )}
          disabled={page >= lastPage}
          onClick={() => onPageChange(page + 1)}
        >
          Next
        </button>
      </div>
    </div>
  );
}
