import clsx from "clsx";

import type { VersionStatus } from "@/lib/api/enums";

// Status pill. Color is never the sole signal — the UPPERCASE label is always
// present (FRONTEND CONTRACT §7 accessibility).
const VARIANTS: Record<VersionStatus, { label: string; className: string }> = {
  live: {
    label: "LIVE",
    className: "bg-status-liveBg text-status-liveFg",
  },
  staging: {
    label: "STAGING",
    className: "bg-status-stagingBg text-status-stagingFg",
  },
  prev: {
    label: "PREV",
    className: "bg-status-prevBg text-status-prevFg",
  },
  draft: {
    label: "DRAFT",
    className:
      "border border-status-draftBorder bg-transparent text-status-draft",
  },
};

export function StatusPill({ status }: { status: VersionStatus }) {
  const variant = VARIANTS[status];
  return (
    <span
      className={clsx(
        "inline-flex items-center rounded-full px-2.5 py-0.5 text-xs font-semibold",
        variant.className,
      )}
    >
      {variant.label}
    </span>
  );
}
