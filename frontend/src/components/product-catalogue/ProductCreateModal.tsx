"use client";

// Client component: Radix Dialog with a controlled create form for a Product.
// Mirrors FeatureCreateModal's pattern — controlled React state validated with
// the same Zod schema (ProductCreate), since react-hook-form is not a declared
// dependency.
import { useState } from "react";
import * as Dialog from "@radix-ui/react-dialog";
import { useMutation, useQueryClient } from "@tanstack/react-query";

import { ApiError } from "@/lib/api/client";
import { createProduct, ProductCreate } from "@/lib/api/products";
import { toUserError } from "@/lib/errors/userError";

type FieldErrors = Partial<
  Record<"label" | "name" | "description" | "form", string>
>;

const inputClass =
  "w-full rounded-md border border-status-prevBg px-3 py-2 text-sm text-nav shadow-sm focus:border-brand-500 focus:outline-none focus:ring-1 focus:ring-brand-500";
const labelClass = "text-sm font-medium text-nav";
const errorClass = "text-xs font-medium text-danger";

export function ProductCreateModal() {
  const [open, setOpen] = useState(false);
  const [label, setLabel] = useState("");
  const [name, setName] = useState("");
  const [description, setDescription] = useState("");
  const [errors, setErrors] = useState<FieldErrors>({});

  const queryClient = useQueryClient();

  const mutation = useMutation({
    mutationFn: createProduct,
    onSuccess: () => {
      void queryClient.invalidateQueries({ queryKey: ["products"] });
      resetAndClose();
    },
    onError: (err: unknown) => {
      // The backend returns 409 SLUG_CONFLICT for a duplicate product label OR
      // name (see product_service::map_conflict_error) — not just the label.
      // The generic version-centric SLUG_CONFLICT copy ("kebab-case") doesn't
      // fit a snake_case product label, so use product-aware copy instead,
      // anchored to the Label field since it's the most common collision
      // (mirrors SiteFormModal's site-aware 409 handling).
      if (err instanceof ApiError && err.code === "SLUG_CONFLICT") {
        setErrors({
          label: "A product with this label or name already exists",
        });
        return;
      }
      const ue = toUserError(err, { surface: "create" });
      setErrors({ form: `${ue.title}. ${ue.howToFix}` });
    },
  });

  function resetAndClose() {
    setLabel("");
    setName("");
    setDescription("");
    setErrors({});
    setOpen(false);
  }

  function handleSubmit(event: React.FormEvent<HTMLFormElement>) {
    event.preventDefault();
    const parsed = ProductCreate.safeParse({
      label,
      name,
      description: description.trim() === "" ? undefined : description,
    });
    if (!parsed.success) {
      const next: FieldErrors = {};
      for (const issue of parsed.error.issues) {
        const field = issue.path[0];
        if (field === "label" || field === "name" || field === "description") {
          next[field] = next[field] ?? issue.message;
        }
      }
      setErrors(next);
      return;
    }
    setErrors({});
    mutation.mutate(parsed.data);
  }

  return (
    <Dialog.Root
      open={open}
      onOpenChange={(next) => (next ? setOpen(true) : resetAndClose())}
    >
      <Dialog.Trigger asChild>
        <button
          type="button"
          className="inline-flex items-center rounded-md bg-action-600 px-4 py-2 text-sm font-medium text-white shadow-sm transition hover:bg-action-700"
        >
          + New Product
        </button>
      </Dialog.Trigger>

      <Dialog.Portal>
        <Dialog.Overlay className="fixed inset-0 z-40 bg-black/60 backdrop-blur-sm" />
        <Dialog.Content className="fixed left-1/2 top-1/2 z-50 w-full max-w-md -translate-x-1/2 -translate-y-1/2 rounded-lg bg-bg-elevated p-6 shadow-xl focus:outline-none">
          <Dialog.Title className="text-lg font-semibold text-nav">
            New Product
          </Dialog.Title>
          <Dialog.Description className="mt-1 text-sm text-status-prevFg">
            Products are matched by the `has_product` decision node.
          </Dialog.Description>

          <form className="mt-5 flex flex-col gap-4" onSubmit={handleSubmit}>
            <div className="flex flex-col gap-1">
              <label htmlFor="product-label" className={labelClass}>
                Label
              </label>
              <input
                id="product-label"
                name="label"
                value={label}
                onChange={(e) => setLabel(e.target.value)}
                placeholder="premium_tier"
                aria-invalid={errors.label ? true : undefined}
                className={inputClass}
              />
              {errors.label && <p className={errorClass}>{errors.label}</p>}
            </div>

            <div className="flex flex-col gap-1">
              <label htmlFor="product-name" className={labelClass}>
                Name
              </label>
              <input
                id="product-name"
                name="name"
                value={name}
                onChange={(e) => setName(e.target.value)}
                placeholder="Premium Tier"
                aria-invalid={errors.name ? true : undefined}
                className={inputClass}
              />
              {errors.name && <p className={errorClass}>{errors.name}</p>}
            </div>

            <div className="flex flex-col gap-1">
              <label htmlFor="product-description" className={labelClass}>
                Description
              </label>
              <textarea
                id="product-description"
                name="description"
                value={description}
                onChange={(e) => setDescription(e.target.value)}
                placeholder="Optional description"
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
                  className="rounded-md border border-status-prevBg px-4 py-2 text-sm font-medium text-nav transition hover:bg-status-prevBg/30"
                >
                  Cancel
                </button>
              </Dialog.Close>
              <button
                type="submit"
                disabled={mutation.isPending}
                className="inline-flex items-center rounded-md bg-action-600 px-4 py-2 text-sm font-medium text-white shadow-sm transition hover:bg-action-700 disabled:cursor-not-allowed disabled:opacity-60"
              >
                {mutation.isPending ? "Creating…" : "Create Product"}
              </button>
            </div>
          </form>
        </Dialog.Content>
      </Dialog.Portal>
    </Dialog.Root>
  );
}
