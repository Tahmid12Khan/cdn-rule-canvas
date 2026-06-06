"use client";

// Searchable single-select site picker for the `site_select` field control
// (sites-host-config-spec §6). Debounces the query (300ms), searches Sites by
// name case-insensitively (the backend `q` filter is ILIKE), and stores the
// selected site's SLUG. The current selection is shown as "name (slug)".
//
// Browser state (open/query) + a network query → client component.
import { useEffect, useId, useRef, useState } from "react";
import { useQuery } from "@tanstack/react-query";

import { getSite, searchSites, type SiteRead } from "@/lib/api/sites";
import { useDebouncedValue } from "@/lib/hooks/useDebouncedValue";

interface SiteSelectControlProps {
  id: string;
  value: string; // the stored slug ("" when unset)
  onChange: (slug: string) => void;
  disabled?: boolean;
  placeholder?: string;
}

export function SiteSelectControl({
  id,
  value,
  onChange,
  disabled = false,
  placeholder = "Search sites by name…",
}: SiteSelectControlProps) {
  const [open, setOpen] = useState(false);
  const [query, setQuery] = useState("");
  const debounced = useDebouncedValue(query, 300);
  const listboxId = useId();
  const containerRef = useRef<HTMLDivElement>(null);

  const { data, isFetching } = useQuery({
    queryKey: ["sites", "search", debounced],
    queryFn: () => searchSites(debounced),
    enabled: open,
  });

  const options: SiteRead[] = data?.items ?? [];

  // Resolve the persisted slug to its site so the current selection reads
  // "name (slug)" (spec §6) rather than a bare slug. Falls back to the slug
  // alone while loading or when the slug no longer resolves to a site.
  const { data: selectedSite } = useQuery({
    queryKey: ["sites", "get", value],
    queryFn: () => getSite(value),
    enabled: value !== "",
  });
  const selectedLabel = selectedSite
    ? `${selectedSite.name} (${selectedSite.slug})`
    : value;

  // Close on outside click so the combobox behaves like a native dropdown.
  useEffect(() => {
    if (!open) return;
    function onClickOutside(e: MouseEvent) {
      if (
        containerRef.current &&
        !containerRef.current.contains(e.target as Node)
      ) {
        setOpen(false);
      }
    }
    document.addEventListener("mousedown", onClickOutside);
    return () => document.removeEventListener("mousedown", onClickOutside);
  }, [open]);

  function select(slug: string) {
    onChange(slug);
    setOpen(false);
    setQuery("");
  }

  return (
    <div ref={containerRef} className="relative">
      {value && !open && (
        <div className="mb-1 flex items-center justify-between gap-2 rounded-md border border-status-prevBg px-3 py-2 text-sm">
          <span className="truncate text-nav">
            Selected:{" "}
            <code className="font-mono text-fg-muted">{selectedLabel}</code>
          </span>
          {!disabled && (
            <button
              type="button"
              onClick={() => onChange("")}
              className="shrink-0 text-xs font-medium text-danger hover:underline"
            >
              Clear
            </button>
          )}
        </div>
      )}

      <input
        id={id}
        type="text"
        role="combobox"
        aria-expanded={open}
        aria-controls={listboxId}
        autoComplete="off"
        value={query}
        onChange={(e) => {
          setQuery(e.target.value);
          if (!open) setOpen(true);
        }}
        onFocus={() => setOpen(true)}
        disabled={disabled}
        placeholder={placeholder}
        className="w-full rounded-md border border-status-prevBg px-3 py-2 text-sm focus:border-brand-500 disabled:cursor-not-allowed disabled:opacity-60"
      />

      {open && (
        <ul
          id={listboxId}
          role="listbox"
          className="absolute z-10 mt-1 max-h-56 w-full overflow-auto rounded-md border border-status-prevBg bg-bg-elevated py-1 shadow-lg"
        >
          {isFetching && options.length === 0 && (
            <li className="px-3 py-2 text-sm text-status-prevFg">Searching…</li>
          )}
          {!isFetching && options.length === 0 && (
            <li className="px-3 py-2 text-sm text-status-prevFg">
              No matching sites
            </li>
          )}
          {options.map((site) => (
            <li key={site.slug} role="option" aria-selected={site.slug === value}>
              <button
                type="button"
                onClick={() => select(site.slug)}
                className="flex w-full items-center justify-between gap-2 px-3 py-2 text-left text-sm text-nav hover:bg-status-prevBg/40"
              >
                <span className="truncate">{site.name}</span>
                <code className="shrink-0 font-mono text-xs text-fg-muted">
                  {site.slug}
                </code>
              </button>
            </li>
          ))}
        </ul>
      )}
    </div>
  );
}
