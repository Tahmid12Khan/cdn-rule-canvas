"use client";

// TestingPanel. A right-docked, collapsible testing sidebar (collapsed by
// default). Collapsed, it renders as a thin vertical rail with a toggle button
// so the rule canvas takes the full width; expanded, it becomes a clamped-width
// (~360–420px), independently scrollable, md:sticky aside hosting the two test
// panels as side-by-side tabs: "Test with live URL" (default) and "Test a
// rule". Only the active tab's body renders. Tabs are a custom role=tablist
// with arrow-key nav + aria-selected, mirroring CanvasSlider (there is no Radix
// tabs dependency).
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

  // Collapsed: a thin vertical rail with a vertical "Testing" label that, when
  // clicked, expands the sidebar. Width shrinks to the rail so the canvas to
  // the left can reclaim the freed horizontal space.
  if (!open) {
    return (
      <aside className="w-full md:w-12 md:shrink-0">
        <button
          type="button"
          onClick={() => setOpen(true)}
          aria-expanded={false}
          className={clsx(
            "flex items-center justify-center gap-2 rounded-lg border border-status-prevBg bg-bg-elevated text-sm font-semibold text-nav shadow-sm transition-colors hover:bg-brand-50 hover:text-brand-700",
            // Horizontal pill on mobile, vertical rail on md+ (sticky so it
            // tracks the canvas as you scroll).
            "w-full px-4 py-3 md:sticky md:top-4 md:h-64 md:w-12 md:flex-col md:px-0 md:py-4",
          )}
        >
          <span aria-hidden className="text-status-prevFg">
            ◀
          </span>
          <span className="md:[writing-mode:vertical-rl] md:rotate-180">
            Testing
          </span>
        </button>
      </aside>
    );
  }

  return (
    <aside
      className={clsx(
        "w-full md:shrink-0",
        // Clamp the expanded width on md+ so the canvas keeps a usable area.
        "md:w-[clamp(360px,32vw,420px)]",
      )}
    >
      <section
        className={clsx(
          "rounded-lg border border-status-prevBg bg-bg-elevated shadow-sm",
          // Independently scrollable + stick to the top while scrolling the
          // canvas alongside it.
          "md:sticky md:top-4 md:max-h-[calc(100vh-2rem)] md:overflow-y-auto",
        )}
      >
        <button
          type="button"
          onClick={() => setOpen(false)}
          aria-expanded
          className="flex w-full items-center justify-between px-4 py-3 text-sm font-semibold text-nav"
        >
          Testing
          <span aria-hidden className="text-status-prevFg">
            ▶
          </span>
        </button>

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
      </section>
    </aside>
  );
}
