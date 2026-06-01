"use client";

// "Save as New Version" dialog (Task 14 / Phase 2 W2): prompts for a description
// AND a Draft/Live choice, then the caller POSTs a new version seeded with the
// current canvas state — and, if Live was chosen, immediately publishes it to
// production. On success the caller navigates to the new version page.
import { useState } from "react";
import * as Dialog from "@radix-ui/react-dialog";
import clsx from "clsx";

import { StatusPill } from "@/components/ui/StatusPill";

export type NewVersionStatus = "draft" | "live";

interface SaveAsNewVersionDialogProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  saving: boolean;
  onConfirm: (description: string, status: NewVersionStatus) => void;
}

const CHOICES: {
  value: NewVersionStatus;
  label: string;
  consequence: string;
}[] = [
  {
    value: "draft",
    label: "Draft",
    consequence: "Saved as an editable draft.",
  },
  {
    value: "live",
    label: "Live",
    consequence: "Publishes the new version to production immediately.",
  },
];

export function SaveAsNewVersionDialog({
  open,
  onOpenChange,
  saving,
  onConfirm,
}: SaveAsNewVersionDialogProps) {
  const [description, setDescription] = useState("");
  const [status, setStatus] = useState<NewVersionStatus>("draft");

  const selected = CHOICES.find((c) => c.value === status) ?? CHOICES[0];
  const confirmLabel =
    status === "live" ? "Create & Publish Live" : "Create Version";

  return (
    <Dialog.Root open={open} onOpenChange={onOpenChange}>
      <Dialog.Portal>
        <Dialog.Overlay className="fixed inset-0 z-40 bg-black/60 backdrop-blur-sm" />
        <Dialog.Content className="fixed left-1/2 top-1/2 z-50 w-full max-w-md -translate-x-1/2 -translate-y-1/2 rounded-lg bg-bg-elevated p-6 shadow-xl focus:outline-none">
          <Dialog.Title className="text-lg font-semibold text-nav">
            Save as New Version
          </Dialog.Title>
          <Dialog.Description className="mt-1 text-sm text-status-prevFg">
            Creates a new version from the current canvas state.
          </Dialog.Description>

          <label
            htmlFor="new-version-description"
            className="mt-4 block text-sm font-medium text-nav"
          >
            Description
          </label>
          <textarea
            id="new-version-description"
            value={description}
            onChange={(e) => setDescription(e.target.value)}
            rows={3}
            maxLength={2000}
            placeholder="What changed in this version?"
            className="mt-1 w-full rounded-md border border-status-prevBg px-3 py-2 text-sm focus:border-brand-500"
          />

          <fieldset className="mt-4">
            <legend className="text-sm font-medium text-nav">Save as</legend>
            <div
              role="radiogroup"
              aria-label="New version status"
              className="mt-2 inline-flex rounded-lg border border-status-prevBg bg-status-prevBg/20 p-0.5"
            >
              {CHOICES.map((choice) => {
                const active = status === choice.value;
                return (
                  <button
                    key={choice.value}
                    type="button"
                    role="radio"
                    aria-checked={active}
                    onClick={() => setStatus(choice.value)}
                    className={clsx(
                      "rounded-md px-4 py-1.5 text-sm font-semibold transition-colors",
                      active
                        ? "bg-bg-elevated text-nav shadow-sm"
                        : "text-status-prevFg hover:text-nav",
                    )}
                  >
                    {choice.label}
                  </button>
                );
              })}
            </div>
            <div className="mt-3 flex items-center gap-2 text-sm text-status-prevFg">
              <span>Result:</span>
              <StatusPill status={status} />
              <span>{selected.consequence}</span>
            </div>
          </fieldset>

          {status === "live" && (
            <p
              role="alert"
              className="mt-3 rounded-md border border-status-stagingFg bg-status-stagingBg px-3 py-2 text-xs font-medium text-status-stagingFg"
            >
              This will replace the current live version and take effect in
              production immediately.
            </p>
          )}

          <div className="mt-5 flex justify-end gap-2">
            <Dialog.Close asChild>
              <button
                type="button"
                className="rounded-md border border-status-prevBg px-4 py-2 text-sm font-medium text-status-prevFg hover:bg-bg-overlay"
              >
                Cancel
              </button>
            </Dialog.Close>
            <button
              type="button"
              onClick={() => {
                // Re-entry guard: `disabled` alone races a fast double-click /
                // Enter+click because isPending only flips after this synchronous
                // handler runs — without this a second call creates a duplicate
                // draft version.
                if (saving) return;
                onConfirm(description.trim(), status);
              }}
              disabled={saving}
              className={clsx(
                "rounded-md px-4 py-2 text-sm font-semibold text-white disabled:opacity-50",
                status === "live"
                  ? "bg-status-live hover:opacity-90"
                  : "bg-brand-500 hover:bg-brand-600",
              )}
            >
              {saving ? "Creating…" : confirmLabel}
            </button>
          </div>
        </Dialog.Content>
      </Dialog.Portal>
    </Dialog.Root>
  );
}
