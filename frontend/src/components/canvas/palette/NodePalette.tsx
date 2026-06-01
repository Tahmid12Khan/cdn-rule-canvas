"use client";

// Horizontal scrollable category palette (Task 12 §4.5). A 🔍 search tab leads;
// selecting a category shows its chips. The search box filters chips across ALL
// categories by name (debounced). Only Content → Meta Tags, Session → Device
// Type, and one chip per Outcome are draggable; the rest are disabled.
import { useEffect, useMemo, useState } from "react";
import clsx from "clsx";

import { useNodeTypes } from "@/hooks/useNodeTypes";
import {
  buildPalette,
  type FeatureType,
  type NodeChipDef,
  type OutcomeOption,
  type PaletteCategory,
} from "@/lib/canvas/nodeTemplates";
import { NodeChip } from "@/components/canvas/palette/NodeChip";

interface NodePaletteProps {
  outcomes: OutcomeOption[];
  // Drag is only allowed in edit mode.
  draggable: boolean;
  // Feature content kind — filters chips by each spec's `applies_to` (req 7).
  featureType?: FeatureType;
}

const SEARCH_TAB = "__search";

export function NodePalette({
  outcomes,
  draggable,
  featureType = "html",
}: NodePaletteProps) {
  const { manifest } = useNodeTypes();
  const categories = useMemo(
    () => buildPalette(manifest, outcomes, featureType),
    [manifest, outcomes, featureType],
  );
  const [activeTab, setActiveTab] = useState<string>(
    categories[0]?.id ?? SEARCH_TAB,
  );
  const [searchInput, setSearchInput] = useState("");
  const [search, setSearch] = useState("");

  // Debounce the search term (300ms) without depending on the Task 08 hook.
  useEffect(() => {
    const t = setTimeout(() => setSearch(searchInput.trim().toLowerCase()), 300);
    return () => clearTimeout(t);
  }, [searchInput]);

  const isSearching = activeTab === SEARCH_TAB && search.length > 0;

  const filtered: NodeChipDef[] = useMemo(() => {
    if (!isSearching) return [];
    const all = categories.flatMap((c) => c.chips);
    return all.filter((chip) => chip.label.toLowerCase().includes(search));
  }, [categories, isSearching, search]);

  const activeCategory: PaletteCategory | undefined = categories.find(
    (c) => c.id === activeTab,
  );

  return (
    <div className="rounded-lg border border-status-prevBg bg-bg-elevated shadow-sm">
      {/* category tab strip (horizontally scrollable) */}
      <div
        role="tablist"
        aria-label="Node categories"
        className="flex items-center gap-1 overflow-x-auto border-b border-status-prevBg px-2 py-2"
      >
        <button
          role="tab"
          type="button"
          aria-selected={activeTab === SEARCH_TAB}
          aria-label="Search nodes"
          onClick={() => setActiveTab(SEARCH_TAB)}
          className={clsx(
            "flex h-8 w-8 shrink-0 items-center justify-center rounded-md text-base",
            activeTab === SEARCH_TAB
              ? "bg-brand-500 text-white"
              : "text-status-prevFg hover:bg-brand-50",
          )}
        >
          <span aria-hidden>🔍</span>
        </button>
        {categories.map((cat) => (
          <button
            key={cat.id}
            role="tab"
            type="button"
            aria-selected={activeTab === cat.id}
            onClick={() => setActiveTab(cat.id)}
            className={clsx(
              "shrink-0 whitespace-nowrap rounded-md px-3 py-1.5 text-sm font-medium transition-colors",
              activeTab === cat.id
                ? "bg-brand-500 text-white"
                : "text-status-prevFg hover:bg-brand-50",
            )}
          >
            {cat.label}
          </button>
        ))}
      </div>

      {/* chip row */}
      <div className="px-3 py-3">
        {activeTab === SEARCH_TAB ? (
          <div className="space-y-3">
            <input
              type="search"
              value={searchInput}
              onChange={(e) => setSearchInput(e.target.value)}
              placeholder="Search nodes…"
              aria-label="Search nodes"
              className="w-full max-w-xs rounded-md border border-status-prevBg px-3 py-1.5 text-sm focus:border-brand-500"
            />
            <div className="flex flex-wrap gap-2">
              {search.length === 0 ? (
                <p className="text-sm text-status-prev">
                  Type to search across all categories.
                </p>
              ) : filtered.length === 0 ? (
                <p className="text-sm text-status-prev">No matching nodes.</p>
              ) : (
                filtered.map((chip) => (
                  <NodeChip key={chip.id} chip={chip} draggable={draggable} />
                ))
              )}
            </div>
          </div>
        ) : (
          <div className="flex flex-wrap gap-2">
            {activeCategory?.chips.map((chip) => (
              <NodeChip key={chip.id} chip={chip} draggable={draggable} />
            ))}
          </div>
        )}
      </div>
    </div>
  );
}
