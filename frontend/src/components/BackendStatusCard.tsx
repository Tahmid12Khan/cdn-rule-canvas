"use client";

// Client component: useQuery(['health']) is a browser-side data hook.
import { useQuery } from "@tanstack/react-query";
import clsx from "clsx";

import { fetchHealth } from "@/lib/api/health";

export function BackendStatusCard() {
  const { data, isPending, isError } = useQuery({
    queryKey: ["health"],
    queryFn: fetchHealth,
  });

  const connected = !isPending && !isError;

  return (
    <div className="flex items-center gap-3 rounded-lg border border-status-prevBg bg-bg-elevated px-4 py-3 shadow-sm">
      <span
        className={clsx(
          "inline-block h-2.5 w-2.5 rounded-full",
          isPending && "bg-status-prev",
          isError && "bg-danger",
          connected && "bg-status-live",
        )}
        aria-hidden
      />
      <div className="text-sm">
        <span className="font-medium text-nav">Backend Status</span>
        <span className="ml-2 text-status-prevFg">
          {isPending && "Checking…"}
          {isError && "Backend unreachable"}
          {connected &&
            `Connected${data?.version ? ` · v${data.version}` : ""}`}
        </span>
      </div>
    </div>
  );
}
