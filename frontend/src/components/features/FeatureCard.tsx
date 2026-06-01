import Link from "next/link";

import type { FeatureRead } from "@/lib/api/features";

// Feature card (FRONTEND CONTRACT §2.2, Task 06). Server-safe presentational
// component: a single feature in the grid, linking to its version list. No
// client interactivity needed — it is a plain navigational link.

const TYPE_BADGE: Record<FeatureRead["type"], string> = {
  html: "bg-brand-50 text-accent-onMuted",
  json: "bg-action-600/10 text-action-700",
};

export function FeatureCard({ feature }: { feature: FeatureRead }) {
  const href = `/products/features/${feature.type}/${feature.id}`;
  const hasLive = feature.live_version_id !== null;
  const hasStaging = feature.staging_version_id !== null;

  return (
    <Link
      href={href}
      className="group flex flex-col gap-3 rounded-lg border border-border bg-bg-elevated p-5 transition-colors hover:border-accent"
    >
      <div className="flex items-start justify-between gap-3">
        <h3 className="text-base font-semibold text-nav group-hover:text-brand-700">
          {feature.name}
        </h3>
        <span
          className={`shrink-0 rounded-full px-2 py-0.5 text-xs font-semibold uppercase ${TYPE_BADGE[feature.type]}`}
        >
          {feature.type}
        </span>
      </div>

      <code className="truncate font-mono text-xs text-fg-muted">
        {feature.id}
      </code>

      <div className="mt-1 flex items-center gap-2 text-xs">
        <span
          className={
            hasLive
              ? "inline-flex items-center gap-1 rounded-full bg-status-liveBg px-2 py-0.5 font-medium text-status-liveFg"
              : "inline-flex items-center gap-1 rounded-full border border-status-draftBorder px-2 py-0.5 font-medium text-status-draft"
          }
        >
          <span
            className={`inline-block h-1.5 w-1.5 rounded-full ${hasLive ? "bg-status-live" : "bg-status-draft"}`}
            aria-hidden
          />
          {hasLive ? "Live" : "No live version"}
        </span>
        {hasStaging && (
          <span className="inline-flex items-center gap-1 rounded-full bg-status-stagingBg px-2 py-0.5 font-medium text-status-stagingFg">
            <span
              className="inline-block h-1.5 w-1.5 rounded-full bg-status-staging"
              aria-hidden
            />
            Staging
          </span>
        )}
      </div>
    </Link>
  );
}
