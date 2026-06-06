"use client";

// Client component: Radix Dialog hosting a controlled create/edit test-preset
// form. Mirrors SiteFormModal — controlled React state validated with the Zod
// schema (TestPresetCreate / TestPresetUpdate). One component serves both modes:
// when `preset` is provided it edits (PATCH; slug + kind locked); otherwise it
// creates (POST). The `payload` is edited as a JSON textarea validated as a
// parseable JSON object before submit.
import { useEffect, useState } from "react";
import * as Dialog from "@radix-ui/react-dialog";
import { useMutation, useQueryClient } from "@tanstack/react-query";

import { ApiError } from "@/lib/api/client";
import {
  createTestPreset,
  TestPresetCreate,
  TestPresetKind,
  type TestPresetRead,
  updateTestPreset,
} from "@/lib/api/test-presets";
import { toUserError } from "@/lib/errors/userError";

type FieldKey = "slug" | "name" | "kind" | "payload";
type FieldErrors = Partial<Record<FieldKey | "form", string>>;

const KINDS = TestPresetKind.options;

const inputClass =
  "w-full rounded-md border border-status-prevBg px-3 py-2 text-sm text-nav shadow-sm focus:border-brand-500 focus:outline-none focus:ring-1 focus:ring-brand-500 disabled:cursor-not-allowed disabled:opacity-60";
const labelClass = "text-sm font-medium text-nav";
const errorClass = "text-xs font-medium text-danger";

interface FormState {
  slug: string;
  name: string;
  kind: TestPresetKind;
  payloadText: string;
}

function emptyState(): FormState {
  return { slug: "", name: "", kind: "rule", payloadText: "{}" };
}

function stateFromPreset(preset: TestPresetRead): FormState {
  return {
    slug: preset.slug,
    name: preset.name,
    kind: preset.kind,
    payloadText: JSON.stringify(preset.payload, null, 2),
  };
}

// Parse the payload textarea, requiring a JSON object (not array / scalar).
function parsePayload(
  text: string,
): { payload: Record<string, unknown> } | { error: string } {
  const trimmed = text.trim();
  if (trimmed === "") return { payload: {} };
  let value: unknown;
  try {
    value = JSON.parse(trimmed);
  } catch {
    return { error: "Payload must be valid JSON" };
  }
  if (value === null || typeof value !== "object" || Array.isArray(value)) {
    return { error: "Payload must be a JSON object (e.g. { … })" };
  }
  return { payload: value as Record<string, unknown> };
}

interface TestPresetFormModalProps {
  // Edit target; omit/undefined to create a new preset.
  preset?: TestPresetRead;
  // Trigger element (create / empty-state usage). Omit when driving `open`
  // externally (the list opens the edit modal from a card action).
  trigger?: React.ReactNode;
  open?: boolean;
  onOpenChange?: (open: boolean) => void;
}

export function TestPresetFormModal({
  preset,
  trigger,
  open: openProp,
  onOpenChange,
}: TestPresetFormModalProps) {
  const isEdit = preset !== undefined;
  const [internalOpen, setInternalOpen] = useState(false);
  const isControlled = openProp !== undefined;
  const open = isControlled ? openProp : internalOpen;
  const setOpen = (next: boolean) => {
    if (!isControlled) setInternalOpen(next);
    onOpenChange?.(next);
  };
  const [form, setForm] = useState<FormState>(() =>
    preset ? stateFromPreset(preset) : emptyState(),
  );
  const [errors, setErrors] = useState<FieldErrors>({});

  const queryClient = useQueryClient();

  useEffect(() => {
    if (open) {
      setForm(preset ? stateFromPreset(preset) : emptyState());
      setErrors({});
    }
  }, [open, preset]);

  const mutation = useMutation({
    mutationFn: (body: { name: string; payload: Record<string, unknown> }) =>
      isEdit
        ? updateTestPreset(preset.slug, body)
        : createTestPreset({
            slug: form.slug,
            name: body.name,
            kind: form.kind,
            payload: body.payload,
          }),
    onSuccess: () => {
      void queryClient.invalidateQueries({ queryKey: ["test-presets"] });
      setOpen(false);
    },
    onError: (err: unknown) => {
      if (err instanceof ApiError && err.status === 409) {
        setErrors({
          form: "A preset with this slug or name already exists.",
          slug: "This slug may already be in use",
        });
        return;
      }
      const ue = toUserError(err, { surface: isEdit ? "save" : "create" });
      setErrors({ form: `${ue.title}. ${ue.howToFix}` });
    },
  });

  function setField<K extends keyof FormState>(key: K, value: FormState[K]) {
    setForm((prev) => ({ ...prev, [key]: value }));
  }

  function handleSubmit(event: React.FormEvent<HTMLFormElement>) {
    event.preventDefault();
    const parsedPayload = parsePayload(form.payloadText);
    if ("error" in parsedPayload) {
      setErrors({ payload: parsedPayload.error });
      return;
    }
    if (!isEdit) {
      const candidate = {
        slug: form.slug,
        name: form.name,
        kind: form.kind,
        payload: parsedPayload.payload,
      };
      const parsed = TestPresetCreate.safeParse(candidate);
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
    } else if (form.name.trim().length < 1) {
      setErrors({ name: "Name is required" });
      return;
    }
    setErrors({});
    mutation.mutate({ name: form.name, payload: parsedPayload.payload });
  }

  return (
    <Dialog.Root open={open} onOpenChange={setOpen}>
      {trigger && <Dialog.Trigger asChild>{trigger}</Dialog.Trigger>}

      <Dialog.Portal>
        <Dialog.Overlay className="fixed inset-0 z-40 bg-black/60 backdrop-blur-sm" />
        <Dialog.Content className="fixed left-1/2 top-1/2 z-50 max-h-[90vh] w-full max-w-md -translate-x-1/2 -translate-y-1/2 overflow-y-auto rounded-lg bg-bg-elevated p-6 shadow-xl focus:outline-none">
          <Dialog.Title className="text-lg font-semibold text-nav">
            {isEdit ? "Edit Test Preset" : "Add A Test Preset"}
          </Dialog.Title>
          <Dialog.Description className="mt-1 text-sm text-status-prevFg">
            Reusable inputs for the rule-builder Test panels.
          </Dialog.Description>

          <form className="mt-5 flex flex-col gap-4" onSubmit={handleSubmit}>
            <div className="flex flex-col gap-1">
              <label htmlFor="preset-slug" className={labelClass}>
                Slug
              </label>
              <input
                id="preset-slug"
                name="slug"
                value={form.slug}
                onChange={(e) => setField("slug", e.target.value)}
                placeholder="mobile-paywall-path"
                disabled={isEdit}
                aria-invalid={errors.slug ? true : undefined}
                className={inputClass}
              />
              {errors.slug && <p className={errorClass}>{errors.slug}</p>}
            </div>

            <div className="flex flex-col gap-1">
              <label htmlFor="preset-name" className={labelClass}>
                Name
              </label>
              <input
                id="preset-name"
                name="name"
                value={form.name}
                onChange={(e) => setField("name", e.target.value)}
                placeholder="Mobile paywall path"
                aria-invalid={errors.name ? true : undefined}
                className={inputClass}
              />
              {errors.name && <p className={errorClass}>{errors.name}</p>}
            </div>

            <div className="flex flex-col gap-1">
              <label htmlFor="preset-kind" className={labelClass}>
                Kind
              </label>
              <select
                id="preset-kind"
                name="kind"
                value={form.kind}
                onChange={(e) =>
                  setField("kind", e.target.value as TestPresetKind)
                }
                disabled={isEdit}
                aria-invalid={errors.kind ? true : undefined}
                className={inputClass}
              >
                {KINDS.map((k) => (
                  <option key={k} value={k}>
                    {k}
                  </option>
                ))}
              </select>
              {errors.kind && <p className={errorClass}>{errors.kind}</p>}
            </div>

            <div className="flex flex-col gap-1">
              <label htmlFor="preset-payload" className={labelClass}>
                Payload (JSON)
              </label>
              <textarea
                id="preset-payload"
                name="payload"
                value={form.payloadText}
                onChange={(e) => setField("payloadText", e.target.value)}
                rows={8}
                aria-invalid={errors.payload ? true : undefined}
                className={`${inputClass} font-mono text-xs`}
              />
              {errors.payload && (
                <p className={errorClass}>{errors.payload}</p>
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
                {mutation.isPending
                  ? isEdit
                    ? "Saving…"
                    : "Creating…"
                  : isEdit
                    ? "Save Preset"
                    : "Create Preset"}
              </button>
            </div>
          </form>
        </Dialog.Content>
      </Dialog.Portal>
    </Dialog.Root>
  );
}
