"use client";

// TestPresetBar. A row shown at the top of each Test-panel body that lets the
// user load a saved test input, save the current inputs as a new preset, or
// update/delete the currently-loaded preset. Presets are a GLOBAL library
// (CONTRACTS.md §3 migration 0011 / §7) keyed by `kind` (rule | url); the
// `payload` is opaque and owned by the panel.
//
// Browser state + network mutations → client component.
import { useEffect, useMemo, useState } from "react";
import * as Dialog from "@radix-ui/react-dialog";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";

import { ApiError } from "@/lib/api/client";
import { SLUG_RE } from "@/lib/api/features";
import {
  createTestPreset,
  deleteTestPreset,
  listTestPresets,
  updateTestPreset,
  type TestPresetKind,
  type TestPresetRead,
} from "@/lib/api/test-presets";
import { toUserError } from "@/lib/errors/userError";

interface TestPresetBarProps {
  kind: TestPresetKind;
  // The panel's current inputs, serialized into the preset payload shape.
  currentPayload: unknown;
  // Whether the current inputs are valid enough to save (e.g. headers valid).
  payloadValid: boolean;
  // Apply a loaded preset's payload back into the panel state.
  onLoad: (payload: unknown) => void;
}

const PAGE_SIZE = 100;

const selectClass =
  "rounded-md border border-status-prevBg px-2 py-1.5 text-sm text-nav focus:border-brand-500 focus:outline-none";
const inputClass =
  "w-full rounded-md border border-status-prevBg px-3 py-2 text-sm text-nav shadow-sm focus:border-brand-500 focus:outline-none focus:ring-1 focus:ring-brand-500 disabled:cursor-not-allowed disabled:opacity-60";
const labelClass = "text-sm font-medium text-nav";
const errorClass = "text-xs font-medium text-danger";

export function TestPresetBar({
  kind,
  currentPayload,
  payloadValid,
  onLoad,
}: TestPresetBarProps) {
  const queryClient = useQueryClient();
  // The slug of the currently-loaded preset ("" when none is loaded).
  const [loadedSlug, setLoadedSlug] = useState("");

  const { data } = useQuery({
    queryKey: ["test-presets", { page: 1, page_size: PAGE_SIZE, kind }],
    queryFn: () => listTestPresets({ page: 1, page_size: PAGE_SIZE, kind }),
  });

  const presets: TestPresetRead[] = useMemo(() => data?.items ?? [], [data]);
  const loaded = presets.find((p) => p.slug === loadedSlug) ?? null;

  const updateMutation = useMutation({
    mutationFn: () => {
      if (!loaded) throw new Error("No preset loaded");
      return updateTestPreset(loaded.slug, {
        payload: currentPayload as Record<string, unknown>,
      });
    },
    onSuccess: () => {
      void queryClient.invalidateQueries({ queryKey: ["test-presets"] });
    },
  });

  // If the loaded preset is deleted out from under us, drop the selection.
  useEffect(() => {
    if (loadedSlug && !presets.some((p) => p.slug === loadedSlug)) {
      setLoadedSlug("");
    }
  }, [loadedSlug, presets]);

  function handleSelect(slug: string) {
    setLoadedSlug(slug);
    if (slug === "") return;
    const preset = presets.find((p) => p.slug === slug);
    if (preset) onLoad(preset.payload);
  }

  return (
    <div className="flex flex-wrap items-center gap-2 rounded-md border border-status-prevBg bg-bg-elevated px-3 py-2">
      <label className="flex items-center gap-2 text-xs font-medium text-nav">
        Saved tests
        <select
          aria-label="Saved tests"
          value={loadedSlug}
          onChange={(e) => handleSelect(e.target.value)}
          className={selectClass}
        >
          <option value="">— none —</option>
          {presets.map((p) => (
            <option key={p.slug} value={p.slug}>
              {p.name}
            </option>
          ))}
        </select>
      </label>

      <SavePresetDialog
        kind={kind}
        currentPayload={currentPayload}
        payloadValid={payloadValid}
        onSaved={(slug) => setLoadedSlug(slug)}
      />

      {loaded && (
        <>
          <button
            type="button"
            onClick={() => updateMutation.mutate()}
            disabled={!payloadValid || updateMutation.isPending}
            className="rounded-md border border-status-prevBg px-3 py-1 text-xs font-medium text-nav transition hover:bg-status-prevBg/30 disabled:cursor-not-allowed disabled:opacity-50"
          >
            {updateMutation.isPending ? "Updating…" : "Update"}
          </button>
          <DeletePresetButton
            slug={loaded.slug}
            name={loaded.name}
            onDeleted={() => setLoadedSlug("")}
          />
        </>
      )}
    </div>
  );
}

interface SavePresetDialogProps {
  kind: TestPresetKind;
  currentPayload: unknown;
  payloadValid: boolean;
  onSaved: (slug: string) => void;
}

function SavePresetDialog({
  kind,
  currentPayload,
  payloadValid,
  onSaved,
}: SavePresetDialogProps) {
  const queryClient = useQueryClient();
  const [open, setOpen] = useState(false);
  const [name, setName] = useState("");
  const [slug, setSlug] = useState("");
  const [errors, setErrors] = useState<{
    name?: string;
    slug?: string;
    form?: string;
  }>({});

  useEffect(() => {
    if (open) {
      setName("");
      setSlug("");
      setErrors({});
    }
  }, [open]);

  const mutation = useMutation({
    mutationFn: () =>
      createTestPreset({
        slug,
        name,
        kind,
        payload: currentPayload as Record<string, unknown>,
      }),
    onSuccess: () => {
      void queryClient.invalidateQueries({ queryKey: ["test-presets"] });
      onSaved(slug);
      setOpen(false);
    },
    onError: (err: unknown) => {
      if (err instanceof ApiError && err.status === 409) {
        setErrors({ slug: "That slug or name is already in use" });
        return;
      }
      const ue = toUserError(err, { surface: "create" });
      setErrors({ form: `${ue.title}. ${ue.howToFix}` });
    },
  });

  function handleSubmit(event: React.FormEvent<HTMLFormElement>) {
    event.preventDefault();
    const next: { name?: string; slug?: string } = {};
    if (name.trim().length < 1) next.name = "Name is required";
    if (slug.length < 3 || slug.length > 64 || !SLUG_RE.test(slug)) {
      next.slug = "Use lowercase kebab-case (3–64 chars, e.g. my-test)";
    }
    if (next.name || next.slug) {
      setErrors(next);
      return;
    }
    setErrors({});
    mutation.mutate();
  }

  return (
    <Dialog.Root open={open} onOpenChange={setOpen}>
      <Dialog.Trigger asChild>
        <button
          type="button"
          disabled={!payloadValid}
          className="rounded-md bg-action-600 px-3 py-1 text-xs font-medium text-white transition hover:bg-action-700 disabled:cursor-not-allowed disabled:opacity-50"
        >
          Save
        </button>
      </Dialog.Trigger>
      <Dialog.Portal>
        <Dialog.Overlay className="fixed inset-0 z-40 bg-black/60 backdrop-blur-sm" />
        <Dialog.Content className="fixed left-1/2 top-1/2 z-50 w-full max-w-md -translate-x-1/2 -translate-y-1/2 rounded-lg bg-bg-elevated p-6 shadow-xl focus:outline-none">
          <Dialog.Title className="text-lg font-semibold text-nav">
            Save test preset
          </Dialog.Title>
          <Dialog.Description className="mt-1 text-sm text-status-prevFg">
            Save the current {kind === "url" ? "live-URL" : "rule"} test inputs
            to reuse later.
          </Dialog.Description>

          <form className="mt-5 flex flex-col gap-4" onSubmit={handleSubmit}>
            <div className="flex flex-col gap-1">
              <label htmlFor="preset-name" className={labelClass}>
                Name
              </label>
              <input
                id="preset-name"
                value={name}
                onChange={(e) => setName(e.target.value)}
                placeholder="Mobile paywall path"
                aria-invalid={errors.name ? true : undefined}
                className={inputClass}
              />
              {errors.name && <p className={errorClass}>{errors.name}</p>}
            </div>

            <div className="flex flex-col gap-1">
              <label htmlFor="preset-slug" className={labelClass}>
                Slug
              </label>
              <input
                id="preset-slug"
                value={slug}
                onChange={(e) => setSlug(e.target.value)}
                placeholder="mobile-paywall-path"
                aria-invalid={errors.slug ? true : undefined}
                className={inputClass}
              />
              {errors.slug && <p className={errorClass}>{errors.slug}</p>}
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
                {mutation.isPending ? "Saving…" : "Save preset"}
              </button>
            </div>
          </form>
        </Dialog.Content>
      </Dialog.Portal>
    </Dialog.Root>
  );
}

interface DeletePresetButtonProps {
  slug: string;
  name: string;
  onDeleted: () => void;
}

function DeletePresetButton({ slug, name, onDeleted }: DeletePresetButtonProps) {
  const queryClient = useQueryClient();
  const [open, setOpen] = useState(false);

  const mutation = useMutation({
    mutationFn: () => deleteTestPreset(slug),
    onSuccess: () => {
      void queryClient.invalidateQueries({ queryKey: ["test-presets"] });
      onDeleted();
      setOpen(false);
    },
  });

  return (
    <Dialog.Root open={open} onOpenChange={setOpen}>
      <Dialog.Trigger asChild>
        <button
          type="button"
          className="rounded-md border border-status-prevBg px-3 py-1 text-xs font-medium text-danger transition hover:bg-danger/10"
        >
          Delete
        </button>
      </Dialog.Trigger>
      <Dialog.Portal>
        <Dialog.Overlay className="fixed inset-0 z-40 bg-black/60 backdrop-blur-sm" />
        <Dialog.Content className="fixed left-1/2 top-1/2 z-50 w-[90vw] max-w-md -translate-x-1/2 -translate-y-1/2 rounded-xl bg-bg-elevated p-6 shadow-xl focus:outline-none">
          <Dialog.Title className="text-lg font-semibold text-nav">
            Delete preset {name}?
          </Dialog.Title>
          <Dialog.Description className="mt-1 text-sm text-status-prevFg">
            This removes the saved test inputs. This action cannot be undone.
          </Dialog.Description>
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
              onClick={() => mutation.mutate()}
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
