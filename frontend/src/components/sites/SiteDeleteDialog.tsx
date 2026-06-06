"use client";

// Client component: controlled Radix Dialog confirming deletion of a Site
// (spec §6). Mirrors versions/ConfirmDeleteDialog. The list client owns the
// open state + which site is targeted.
import { useState } from "react";
import * as Dialog from "@radix-ui/react-dialog";
import { useMutation, useQueryClient } from "@tanstack/react-query";

import { ErrorBanner } from "@/components/ui/ErrorBanner";
import { deleteSite, type SiteRead } from "@/lib/api/sites";
import { toUserError, type UserError } from "@/lib/errors/userError";

interface SiteDeleteDialogProps {
  site: SiteRead | null;
  open: boolean;
  onOpenChange: (open: boolean) => void;
}

export function SiteDeleteDialog({
  site,
  open,
  onOpenChange,
}: SiteDeleteDialogProps) {
  const queryClient = useQueryClient();
  const [error, setError] = useState<UserError | null>(null);

  const mutation = useMutation({
    mutationFn: () => {
      if (!site) throw new Error("No site selected");
      return deleteSite(site.slug);
    },
    onSuccess: () => {
      void queryClient.invalidateQueries({ queryKey: ["sites"] });
      onOpenChange(false);
    },
    onError: (e: unknown) => {
      setError(toUserError(e, { surface: "delete" }));
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
            Delete Site {site?.name}?
          </Dialog.Title>
          <Dialog.Description className="mt-1 text-sm text-status-prevFg">
            This removes the routing for{" "}
            <code className="font-mono text-fg-muted">
              {site
                ? `${site.source_protocol}://${site.source_host}:${site.source_port}`
                : ""}
            </code>
            . This action cannot be undone.
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
                setError(null);
                mutation.mutate();
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
