"use client";

// A palette chip. Enabled chips are HTML5 drag sources that stash a JSON
// ChipPayload on the DataTransfer; disabled chips render with a "Coming soon"
// tooltip and are not draggable. (Task 12)
import * as Tooltip from "@radix-ui/react-tooltip";
import clsx from "clsx";

import { CHIP_MIME, type NodeChipDef } from "@/lib/canvas/nodeTemplates";

interface NodeChipProps {
  chip: NodeChipDef;
  // When false (view mode), even enabled chips are not draggable.
  draggable: boolean;
}

export function NodeChip({ chip, draggable }: NodeChipProps) {
  const interactive = chip.enabled && draggable;

  function onDragStart(e: React.DragEvent) {
    if (!interactive || !chip.payload) return;
    e.dataTransfer.setData(CHIP_MIME, JSON.stringify(chip.payload));
    e.dataTransfer.effectAllowed = "copy";
  }

  const chipEl = (
    <div
      role="button"
      tabIndex={chip.enabled ? 0 : -1}
      aria-disabled={chip.enabled ? undefined : "true"}
      draggable={interactive}
      onDragStart={onDragStart}
      data-chip-id={chip.id}
      data-enabled={chip.enabled}
      className={clsx(
        "select-none whitespace-nowrap rounded-full border px-3 py-1.5 text-sm font-medium transition-colors",
        chip.enabled
          ? "cursor-grab border-action-600 bg-action-600 text-white hover:bg-action-700 active:cursor-grabbing"
          : "cursor-not-allowed border-status-prevBg bg-bg-elevated text-status-prev opacity-60",
      )}
    >
      {chip.label}
    </div>
  );

  if (chip.enabled) return chipEl;

  return (
    <Tooltip.Provider delayDuration={200}>
      <Tooltip.Root>
        <Tooltip.Trigger asChild>{chipEl}</Tooltip.Trigger>
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
