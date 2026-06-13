"use client";

// Create-version modal (design §5.3 version bar). A new version clones the
// current default's body/variables server-side unless supplied; here the author
// only enters a description + chooses whether it becomes the default. "Make
// default" defaults ON because "latest is generally default" (design §2).
import { useState } from "react";
import * as Dialog from "@radix-ui/react-dialog";

import { ApiError } from "@/lib/api/client";
import { useCreateVersion } from "@/lib/api/componentTemplates";
import { toUserError } from "@/lib/errors/userError";

const inputClass =
  "w-full rounded-md border border-border bg-bg px-3 py-2 text-sm text-fg shadow-sm focus:border-accent focus:outline-none focus:ring-1 focus:ring-accent";

interface CreateVersionModalProps {
  componentId: string;
  open: boolean;
  onOpenChange: (open: boolean) => void;
  // Notify the parent of the new version number so it can switch to it.
  onCreated: (versionNumber: number) => void;
}

export function CreateVersionModal({
  componentId,
  open,
  onOpenChange,
  onCreated,
}: CreateVersionModalProps) {
  const [description, setDescription] = useState("");
  const [makeDefault, setMakeDefault] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const mutation = useCreateVersion(componentId);

  function handleSubmit(event: React.FormEvent<HTMLFormElement>) {
    event.preventDefault();
    setError(null);
    mutation.mutate(
      {
        description: description.trim() || undefined,
        make_default: makeDefault,
      },
      {
        onSuccess: (created) => {
          setDescription("");
          setMakeDefault(true);
          onCreated(created.version_number);
          onOpenChange(false);
        },
        onError: (err: unknown) => {
          if (err instanceof ApiError) {
            setError(err.message);
            return;
          }
          const ue = toUserError(err, { surface: "create" });
          setError(`${ue.title}. ${ue.howToFix}`);
        },
      },
    );
  }

  return (
    <Dialog.Root open={open} onOpenChange={onOpenChange}>
      <Dialog.Portal>
        <Dialog.Overlay className="fixed inset-0 z-40 bg-black/60 backdrop-blur-sm" />
        <Dialog.Content className="fixed left-1/2 top-1/2 z-50 w-full max-w-md -translate-x-1/2 -translate-y-1/2 rounded-lg bg-bg-elevated p-6 shadow-xl focus:outline-none">
          <Dialog.Title className="text-lg font-semibold text-fg">
            Create a new version
          </Dialog.Title>
          <Dialog.Description className="mt-1 text-sm text-fg-muted">
            The new version starts as a copy of the current default. You can edit
            its HTML afterwards.
          </Dialog.Description>

          <form className="mt-5 flex flex-col gap-4" onSubmit={handleSubmit}>
            <label className="flex flex-col gap-1">
              <span className="text-sm font-medium text-fg">
                Description (optional)
              </span>
              <input
                value={description}
                onChange={(e) => setDescription(e.target.value)}
                placeholder="e.g. Summer headline experiment"
                className={inputClass}
              />
            </label>

            <label className="flex items-center gap-2 text-sm text-fg">
              <input
                type="checkbox"
                checked={makeDefault}
                onChange={(e) => setMakeDefault(e.target.checked)}
                className="h-4 w-4 rounded border-border text-accent focus:ring-accent"
              />
              Make this the default version (rules following &ldquo;default&rdquo;
              switch to it)
            </label>

            {error && (
              <p role="alert" className="text-xs font-medium text-danger">
                {error}
              </p>
            )}

            <div className="mt-2 flex justify-end gap-3">
              <Dialog.Close asChild>
                <button
                  type="button"
                  className="rounded-md border border-border px-4 py-2 text-sm font-medium text-fg transition hover:bg-bg-overlay"
                >
                  Cancel
                </button>
              </Dialog.Close>
              <button
                type="submit"
                disabled={mutation.isPending}
                className="inline-flex items-center rounded-md bg-accent px-4 py-2 text-sm font-medium text-accent-fg shadow-sm transition hover:opacity-90 disabled:cursor-not-allowed disabled:opacity-60"
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
