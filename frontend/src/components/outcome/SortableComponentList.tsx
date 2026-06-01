"use client";

// SortableComponentList (FRONTEND CONTRACT §2.5, Task 15).
//
// Wraps the component rows in a @dnd-kit DndContext + vertical SortableContext.
// Pointer + keyboard sensors are enabled so the list is fully reorderable from
// the keyboard (drag handle is a focusable <button> in ComponentRow). On drop
// it computes the new order with arrayMove and reports it to the parent, which
// owns the order state + persistence (batched reorder on Save).
import {
  DndContext,
  KeyboardSensor,
  PointerSensor,
  closestCenter,
  useSensor,
  useSensors,
  type DragEndEvent,
} from "@dnd-kit/core";
import {
  SortableContext,
  arrayMove,
  sortableKeyboardCoordinates,
  verticalListSortingStrategy,
} from "@dnd-kit/sortable";

import { ComponentRow, type DraftComponent } from "./ComponentRow";

interface SortableComponentListProps {
  components: DraftComponent[];
  onReorder: (next: DraftComponent[]) => void;
  onEdit: (id: string) => void;
  onDelete: (id: string) => void;
  disabled?: boolean;
}

export function SortableComponentList({
  components,
  onReorder,
  onEdit,
  onDelete,
  disabled = false,
}: SortableComponentListProps) {
  const sensors = useSensors(
    useSensor(PointerSensor, {
      activationConstraint: { distance: 4 },
    }),
    useSensor(KeyboardSensor, {
      coordinateGetter: sortableKeyboardCoordinates,
    }),
  );

  function handleDragEnd(event: DragEndEvent) {
    const { active, over } = event;
    if (!over || active.id === over.id) return;

    const oldIndex = components.findIndex((c) => c.id === active.id);
    const newIndex = components.findIndex((c) => c.id === over.id);
    if (oldIndex === -1 || newIndex === -1) return;

    onReorder(arrayMove(components, oldIndex, newIndex));
  }

  if (components.length === 0) {
    return (
      <p className="rounded-lg border border-dashed border-status-prevBg px-4 py-8 text-center text-sm text-status-prevFg">
        No components yet. Add one below.
      </p>
    );
  }

  return (
    <DndContext
      sensors={sensors}
      collisionDetection={closestCenter}
      onDragEnd={handleDragEnd}
      accessibility={{
        announcements: {
          onDragStart: ({ active }) => `Picked up component ${active.id}.`,
          onDragOver: ({ active, over }) =>
            over
              ? `Component ${active.id} is over position ${over.id}.`
              : `Component ${active.id} is no longer over a drop target.`,
          onDragEnd: ({ active, over }) =>
            over
              ? `Component ${active.id} dropped at position ${over.id}.`
              : `Component ${active.id} dropped.`,
          onDragCancel: ({ active }) =>
            `Reordering cancelled. Component ${active.id} returned to its position.`,
        },
      }}
    >
      <SortableContext
        items={components.map((c) => c.id)}
        strategy={verticalListSortingStrategy}
      >
        <ul className="flex flex-col gap-2">
          {components.map((component) => (
            <ComponentRow
              key={component.id}
              component={component}
              onEdit={onEdit}
              onDelete={onDelete}
              disabled={disabled}
            />
          ))}
        </ul>
      </SortableContext>
    </DndContext>
  );
}
