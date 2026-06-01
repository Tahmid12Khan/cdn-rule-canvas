"use client";

// Client component: Radix Dialog for creating a new draft version. The backend
// seeds the rule_graph + builtin outcome; this form only collects an optional
// description (BACKEND CONTRACT §5 VersionCreate). On success it invalidates the
// versions + feature queries (deployment pills are unaffected by create, but
// the feature query is cheap to refresh).
import { useState, type FormEvent } from "react";
import * as Dialog from "@radix-ui/react-dialog";
import { useMutation, useQueryClient } from "@tanstack/react-query";

import { ErrorBanner } from "@/components/ui/ErrorBanner";
import { createVersion, VersionCreate } from "@/lib/api/versions";
import { toUserError, type UserError } from "@/lib/errors/userError";
import { useOnboardingStore } from "@/state/onboardingStore";

interface AddVersionDialogProps {
  featureId: string;
}

export function AddVersionDialog({ featureId }: AddVersionDialogProps) {
  const queryClient = useQueryClient();
  const completeOnboarding = useOnboardingStore((s) => s.complete);
  const [open, setOpen] = useState(false);
  const [description, setDescription] = useState("");
  const [error, setError] = useState<UserError | null>(null);

  const mutation = useMutation({
    mutationFn: () => {
      const trimmed = description.trim();
      const body = VersionCreate.parse({
        description: trimmed.length > 0 ? trimmed : undefined,
      });
      return createVersion(featureId, body);
    },
    onSuccess: () => {
      void queryClient.invalidateQueries({ queryKey: ["versions", featureId] });
      void queryClient.invalidateQueries({ queryKey: ["feature", featureId] });
      completeOnboarding("version");
      setDescription("");
      setError(null);
      setOpen(false);
    },
    onError: (e: unknown) => {
      setError(toUserError(e, { surface: "create" }));
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
        if (!mutation.isPending) {
          setOpen(next);
          if (!next) {
            setDescription("");
            setError(null);
          }
        }
      }}
    >
      <Dialog.Trigger asChild>
        <button
          type="button"
          className="inline-flex items-center gap-1.5 rounded-lg bg-action px-3.5 py-2 text-sm font-semibold text-white hover:bg-action-700 focus-visible:outline focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-action"
        >
          <span aria-hidden className="text-base leading-none">
            +
          </span>
          Add A New Version
        </button>
      </Dialog.Trigger>

      <Dialog.Portal>
        <Dialog.Overlay className="fixed inset-0 z-40 bg-black/60 backdrop-blur-sm data-[state=open]:animate-in" />
        <Dialog.Content className="fixed left-1/2 top-1/2 z-50 w-[90vw] max-w-md -translate-x-1/2 -translate-y-1/2 rounded-xl bg-bg-elevated p-6 shadow-xl focus:outline-none">
          <Dialog.Title className="text-lg font-semibold text-nav">
            Add A New Version
          </Dialog.Title>
          <Dialog.Description className="mt-1 text-sm text-status-prevFg">
            A new draft version will be created from the current live rules.
          </Dialog.Description>

          <form onSubmit={handleSubmit} className="mt-4 space-y-4">
            <div>
              <label
                htmlFor="version-description"
                className="block text-sm font-medium text-nav"
              >
                Description{" "}
                <span className="font-normal text-status-prev">(optional)</span>
              </label>
              <textarea
                id="version-description"
                value={description}
                onChange={(e) => setDescription(e.target.value)}
                maxLength={2000}
                rows={3}
                placeholder="What changed in this version?"
                className="mt-1 w-full rounded-lg border border-status-prevBg px-3 py-2 text-sm text-nav placeholder:text-status-prev focus:border-brand-500 focus:outline-none focus:ring-1 focus:ring-brand-500"
              />
            </div>

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
                {mutation.isPending ? "Creating…" : "Create Version"}
              </button>
            </div>
          </form>
        </Dialog.Content>
      </Dialog.Portal>
    </Dialog.Root>
  );
}
