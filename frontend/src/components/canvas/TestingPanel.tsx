"use client";

// TestingPanel. A collapsible "Testing" section (collapsed by default) that
// hosts the two test panels as side-by-side tabs: "Test with live URL"
// (default) and "Test a rule". Only the active tab's body renders below. Tabs
// are a custom role=tablist with arrow-key nav + aria-selected, mirroring
// CanvasSlider (there is no Radix tabs dependency).
import { useRef, useState } from "react";
import clsx from "clsx";

import { TestPanel } from "@/components/canvas/TestPanel";
import { UrlTestPanel } from "@/components/canvas/UrlTestPanel";

interface TestingPanelProps {
  // Title resolver so a matched-outcome banner can show a friendly name.
  outcomeTitleById: (id: string) => string;
  // Feature content kind — passed to both panels.
  featureType: "html" | "json";
}

type TabKey = "url" | "rule";

const TABS: { key: TabKey; label: string }[] = [
  { key: "url", label: "Test with live URL" },
  { key: "rule", label: "Test a rule" },
];

export function TestingPanel({
  outcomeTitleById,
  featureType,
}: TestingPanelProps) {
  const [open, setOpen] = useState(false);
  const [activeTab, setActiveTab] = useState<TabKey>("url");
  const refs = useRef<(HTMLButtonElement | null)[]>([]);

  function onKeyDown(e: React.KeyboardEvent, idx: number) {
    if (e.key !== "ArrowRight" && e.key !== "ArrowLeft") return;
    e.preventDefault();
    const dir = e.key === "ArrowRight" ? 1 : -1;
    const next = (idx + dir + TABS.length) % TABS.length;
    setActiveTab(TABS[next].key);
    refs.current[next]?.focus();
  }

  return (
    <section className="rounded-lg border border-status-prevBg bg-bg-elevated shadow-sm">
      <button
        type="button"
        onClick={() => setOpen((o) => !o)}
        aria-expanded={open}
        className="flex w-full items-center justify-between px-4 py-3 text-sm font-semibold text-nav"
      >
        Testing
        <span aria-hidden className="text-status-prevFg">
          {open ? "▲" : "▼"}
        </span>
      </button>

      {open && (
        <div className="border-t border-status-prevBg p-4">
          <div
            role="tablist"
            aria-label="Testing mode"
            className="inline-flex items-center gap-1 rounded-full border border-status-prevBg bg-bg-elevated p-1 shadow-sm"
          >
            {TABS.map((tab, idx) => {
              const active = tab.key === activeTab;
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
                  onClick={() => setActiveTab(tab.key)}
                  onKeyDown={(e) => onKeyDown(e, idx)}
                  className={clsx(
                    "rounded-full px-3 py-1.5 text-sm font-medium transition-colors",
                    active
                      ? "bg-brand-500 text-white"
                      : "text-status-prevFg hover:bg-brand-50",
                  )}
                >
                  {tab.label}
                </button>
              );
            })}
          </div>

          <div className="mt-4">
            {activeTab === "url" ? (
              <UrlTestPanel
                outcomeTitleById={outcomeTitleById}
                featureType={featureType}
              />
            ) : (
              <TestPanel
                outcomeTitleById={outcomeTitleById}
                featureType={featureType}
              />
            )}
          </div>
        </div>
      )}
    </section>
  );
}
