"use client";

// ComponentRow (FRONTEND CONTRACT §2.5, Task 15).
//
// A single sortable component row in the outcome editor's Components list. The
// row is made sortable by @dnd-kit/sortable via `useSortable`; the drag handle
// is a real <button> (keyboard-operable, aria-labelled) per the accessibility
// notes (§7). Name/label + derived badges on the left, edit/delete on the
// right. Callbacks come from the parent list — the row itself is presentational
// + drag wiring only.
import { useSortable } from "@dnd-kit/sortable";
import { CSS } from "@dnd-kit/utilities";
import clsx from "clsx";

import { componentBadges } from "@/lib/canvas/componentBadges";
import type { ComponentRead } from "@/lib/api/components";
import type { Placement } from "@/lib/api/enums";

export interface DraftComponent extends ComponentRead {
  /** True while this row is a not-yet-persisted addition. */
  isNew?: boolean;
}

interface ComponentRowProps {
  component: DraftComponent;
  onEdit: (id: string) => void;
  onDelete: (id: string) => void;
  disabled?: boolean;
}

const BADGE_TONE: Record<string, string> = {
  type: "bg-brand-50 text-accent-onMuted",
  placement: "bg-status-stagingBg text-status-stagingFg",
  info: "bg-status-prevBg text-status-prevFg",
};

// Row-level placement (not part of `config`) gets its own badge so a sticky
// footer / pop-up component is identifiable at a glance (Task 15 Verify §3).
const PLACEMENT_BADGE: Partial<Record<Placement, string>> = {
  sticky_footer: "Sticky Footer",
  popup: "Pop-Up",
};

export function ComponentRow({
  component,
  onEdit,
  onDelete,
  disabled = false,
}: ComponentRowProps) {
  const {
    attributes,
    listeners,
    setNodeRef,
    transform,
    transition,
    isDragging,
  } = useSortable({ id: component.id, disabled });

  const style = {
    transform: CSS.Transform.toString(transform),
    transition,
  };

  const badges = componentBadges(component.config);
  const placementLabel = PLACEMENT_BADGE[component.placement];

  return (
    <li
      ref={setNodeRef}
      style={style}
      data-testid={`component-row-${component.id}`}
      className={clsx(
        "flex items-center gap-3 rounded-lg border bg-bg-elevated px-3 py-3 shadow-sm",
        isDragging
          ? "z-10 border-brand-400 shadow-md"
          : "border-status-prevBg",
      )}
    >
      <button
        type="button"
        aria-label={`Reorder ${component.slug}`}
        className="flex h-8 w-6 shrink-0 cursor-grab touch-none items-center justify-center rounded text-status-prev hover:bg-brand-50 hover:text-brand-600 focus-visible:outline-none active:cursor-grabbing disabled:cursor-not-allowed disabled:opacity-40"
        disabled={disabled}
        {...attributes}
        {...listeners}
        // After the spreads so our value wins (dnd-kit doesn't set aria-grabbed).
        aria-grabbed={isDragging}
      >
        <span aria-hidden className="text-lg leading-none">
          ⠿
        </span>
      </button>

      <div className="min-w-0 flex-1">
        <p className="truncate text-sm font-semibold text-nav">
          {component.slug}
          {component.isNew && (
            <span className="ml-2 rounded bg-brand-50 px-1.5 py-0.5 text-[10px] font-medium uppercase tracking-wide text-accent-onMuted">
              New
            </span>
          )}
        </p>
        <div className="mt-1 flex flex-wrap gap-1.5">
          {placementLabel && (
            <span
              className={clsx(
                "inline-flex items-center rounded-full px-2 py-0.5 text-[11px] font-medium",
                BADGE_TONE.placement,
              )}
            >
              {placementLabel}
            </span>
          )}
          {badges.map((b, i) => (
            <span
              key={`${b.label}-${i}`}
              className={clsx(
                "inline-flex items-center rounded-full px-2 py-0.5 text-[11px] font-medium",
                BADGE_TONE[b.tone],
              )}
            >
              {b.label}
            </span>
          ))}
        </div>
      </div>

      <div className="flex shrink-0 items-center gap-1">
        <button
          type="button"
          aria-label={`Edit ${component.slug}`}
          onClick={() => onEdit(component.id)}
          disabled={disabled}
          className="rounded-md px-2 py-1 text-xs font-medium text-action hover:bg-action/10 disabled:opacity-40"
        >
          Edit
        </button>
        <button
          type="button"
          aria-label={`Delete ${component.slug}`}
          onClick={() => onDelete(component.id)}
          disabled={disabled}
          className="rounded-md px-2 py-1 text-xs font-medium text-status-stagingFg hover:bg-status-stagingBg disabled:opacity-40"
        >
          Delete
        </button>
      </div>
    </li>
  );
}
