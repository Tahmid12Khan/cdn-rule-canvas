"use client";

// AddComponentDrawer (FRONTEND CONTRACT §2.5, Task 15).
//
// "+ Add A Component Or Form" dashed trigger that opens a Radix Dialog side
// drawer listing the creatable component types (html_injection,
// content_truncation, html_remove, component_ref). Picking a type closes the drawer and reports the choice
// to the parent, which appends a new draft row and opens the config modal
// (Task 16). Radix gives us focus trap + Escape close for free (§7).
import { useState } from "react";
import * as Dialog from "@radix-ui/react-dialog";

import type { ComponentType } from "@/lib/schemas/components";

interface ComponentTypeOption {
  type: ComponentType;
  title: string;
  description: string;
}

const HTML_OPTIONS: ComponentTypeOption[] = [
  {
    type: "html_injection",
    title: "HTML Injection",
    description:
      "Insert custom HTML into the page relative to a target selector.",
  },
  {
    type: "content_truncation",
    title: "Content Truncation",
    description:
      "Trim content to a word budget, optionally with a fade-out gradient.",
  },
  {
    type: "html_remove",
    title: "HTML Remove",
    description:
      "Delete a target element's contents — or the element itself — injecting nothing.",
  },
  {
    type: "component_ref",
    title: "Component",
    description:
      "Render a reusable library Component (versioned, with variables) into the page.",
  },
];

const JSON_OPTIONS: ComponentTypeOption[] = [
  {
    type: "json_remove",
    title: "JSON Remove",
    description: "Delete the value at a path in the JSON response body.",
  },
  {
    type: "json_set",
    title: "JSON Set",
    description: "Upsert (create or overwrite) a value at a path.",
  },
  {
    type: "json_replace",
    title: "JSON Replace",
    description: "Overwrite a value only if the path already exists.",
  },
  {
    type: "component_ref_json",
    title: "Component",
    description:
      "Render a library Component to an HTML string and set it at a JSON path.",
  },
];

interface AddComponentDrawerProps {
  onPick: (type: ComponentType) => void;
  disabled?: boolean;
  // Feature content kind — picks the offered component types (req JSON).
  featureType?: "html" | "json";
}

export function AddComponentDrawer({
  onPick,
  disabled = false,
  featureType = "html",
}: AddComponentDrawerProps) {
  const [open, setOpen] = useState(false);
  const options = featureType === "json" ? JSON_OPTIONS : HTML_OPTIONS;

  function handlePick(type: ComponentType) {
    setOpen(false);
    onPick(type);
  }

  return (
    <Dialog.Root open={open} onOpenChange={setOpen}>
      <Dialog.Trigger asChild>
        <button
          type="button"
          disabled={disabled}
          className="flex w-full items-center justify-center gap-2 rounded-lg border-2 border-dashed border-status-draftBorder px-4 py-4 text-sm font-medium text-status-prevFg transition-colors hover:border-brand-400 hover:bg-brand-50 hover:text-brand-700 disabled:cursor-not-allowed disabled:opacity-50"
        >
          <span aria-hidden className="text-lg leading-none">
            +
          </span>
          Add A Component Or Form
        </button>
      </Dialog.Trigger>

      <Dialog.Portal>
        <Dialog.Overlay className="fixed inset-0 z-40 bg-black/60 backdrop-blur-sm" />
        <Dialog.Content className="fixed right-0 top-0 z-50 flex h-full w-full max-w-md flex-col bg-bg-elevated shadow-xl focus:outline-none">
          <div className="flex items-start justify-between border-b border-status-prevBg px-6 py-4">
            <div>
              <Dialog.Title className="text-lg font-semibold text-nav">
                Add a component
              </Dialog.Title>
              <Dialog.Description className="mt-1 text-sm text-status-prevFg">
                Choose a component type to add to this outcome.
              </Dialog.Description>
            </div>
            <Dialog.Close asChild>
              <button
                type="button"
                aria-label="Close"
                className="rounded-md p-1 text-status-prev hover:bg-status-prevBg"
              >
                <span aria-hidden className="text-xl leading-none">
                  ×
                </span>
              </button>
            </Dialog.Close>
          </div>

          <div className="flex flex-col gap-3 overflow-y-auto px-6 py-5">
            {options.map((option) => (
              <button
                key={option.type}
                type="button"
                onClick={() => handlePick(option.type)}
                className="rounded-lg border border-status-prevBg px-4 py-3 text-left transition-colors hover:border-brand-400 hover:bg-brand-50 focus-visible:outline-none"
              >
                <p className="text-sm font-semibold text-nav">
                  {option.title}
                </p>
                <p className="mt-1 text-xs text-status-prevFg">
                  {option.description}
                </p>
              </button>
            ))}
          </div>
        </Dialog.Content>
      </Dialog.Portal>
    </Dialog.Root>
  );
}
