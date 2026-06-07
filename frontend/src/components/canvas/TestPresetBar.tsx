"use client";

// TestPresetBar. A row shown at the top of each Test-panel body that lets the
// user load a saved test input, save the current inputs as a new preset, or
// update/delete the currently-loaded preset. Presets are a GLOBAL library
// (CONTRACTS.md §3 migration 0011 / §7) keyed by `kind` (rule | url); the
// `payload` is opaque and owned by the panel.
//
// Saving opens the shared TestPresetFormModal in "quick" mode (Name + Slug
// only) so a non-technical user never edits raw JSON — the panel's current
// inputs are handed in as the payload + kind.
//
// Browser state + network mutations → client component.
import { useEffect, useMemo, useState } from "react";
import * as Dialog from "@radix-ui/react-dialog";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";

import { TestPresetFormModal } from "@/components/test-presets/TestPresetFormModal";
import {
  deleteTestPreset,
  listTestPresets,
  updateTestPreset,
  type TestPresetKind,
  type TestPresetRead,
} from "@/lib/api/test-presets";

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

      <TestPresetFormModal
        mode="quick"
        presetKind={kind}
        presetPayload={currentPayload as Record<string, unknown>}
        onSaved={(slug) => setLoadedSlug(slug)}
        trigger={
          <button
            type="button"
            disabled={!payloadValid}
            className="rounded-md bg-action-600 px-3 py-1 text-xs font-medium text-white transition hover:bg-action-700 disabled:cursor-not-allowed disabled:opacity-50"
          >
            Save
          </button>
        }
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
