"use client";

// Searchable single-select saved-outcome picker for the `saved_outcome_select`
// field control (used by apply_saved_outcome / apply_saved_outcome_json).
// Debounces the query (300ms), searches saved outcomes by name
// case-insensitively (the backend `q` filter is ILIKE), and stores the selected
// outcome's ID (a UUID — not a slug, so the current selection is shown as
// "name (component_name)" for context rather than an "(id)" suffix).
//
// Browser state (open/query) + a network query → client component.
import { useEffect, useId, useRef, useState } from "react";
import { useQuery } from "@tanstack/react-query";

import {
  getSavedOutcome,
  searchSavedOutcomes,
  type SavedOutcomeRead,
} from "@/lib/api/savedOutcomes";
import { useDebouncedValue } from "@/lib/hooks/useDebouncedValue";

interface SavedOutcomeSelectControlProps {
  id: string;
  value: string; // the stored saved outcome id ("" when unset)
  onChange: (id: string) => void;
  disabled?: boolean;
  placeholder?: string;
}

export function SavedOutcomeSelectControl({
  id,
  value,
  onChange,
  disabled = false,
  placeholder = "Search saved outcomes by name…",
}: SavedOutcomeSelectControlProps) {
  const [open, setOpen] = useState(false);
  const [query, setQuery] = useState("");
  const debounced = useDebouncedValue(query, 300);
  const listboxId = useId();
  const containerRef = useRef<HTMLDivElement>(null);

  const { data, isFetching } = useQuery({
    queryKey: ["saved-outcomes", "search", debounced],
    queryFn: () => searchSavedOutcomes(debounced),
    enabled: open,
  });

  const options: SavedOutcomeRead[] = data?.items ?? [];

  // Resolve the persisted id to its outcome so the current selection reads
  // "name (component_name)" rather than a bare id. Falls back to the id alone
  // while loading or when the id no longer resolves to a saved outcome.
  const { data: selectedOutcome } = useQuery({
    queryKey: ["saved-outcomes", "get", value],
    queryFn: () => getSavedOutcome(value),
    enabled: value !== "",
  });
  const selectedLabel = selectedOutcome
    ? `${selectedOutcome.name} (${selectedOutcome.component_name})`
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

  function select(outcomeId: string) {
    onChange(outcomeId);
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
              No matching saved outcomes
            </li>
          )}
          {options.map((outcome) => (
            <li
              key={outcome.id}
              role="option"
              aria-selected={outcome.id === value}
            >
              <button
                type="button"
                onClick={() => select(outcome.id)}
                className="flex w-full items-center justify-between gap-2 px-3 py-2 text-left text-sm text-nav hover:bg-status-prevBg/40"
              >
                <span className="truncate">{outcome.name}</span>
                <code className="shrink-0 font-mono text-xs text-fg-muted">
                  {outcome.component_name}
                </code>
              </button>
            </li>
          ))}
        </ul>
      )}
    </div>
  );
}
