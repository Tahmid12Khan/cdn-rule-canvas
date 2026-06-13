"use client";

// Controlled Radix Dialog confirming deletion of a whole Component (all its
// versions). The list client owns the open state + targeted component. Mirrors
// TestPresetDeleteDialog / SiteDeleteDialog.
import { useState } from "react";
import * as Dialog from "@radix-ui/react-dialog";

import { ErrorBanner } from "@/components/ui/ErrorBanner";
import { useDeleteComponentTemplate } from "@/lib/api/componentTemplates";
import type { ComponentTemplateSummary } from "@/lib/schemas/componentTemplates";
import { toUserError, type UserError } from "@/lib/errors/userError";

interface ComponentDeleteDialogProps {
  component: ComponentTemplateSummary | null;
  open: boolean;
  onOpenChange: (open: boolean) => void;
}

export function ComponentDeleteDialog({
  component,
  open,
  onOpenChange,
}: ComponentDeleteDialogProps) {
  const [error, setError] = useState<UserError | null>(null);
  const mutation = useDeleteComponentTemplate();

  return (
    <Dialog.Root
      open={open}
      onOpenChange={(next) => {
        if (!mutation.isPending) {
          setError(null);
          onOpenChange(next);
        }
      }}
    >
      <Dialog.Portal>
        <Dialog.Overlay className="fixed inset-0 z-40 bg-black/60 backdrop-blur-sm" />
        <Dialog.Content className="fixed left-1/2 top-1/2 z-50 w-[90vw] max-w-md -translate-x-1/2 -translate-y-1/2 rounded-xl bg-bg-elevated p-6 shadow-xl focus:outline-none">
          <Dialog.Title className="text-lg font-semibold text-fg">
            Delete component {component?.name}?
          </Dialog.Title>
          <Dialog.Description className="mt-1 text-sm text-fg-muted">
            This removes the component{" "}
            <code className="font-mono text-fg-muted">{component?.slug}</code>{" "}
            and all its versions. Rules referencing it will fail open. This
            cannot be undone.
          </Dialog.Description>

          {error && (
            <div className="mt-4">
              <ErrorBanner error={error} />
            </div>
          )}

          <div className="mt-6 flex justify-end gap-2">
            <Dialog.Close asChild>
              <button
                type="button"
                disabled={mutation.isPending}
                className="rounded-lg border border-border px-3.5 py-2 text-sm font-medium text-fg hover:bg-bg-overlay disabled:opacity-50"
              >
                Cancel
              </button>
            </Dialog.Close>
            <button
              type="button"
              onClick={() => {
                if (!component) return;
                setError(null);
                mutation.mutate(component.id, {
                  onSuccess: () => onOpenChange(false),
                  onError: (e) =>
                    setError(toUserError(e, { surface: "delete" })),
                });
              }}
              disabled={mutation.isPending}
              className="rounded-lg bg-danger px-3.5 py-2 text-sm font-semibold text-white hover:opacity-90 disabled:opacity-50"
            >
              {mutation.isPending ? "Deleting…" : "Delete"}
            </button>
          </div>
        </Dialog.Content>
      </Dialog.Portal>
    </Dialog.Root>
  );
}
