"use client";

// Client component: per-row overflow menu (Radix DropdownMenu). Items are gated
// by version status per BACKEND CONTRACT §7 status lifecycle:
//   - Make Live: enabled for any version that is NOT already live (W3)
//   - Unpublish: enabled only for LIVE | STAGING
//   - Edit Description: always
//   - Delete: enabled only for DRAFT | PREV
// Unpublish runs inline (env derived from status); Make Live / Edit / Delete
// open the parent dialogs via callbacks.
import { useState } from "react";
import * as DropdownMenu from "@radix-ui/react-dropdown-menu";
import { useMutation, useQueryClient } from "@tanstack/react-query";
import clsx from "clsx";

import { ErrorBanner } from "@/components/ui/ErrorBanner";
import type { VersionStatus } from "@/lib/api/enums";
import { unpublishVersion } from "@/lib/api/versions";
import { toUserError, type UserError } from "@/lib/errors/userError";

interface RowActionsMenuProps {
  featureId: string;
  versionNumber: number;
  status: VersionStatus;
  onMakeLive: () => void;
  onEditDescription: () => void;
  onDelete: () => void;
}

const ITEM_CLASS =
  "flex w-full cursor-pointer select-none items-center rounded-md px-2.5 py-2 text-sm text-nav outline-none data-[highlighted]:bg-status-prevBg data-[disabled]:cursor-not-allowed data-[disabled]:text-status-prev data-[disabled]:opacity-60";

export function RowActionsMenu({
  featureId,
  versionNumber,
  status,
  onMakeLive,
  onEditDescription,
  onDelete,
}: RowActionsMenuProps) {
  const queryClient = useQueryClient();
  // Surface unpublish failures (409/5xx/network) instead of failing silently on
  // this production-affecting action (toUserError + ErrorBanner pattern). The
  // error banner renders below the (now-closed) menu, so close the menu on
  // error too — otherwise Radix's open-menu aria-hidden swallows the alert.
  const [open, setOpen] = useState(false);
  const [unpublishError, setUnpublishError] = useState<UserError | null>(null);

  const canMakeLive = status !== "live";
  const canUnpublish = status === "live" || status === "staging";
  const canDelete = status === "draft" || status === "prev";

  const unpublish = useMutation({
    mutationFn: () =>
      unpublishVersion(featureId, versionNumber, {
        environment: status === "live" ? "live" : "staging",
      }),
    onSuccess: () => {
      setUnpublishError(null);
      void queryClient.invalidateQueries({ queryKey: ["versions", featureId] });
      void queryClient.invalidateQueries({ queryKey: ["feature", featureId] });
    },
    onError: (err) => {
      setOpen(false);
      setUnpublishError(toUserError(err, { surface: "publish" }));
    },
  });

  return (
    <DropdownMenu.Root open={open} onOpenChange={setOpen}>
      <div className="relative inline-block">
        <DropdownMenu.Trigger asChild>
          <button
            type="button"
            aria-label={`Actions for version ${versionNumber}`}
            className="inline-flex h-8 w-8 items-center justify-center rounded-md text-status-prevFg hover:bg-status-prevBg focus-visible:outline focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-brand-500"
          >
            <span aria-hidden className="text-lg leading-none">
              ⋯
            </span>
          </button>
        </DropdownMenu.Trigger>

        {unpublishError && (
          <div className="absolute right-0 top-full z-50 mt-2 w-72 text-left">
            <ErrorBanner
              error={unpublishError}
              onRetry={
                unpublishError.retryable
                  ? () => unpublish.mutate()
                  : undefined
              }
            />
            <button
              type="button"
              onClick={() => setUnpublishError(null)}
              className="mt-1 text-xs font-medium text-status-prevFg underline hover:text-nav"
            >
              Dismiss
            </button>
          </div>
        )}
      </div>

      <DropdownMenu.Portal>
        <DropdownMenu.Content
          align="end"
          sideOffset={4}
          className={clsx(
            "z-50 min-w-[12rem] rounded-lg border border-status-prevBg bg-bg-elevated p-1 shadow-lg",
          )}
        >
          <DropdownMenu.Item
            disabled={!canMakeLive}
            onSelect={(e) => {
              e.preventDefault();
              onMakeLive();
            }}
            className={ITEM_CLASS}
          >
            Make Live
          </DropdownMenu.Item>

          <DropdownMenu.Item
            disabled={!canUnpublish || unpublish.isPending}
            onSelect={(e) => {
              e.preventDefault();
              unpublish.mutate();
            }}
            className={ITEM_CLASS}
          >
            Unpublish
          </DropdownMenu.Item>

          <DropdownMenu.Item
            onSelect={(e) => {
              e.preventDefault();
              onEditDescription();
            }}
            className={ITEM_CLASS}
          >
            Edit Description
          </DropdownMenu.Item>

          <DropdownMenu.Separator className="my-1 h-px bg-status-prevBg" />

          <DropdownMenu.Item
            disabled={!canDelete}
            onSelect={(e) => {
              e.preventDefault();
              onDelete();
            }}
            className={clsx(
              ITEM_CLASS,
              canDelete && "text-danger data-[highlighted]:bg-danger-bg",
            )}
          >
            Delete
          </DropdownMenu.Item>
        </DropdownMenu.Content>
      </DropdownMenu.Portal>
    </DropdownMenu.Root>
  );
}
