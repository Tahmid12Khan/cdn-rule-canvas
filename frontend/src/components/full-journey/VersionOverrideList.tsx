"use client";

// Per-feature version-override list (spec item 5 form). Lists every feature
// grouped into HTML Rules / JSON Rules, ordered by execution_order, each with a
// version <select> defaulting to "Active (live)" plus its concrete version
// numbers. A non-default selection is surfaced to the parent as
// version_overrides[feature_id] = version_number; clearing it back to "Active"
// removes the entry.
//
// Client component: each feature row lazily fetches its own versions (useQuery)
// only when present on the page, and owns no override state itself — the map is
// lifted to FullJourneyClient so the POST body stays a single source of truth.
import { useMemo } from "react";
import { useQuery } from "@tanstack/react-query";

import { listFeatures, type FeatureRead } from "@/lib/api/features";
import { listVersions } from "@/lib/api/versions";

interface VersionOverrideListProps {
  // feature_id → concrete version_number to run instead of the active version.
  overrides: Record<string, number>;
  onChange: (next: Record<string, number>) => void;
}

const FEATURES_PAGE_SIZE = 100;
const VERSIONS_PAGE_SIZE = 100;

// "Active (live)" sentinel for the select's default option value.
const ACTIVE = "active";

function FeatureRow({
  feature,
  value,
  onSelect,
}: {
  feature: FeatureRead;
  value: number | undefined;
  onSelect: (version: number | undefined) => void;
}) {
  const { data, isPending } = useQuery({
    queryKey: ["fj-versions", feature.id],
    queryFn: () =>
      listVersions(feature.id, { page: 1, page_size: VERSIONS_PAGE_SIZE }),
  });

  const versions = data?.items ?? [];

  return (
    <div className="flex items-center justify-between gap-3 rounded-md border border-status-prevBg bg-bg px-3 py-2">
      <div className="min-w-0">
        <p className="truncate text-sm font-medium text-nav" title={feature.name}>
          {feature.name}
        </p>
        <p className="truncate font-mono text-[11px] text-status-prevFg">
          {feature.id} · order {feature.execution_order}
        </p>
      </div>
      <select
        aria-label={`Version for ${feature.name}`}
        value={value === undefined ? ACTIVE : String(value)}
        disabled={isPending}
        onChange={(e) => {
          const v = e.target.value;
          onSelect(v === ACTIVE ? undefined : Number(v));
        }}
        className="shrink-0 rounded-md border border-status-prevBg bg-bg-elevated px-2 py-1 text-sm text-nav focus:border-brand-500 focus:outline-none disabled:opacity-50"
      >
        <option value={ACTIVE}>Active (live)</option>
        {versions.map((v) => (
          <option key={v.id} value={String(v.version_number)}>
            v{v.version_number}
            {v.status === "live" ? " (live)" : ""}
          </option>
        ))}
      </select>
    </div>
  );
}

export function VersionOverrideList({
  overrides,
  onChange,
}: VersionOverrideListProps) {
  const { data, isPending, isError } = useQuery({
    queryKey: ["fj-features"],
    queryFn: () => listFeatures({ page: 1, page_size: FEATURES_PAGE_SIZE }),
  });

  const { htmlFeatures, jsonFeatures } = useMemo(() => {
    const items = data?.items ?? [];
    const byOrder = (a: FeatureRead, b: FeatureRead) =>
      a.execution_order - b.execution_order;
    return {
      htmlFeatures: items.filter((f) => f.type === "html").sort(byOrder),
      jsonFeatures: items.filter((f) => f.type === "json").sort(byOrder),
    };
  }, [data]);

  function setOverride(featureId: string, version: number | undefined) {
    const next = { ...overrides };
    if (version === undefined) delete next[featureId];
    else next[featureId] = version;
    onChange(next);
  }

  if (isPending) {
    return (
      <p className="text-xs text-status-prevFg">Loading features…</p>
    );
  }

  if (isError) {
    return (
      <p className="text-xs font-medium text-danger">
        Couldn&apos;t load features — version overrides are unavailable. The
        journey still runs each feature&apos;s active version.
      </p>
    );
  }

  function renderGroup(label: string, features: FeatureRead[]) {
    if (features.length === 0) return null;
    return (
      <div className="flex flex-col gap-2">
        <p className="text-xs font-semibold uppercase tracking-wide text-status-prevFg">
          {label}
        </p>
        {features.map((feature) => (
          <FeatureRow
            key={feature.id}
            feature={feature}
            value={overrides[feature.id]}
            onSelect={(version) => setOverride(feature.id, version)}
          />
        ))}
      </div>
    );
  }

  if (htmlFeatures.length === 0 && jsonFeatures.length === 0) {
    return (
      <p className="text-xs text-status-prevFg">
        No features configured yet.
      </p>
    );
  }

  return (
    <div className="flex flex-col gap-4">
      {renderGroup("HTML Rules", htmlFeatures)}
      {renderGroup("JSON Rules", jsonFeatures)}
    </div>
  );
}
