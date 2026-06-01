"use client";

// "Template Library" dashed button anchored bottom-right. No-op for MVP
// (disabled with a "Coming soon" tooltip). (Task 11)
import * as Tooltip from "@radix-ui/react-tooltip";

export function TemplateLibraryButton() {
  return (
    <Tooltip.Provider delayDuration={200}>
      <Tooltip.Root>
        <Tooltip.Trigger asChild>
          <button
            type="button"
            disabled
            aria-disabled="true"
            className="flex items-center gap-1.5 rounded-md border border-dashed border-status-prevBg bg-bg-elevated/80 px-3 py-2 text-sm font-medium text-status-prevFg opacity-70"
          >
            <span aria-hidden>＋</span>
            Template Library
          </button>
        </Tooltip.Trigger>
        <Tooltip.Portal>
          <Tooltip.Content
            side="top"
            className="rounded bg-nav px-2 py-1 text-xs text-nav-fg shadow-md"
          >
            Coming soon
            <Tooltip.Arrow className="fill-nav" />
          </Tooltip.Content>
        </Tooltip.Portal>
      </Tooltip.Root>
    </Tooltip.Provider>
  );
}
