"use client";

// Client component: Radix Dialog with a controlled form (browser state +
// events). FRONTEND CONTRACT §2.2, Task 06.
//
// DEVIATION: the contract calls for React Hook Form + Zod. react-hook-form is
// NOT a declared dependency in package.json, so per the buildability rule
// ("only use npm deps declared in package.json") this uses controlled React
// state validated with the same Zod schema (FeatureCreate.safeParse). The
// validation behaviour (per-field errors, submit gating) is identical.
import { useState } from "react";
import * as Dialog from "@radix-ui/react-dialog";
import { useMutation, useQueryClient } from "@tanstack/react-query";

import { ApiError } from "@/lib/api/client";
import { FeatureType } from "@/lib/api/enums";
import { createFeature, FeatureCreate } from "@/lib/api/features";
import { toUserError } from "@/lib/errors/userError";
import { useOnboardingStore } from "@/state/onboardingStore";

type FieldErrors = Partial<Record<"id" | "name" | "type" | "form", string>>;

const FEATURE_TYPES = FeatureType.options;

const inputClass =
  "w-full rounded-md border border-status-prevBg px-3 py-2 text-sm text-nav shadow-sm focus:border-brand-500 focus:outline-none focus:ring-1 focus:ring-brand-500";
const labelClass = "text-sm font-medium text-nav";
const errorClass = "text-xs font-medium text-danger";

export function FeatureCreateModal() {
  const [open, setOpen] = useState(false);
  const [id, setId] = useState("");
  const [name, setName] = useState("");
  const [type, setType] = useState<FeatureType>("html");
  const [errors, setErrors] = useState<FieldErrors>({});

  const queryClient = useQueryClient();
  const completeOnboarding = useOnboardingStore((s) => s.complete);

  const mutation = useMutation({
    mutationFn: createFeature,
    onSuccess: () => {
      void queryClient.invalidateQueries({ queryKey: ["features"] });
      completeOnboarding("feature");
      resetAndClose();
    },
    onError: (err: unknown) => {
      if (err instanceof ApiError && err.code === "SLUG_CONFLICT") {
        const ue = toUserError(err, { surface: "create" });
        setErrors({ id: `${ue.title} — ${ue.howToFix}` });
        return;
      }
      const ue = toUserError(err, { surface: "create" });
      setErrors({ form: `${ue.title}. ${ue.howToFix}` });
    },
  });

  function resetAndClose() {
    setId("");
    setName("");
    setType("html");
    setErrors({});
    setOpen(false);
  }

  function handleSubmit(event: React.FormEvent<HTMLFormElement>) {
    event.preventDefault();
    const parsed = FeatureCreate.safeParse({ id, name, type });
    if (!parsed.success) {
      const next: FieldErrors = {};
      for (const issue of parsed.error.issues) {
        const field = issue.path[0];
        if (field === "id" || field === "name" || field === "type") {
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
          + Add A New Feature
        </button>
      </Dialog.Trigger>

      <Dialog.Portal>
        <Dialog.Overlay className="fixed inset-0 z-40 bg-black/60 backdrop-blur-sm" />
        <Dialog.Content className="fixed left-1/2 top-1/2 z-50 w-full max-w-md -translate-x-1/2 -translate-y-1/2 rounded-lg bg-bg-elevated p-6 shadow-xl focus:outline-none">
          <Dialog.Title className="text-lg font-semibold text-nav">
            Add A New Feature
          </Dialog.Title>
          <Dialog.Description className="mt-1 text-sm text-status-prevFg">
            Create a feature to author response transformation rules.
          </Dialog.Description>

          <form className="mt-5 flex flex-col gap-4" onSubmit={handleSubmit}>
            <div className="flex flex-col gap-1">
              <label htmlFor="feature-id" className={labelClass}>
                Slug
              </label>
              <input
                id="feature-id"
                name="id"
                value={id}
                onChange={(e) => setId(e.target.value)}
                placeholder="dn-article"
                aria-invalid={errors.id ? true : undefined}
                className={inputClass}
              />
              {errors.id && <p className={errorClass}>{errors.id}</p>}
            </div>

            <div className="flex flex-col gap-1">
              <label htmlFor="feature-name" className={labelClass}>
                Name
              </label>
              <input
                id="feature-name"
                name="name"
                value={name}
                onChange={(e) => setName(e.target.value)}
                placeholder="Article Paywall"
                aria-invalid={errors.name ? true : undefined}
                className={inputClass}
              />
              {errors.name && <p className={errorClass}>{errors.name}</p>}
            </div>

            <div className="flex flex-col gap-1">
              <label htmlFor="feature-type" className={labelClass}>
                Type
              </label>
              <select
                id="feature-type"
                name="type"
                value={type}
                onChange={(e) => setType(e.target.value as FeatureType)}
                className={inputClass}
              >
                {FEATURE_TYPES.map((t) => (
                  <option key={t} value={t}>
                    {t.toUpperCase()}
                  </option>
                ))}
              </select>
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
                {mutation.isPending ? "Creating…" : "Create Feature"}
              </button>
            </div>
          </form>
        </Dialog.Content>
      </Dialog.Portal>
    </Dialog.Root>
  );
}
