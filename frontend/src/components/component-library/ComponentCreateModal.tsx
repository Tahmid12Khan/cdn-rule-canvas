"use client";

// Create-component modal (design §5.1). A minimal create form: slug + name +
// optional description. The HTML body and variables are authored afterwards in
// the editor, so this dialog only seeds the component + its v1. On success it
// navigates to the new component's editor. Mirrors TestPresetFormModal /
// SiteFormModal.
import { useState } from "react";
import * as Dialog from "@radix-ui/react-dialog";
import { useRouter } from "next/navigation";

import { ApiError } from "@/lib/api/client";
import { useCreateComponentTemplate } from "@/lib/api/componentTemplates";
import { ComponentTemplateCreate } from "@/lib/schemas/componentTemplates";
import { toUserError } from "@/lib/errors/userError";

type FieldKey = "slug" | "name" | "description";
type FieldErrors = Partial<Record<FieldKey | "form", string>>;

const inputClass =
  "w-full rounded-md border border-border bg-bg px-3 py-2 text-sm text-fg shadow-sm focus:border-accent focus:outline-none focus:ring-1 focus:ring-accent";
const labelClass = "text-sm font-medium text-fg";
const helpClass = "text-xs text-fg-muted";
const errorClass = "text-xs font-medium text-danger";

interface ComponentCreateModalProps {
  trigger?: React.ReactNode;
  open?: boolean;
  onOpenChange?: (open: boolean) => void;
}

export function ComponentCreateModal({
  trigger,
  open: openProp,
  onOpenChange,
}: ComponentCreateModalProps) {
  const router = useRouter();
  const [internalOpen, setInternalOpen] = useState(false);
  const isControlled = openProp !== undefined;
  const open = isControlled ? openProp : internalOpen;
  const setOpen = (next: boolean) => {
    if (!isControlled) setInternalOpen(next);
    onOpenChange?.(next);
  };

  const [slug, setSlug] = useState("");
  const [name, setName] = useState("");
  const [description, setDescription] = useState("");
  const [errors, setErrors] = useState<FieldErrors>({});

  const mutation = useCreateComponentTemplate();

  function handleSubmit(event: React.FormEvent<HTMLFormElement>) {
    event.preventDefault();
    const candidate = {
      slug,
      name,
      description: description.trim() || undefined,
    };
    const parsed = ComponentTemplateCreate.safeParse(candidate);
    if (!parsed.success) {
      const next: FieldErrors = {};
      for (const issue of parsed.error.issues) {
        const field = issue.path[0];
        if (typeof field === "string") {
          next[field as FieldKey] = next[field as FieldKey] ?? issue.message;
        }
      }
      setErrors(next);
      return;
    }
    setErrors({});
    mutation.mutate(parsed.data, {
      onSuccess: (created) => {
        setOpen(false);
        setSlug("");
        setName("");
        setDescription("");
        router.push(`/products/components/${created.slug}`);
      },
      onError: (err: unknown) => {
        if (err instanceof ApiError && err.status === 409) {
          setErrors({
            form: "A component with this slug already exists.",
            slug: "This slug may already be in use",
          });
          return;
        }
        const ue = toUserError(err, { surface: "create" });
        setErrors({ form: `${ue.title}. ${ue.howToFix}` });
      },
    });
  }

  return (
    <Dialog.Root open={open} onOpenChange={setOpen}>
      {trigger && <Dialog.Trigger asChild>{trigger}</Dialog.Trigger>}
      <Dialog.Portal>
        <Dialog.Overlay className="fixed inset-0 z-40 bg-black/60 backdrop-blur-sm" />
        <Dialog.Content className="fixed left-1/2 top-1/2 z-50 max-h-[90vh] w-full max-w-md -translate-x-1/2 -translate-y-1/2 overflow-y-auto rounded-lg bg-bg-elevated p-6 shadow-xl focus:outline-none">
          <Dialog.Title className="text-lg font-semibold text-fg">
            Create a Component
          </Dialog.Title>
          <Dialog.Description className="mt-1 text-sm text-fg-muted">
            A reusable HTML template. You&apos;ll author the markup and variables
            next.
          </Dialog.Description>

          <form className="mt-5 flex flex-col gap-4" onSubmit={handleSubmit}>
            <div className="flex flex-col gap-1">
              <label htmlFor="component-slug" className={labelClass}>
                Slug
              </label>
              <input
                id="component-slug"
                name="slug"
                value={slug}
                onChange={(e) => setSlug(e.target.value)}
                placeholder="paywall-cta"
                aria-invalid={errors.slug ? true : undefined}
                aria-describedby="component-slug-help"
                className={inputClass}
              />
              <p id="component-slug-help" className={helpClass}>
                Lowercase kebab-case. Can&apos;t change later.
              </p>
              {errors.slug && <p className={errorClass}>{errors.slug}</p>}
            </div>

            <div className="flex flex-col gap-1">
              <label htmlFor="component-name" className={labelClass}>
                Name
              </label>
              <input
                id="component-name"
                name="name"
                value={name}
                onChange={(e) => setName(e.target.value)}
                placeholder="Paywall CTA"
                aria-invalid={errors.name ? true : undefined}
                className={inputClass}
              />
              {errors.name && <p className={errorClass}>{errors.name}</p>}
            </div>

            <div className="flex flex-col gap-1">
              <label htmlFor="component-description" className={labelClass}>
                Description (optional)
              </label>
              <textarea
                id="component-description"
                name="description"
                value={description}
                onChange={(e) => setDescription(e.target.value)}
                rows={3}
                aria-invalid={errors.description ? true : undefined}
                className={inputClass}
              />
              {errors.description && (
                <p className={errorClass}>{errors.description}</p>
              )}
            </div>

            {errors.form && (
              <p role="alert" className={errorClass}>
                {errors.form}
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
                {mutation.isPending ? "Creating…" : "Create Component"}
              </button>
            </div>
          </form>
        </Dialog.Content>
      </Dialog.Portal>
    </Dialog.Root>
  );
}
