"use client";

// Make-Live confirm dialog (Phase 2 W3). Publishes a specific version to the
// LIVE environment after an explicit, production-affecting confirmation. The
// backend promotes the target → live, demotes the prior live → prev, and leaves
// the newest draft untouched. Open state is owned by the parent (RowActionsMenu
// row or the version detail header) so the trigger can live anywhere.
import { useState } from "react";
import * as Dialog from "@radix-ui/react-dialog";
import { useMutation, useQueryClient } from "@tanstack/react-query";

import { ErrorBanner } from "@/components/ui/ErrorBanner";
import { publishVersion } from "@/lib/api/versions";
import { toUserError, type UserError } from "@/lib/errors/userError";

interface MakeLiveDialogProps {
  featureId: string;
  versionNumber: number;
  open: boolean;
  onOpenChange: (open: boolean) => void;
}

export function MakeLiveDialog({
  featureId,
  versionNumber,
  open,
  onOpenChange,
}: MakeLiveDialogProps) {
  const queryClient = useQueryClient();
  const [error, setError] = useState<UserError | null>(null);

  const mutation = useMutation({
    mutationFn: () =>
      publishVersion(featureId, versionNumber, { environment: "live" }),
    onSuccess: () => {
      // Target → live, prior live → prev, newest draft unchanged. Refresh the
      // list, the feature (holds live_version_id), and this version's detail.
      void queryClient.invalidateQueries({ queryKey: ["versions", featureId] });
      void queryClient.invalidateQueries({ queryKey: ["feature", featureId] });
      void queryClient.invalidateQueries({
        queryKey: ["version", featureId, versionNumber],
      });
      setError(null);
      onOpenChange(false);
    },
    onError: (e: unknown) => {
      setError(toUserError(e, { surface: "publish" }));
    },
  });

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
          <Dialog.Title className="text-lg font-semibold text-nav">
            Publish version {versionNumber} to production?
          </Dialog.Title>
          <Dialog.Description className="mt-1 text-sm text-status-prevFg">
            This replaces the current live version and takes effect immediately.
            The previously live version becomes &ldquo;prev&rdquo;.
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
                className="rounded-lg border border-status-prevBg px-3.5 py-2 text-sm font-medium text-nav hover:bg-status-prevBg disabled:opacity-50"
              >
                Cancel
              </button>
            </Dialog.Close>
            <button
              type="button"
              onClick={() => {
                // Re-entry guard: avoid a second publish call from a fast
                // double-click before isPending flips (consistent with the
                // SaveAsNewVersion dialog; publish itself 409s but stay safe).
                if (mutation.isPending) return;
                setError(null);
                mutation.mutate();
              }}
              disabled={mutation.isPending}
              className="rounded-lg bg-status-live px-3.5 py-2 text-sm font-semibold text-white hover:opacity-90 disabled:opacity-50"
            >
              {mutation.isPending ? "Publishing…" : "Make Live"}
            </button>
          </div>
        </Dialog.Content>
      </Dialog.Portal>
    </Dialog.Root>
  );
}
