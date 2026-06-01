"use client";

// 3-dot user-type slider: Anonymous / Registered / Customer. Implemented as a
// role="tablist" with arrow-key navigation (FRONTEND CONTRACT §7). Switching
// preserves all three canvases (the store keeps them independent). (Task 11)
import { useRef } from "react";
import clsx from "clsx";

import type { CanvasKey } from "@/lib/canvas/types";

const TABS: { key: CanvasKey; label: string }[] = [
  { key: "anonymous", label: "Anonymous" },
  { key: "registered", label: "Registered" },
  { key: "customer", label: "Customer" },
];

interface CanvasSliderProps {
  selected: CanvasKey;
  onSelect: (key: CanvasKey) => void;
}

export function CanvasSlider({ selected, onSelect }: CanvasSliderProps) {
  const refs = useRef<(HTMLButtonElement | null)[]>([]);

  function onKeyDown(e: React.KeyboardEvent, idx: number) {
    if (e.key !== "ArrowRight" && e.key !== "ArrowLeft") return;
    e.preventDefault();
    const dir = e.key === "ArrowRight" ? 1 : -1;
    const next = (idx + dir + TABS.length) % TABS.length;
    onSelect(TABS[next].key);
    refs.current[next]?.focus();
  }

  return (
    <div
      role="tablist"
      aria-label="Canvas user type"
      className="inline-flex items-center gap-1 rounded-full border border-status-prevBg bg-bg-elevated p-1 shadow-sm"
    >
      {TABS.map((tab, idx) => {
        const active = tab.key === selected;
        return (
          <button
            key={tab.key}
            ref={(el) => {
              refs.current[idx] = el;
            }}
            role="tab"
            type="button"
            aria-selected={active}
            tabIndex={active ? 0 : -1}
            onClick={() => onSelect(tab.key)}
            onKeyDown={(e) => onKeyDown(e, idx)}
            className={clsx(
              "flex items-center gap-2 rounded-full px-3 py-1.5 text-sm font-medium transition-colors",
              active
                ? "bg-brand-500 text-white"
                : "text-status-prevFg hover:bg-brand-50",
            )}
          >
            <span
              aria-hidden
              className={clsx(
                "h-2 w-2 rounded-full",
                active ? "bg-bg-elevated" : "bg-status-prev",
              )}
            />
            {tab.label}
          </button>
        );
      })}
    </div>
  );
}
