"use client";

// Client component: controlled Radix Dialog that PATCHes a version's
// description (BACKEND CONTRACT §5 VersionUpdate). Open state is owned by the
// parent row so the RowActionsMenu can trigger it. On success it invalidates
// the versions list query.
import { useEffect, useState, type FormEvent } from "react";
import * as Dialog from "@radix-ui/react-dialog";
import { useMutation, useQueryClient } from "@tanstack/react-query";

import { ErrorBanner } from "@/components/ui/ErrorBanner";
import { updateVersion } from "@/lib/api/versions";
import { toUserError, type UserError } from "@/lib/errors/userError";

interface EditDescriptionDialogProps {
  featureId: string;
  versionNumber: number;
  initialDescription: string | null;
  open: boolean;
  onOpenChange: (open: boolean) => void;
}

export function EditDescriptionDialog({
  featureId,
  versionNumber,
  initialDescription,
  open,
  onOpenChange,
}: EditDescriptionDialogProps) {
  const queryClient = useQueryClient();
  const [description, setDescription] = useState(initialDescription ?? "");
  const [error, setError] = useState<UserError | null>(null);

  // Re-sync the field whenever the dialog (re)opens for a row.
  useEffect(() => {
    if (open) {
      setDescription(initialDescription ?? "");
      setError(null);
    }
  }, [open, initialDescription]);

  const mutation = useMutation({
    mutationFn: () =>
      updateVersion(featureId, versionNumber, {
        description: description.trim(),
      }),
    onSuccess: () => {
      void queryClient.invalidateQueries({ queryKey: ["versions", featureId] });
      void queryClient.invalidateQueries({
        queryKey: ["version", featureId, versionNumber],
      });
      onOpenChange(false);
    },
    onError: (e: unknown) => {
      setError(toUserError(e, { surface: "save" }));
    },
  });

  function handleSubmit(e: FormEvent) {
    e.preventDefault();
    setError(null);
    mutation.mutate();
  }

  return (
    <Dialog.Root
      open={open}
      onOpenChange={(next) => {
        if (!mutation.isPending) onOpenChange(next);
      }}
    >
      <Dialog.Portal>
        <Dialog.Overlay className="fixed inset-0 z-40 bg-black/60 backdrop-blur-sm" />
        <Dialog.Content className="fixed left-1/2 top-1/2 z-50 w-[90vw] max-w-md -translate-x-1/2 -translate-y-1/2 rounded-xl bg-bg-elevated p-6 shadow-xl focus:outline-none">
          <Dialog.Title className="text-lg font-semibold text-nav">
            Edit Description · V{versionNumber}
          </Dialog.Title>
          <Dialog.Description className="mt-1 text-sm text-status-prevFg">
            Update the description for this version.
          </Dialog.Description>

          <form onSubmit={handleSubmit} className="mt-4 space-y-4">
            <textarea
              aria-label="Version description"
              value={description}
              onChange={(e) => setDescription(e.target.value)}
              maxLength={2000}
              rows={3}
              className="w-full rounded-lg border border-status-prevBg px-3 py-2 text-sm text-nav placeholder:text-status-prev focus:border-brand-500 focus:outline-none focus:ring-1 focus:ring-brand-500"
            />

            {error && <ErrorBanner error={error} />}

            <div className="flex justify-end gap-2">
              <Dialog.Close asChild>
                <button
                  type="button"
                  disabled={mutation.isPending}
                  className="rounded-lg border border-status-prevBg px-3.5 py-2 text-sm font-medium text-nav hover:bg-status-prevBg disabled:opacity-50"
                >
                  Cancel
                </button>
              </Dialog.Close>
              <button
                type="submit"
                disabled={mutation.isPending}
                className="rounded-lg bg-action px-3.5 py-2 text-sm font-semibold text-white hover:bg-action-700 disabled:opacity-50"
              >
                {mutation.isPending ? "Saving…" : "Save"}
              </button>
            </div>
          </form>
        </Dialog.Content>
      </Dialog.Portal>
    </Dialog.Root>
  );
}
