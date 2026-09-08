"use client";

// ComponentConfigModal (FRONTEND CONTRACT §2.5, Task 16).
//
// Radix Dialog hosting the component configuration editor. Supports two flows:
//   - "create": user picks a component type (tab switch between HTML Injection /
//     Content Truncation / HTML Remove / Component), each tab swapping in its
//     own validated form.
//   - "edit": the type is fixed to the existing component's type; the tab strip
//     is shown but locked to that type.
//
// The modal owns the cross-cutting fields (slug + placement) and composes the
// per-type config form. On submit it assembles a full `ComponentCreate` /
// `ComponentUpdate` payload and lifts it via `onSubmit`. The parent
// (OutcomeEditorClient) owns the actual mutation + query invalidation.
//
// react-hook-form is not declared in package.json, so validation is done with
// the zod schemas directly inside the child forms + here for slug/placement.
import { useMemo, useState } from "react";
import * as Dialog from "@radix-ui/react-dialog";
import clsx from "clsx";

import {
  ComponentRefForm,
  type ComponentRefAnyConfig,
} from "@/components/component-config/ComponentRefForm";
import { ContentTruncationForm } from "@/components/component-config/ContentTruncationForm";
import { HtmlInjectionForm } from "@/components/component-config/HtmlInjectionForm";
import { HtmlRemoveForm } from "@/components/component-config/HtmlRemoveForm";
import {
  JsonMutationForm,
  type JsonMutationConfig,
} from "@/components/component-config/JsonMutationForm";
import { Placement } from "@/lib/api/enums";
import {
  ComponentConfig,
  ComponentCreate,
  ComponentType,
  defaultConfigFor,
} from "@/lib/schemas/components";

const PLACEMENT_OPTIONS: { value: Placement; label: string }[] = [
  { value: "inline", label: "Inline" },
  { value: "sticky_footer", label: "Sticky Footer" },
  { value: "popup", label: "Pop-Up" },
];

// Tab set per feature content kind (spec §4): HTML features mutate the HTML
// page; JSON features mutate the JSON body.
const HTML_TYPE_TABS: { value: ComponentType; label: string }[] = [
  { value: "html_injection", label: "HTML Injection" },
  { value: "content_truncation", label: "Content Truncation" },
  { value: "html_remove", label: "HTML Remove" },
  { value: "component_ref", label: "Component" },
];
const JSON_TYPE_TABS: { value: ComponentType; label: string }[] = [
  { value: "json_remove", label: "JSON Remove" },
  { value: "json_set", label: "JSON Set" },
  { value: "json_replace", label: "JSON Replace" },
  { value: "component_ref_json", label: "Component" },
];

const FORM_ID = "component-config-form";

export interface ComponentConfigSubmit {
  slug: string;
  type: ComponentType;
  config: ComponentConfig;
  placement: Placement;
}

interface ComponentConfigModalProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  // Defaults to "create". In "edit" mode the type tab is locked.
  mode?: "create" | "edit";
  initialSlug?: string;
  initialType?: ComponentType;
  initialPlacement?: Placement;
  initialConfig?: ComponentConfig;
  // Feature content kind — picks the tab set (HTML vs JSON component types).
  featureType?: "html" | "json";
  onSubmit: (payload: ComponentConfigSubmit) => void;
  submitting?: boolean;
}

export function ComponentConfigModal({
  open,
  onOpenChange,
  mode = "create",
  initialSlug = "",
  initialType,
  initialPlacement = "inline",
  initialConfig,
  featureType = "html",
  onSubmit,
  submitting = false,
}: ComponentConfigModalProps) {
  const typeTabs = featureType === "json" ? JSON_TYPE_TABS : HTML_TYPE_TABS;
  const [type, setType] = useState<ComponentType>(
    initialType ?? typeTabs[0].value,
  );
  const [slug, setSlug] = useState(initialSlug);
  const [placement, setPlacement] = useState<Placement>(initialPlacement);
  const [slugError, setSlugError] = useState<string | null>(null);

  // Per-type initial configs. Each form is seeded from `initialConfig` only when
  // its discriminant matches that form's type; otherwise sensible defaults.
  const htmlInitial = useMemo(
    () =>
      initialConfig?.type === "html_injection"
        ? initialConfig
        : defaultConfigFor("html_injection"),
    [initialConfig],
  );
  const truncationInitial = useMemo(
    () =>
      initialConfig?.type === "content_truncation"
        ? initialConfig
        : defaultConfigFor("content_truncation"),
    [initialConfig],
  );
  const removeInitial = useMemo(
    () =>
      initialConfig?.type === "html_remove"
        ? initialConfig
        : defaultConfigFor("html_remove"),
    [initialConfig],
  );
  // JSON mutation initial config: seed from initialConfig when its type matches
  // the currently-selected json_* tab; otherwise a default for that tab.
  const jsonInitial = useMemo<JsonMutationConfig>(() => {
    const jsonType =
      type === "json_remove" || type === "json_set" || type === "json_replace"
        ? type
        : "json_set";
    if (initialConfig && initialConfig.type === jsonType) {
      return initialConfig;
    }
    if (jsonType === "json_remove") return defaultConfigFor("json_remove");
    if (jsonType === "json_replace") return defaultConfigFor("json_replace");
    return defaultConfigFor("json_set");
  }, [initialConfig, type]);
  // Component-ref initial config: seed from initialConfig when its type matches
  // the active component_ref tab (HTML vs JSON); otherwise a default.
  const componentRefInitial = useMemo<ComponentRefAnyConfig>(() => {
    const refType =
      type === "component_ref_json" ? "component_ref_json" : "component_ref";
    if (initialConfig && initialConfig.type === refType) {
      return initialConfig;
    }
    return refType === "component_ref_json"
      ? defaultConfigFor("component_ref_json")
      : defaultConfigFor("component_ref");
  }, [initialConfig, type]);

  function handleValidConfig(config: ComponentConfig) {
    const parsedSlug = ComponentCreate.shape.slug.safeParse(slug);
    if (!parsedSlug.success) {
      setSlugError(parsedSlug.error.issues[0]?.message ?? "Invalid slug");
      return;
    }
    setSlugError(null);
    onSubmit({ slug: parsedSlug.data, type, config, placement });
  }

  const title = mode === "edit" ? "Edit Component" : "Add A Component";

  return (
    <Dialog.Root open={open} onOpenChange={onOpenChange}>
      <Dialog.Portal>
        <Dialog.Overlay className="fixed inset-0 z-40 bg-black/60 backdrop-blur-sm data-[state=open]:animate-in" />
        <Dialog.Content
          className="fixed left-1/2 top-1/2 z-50 flex max-h-[90vh] w-[92vw] max-w-lg -translate-x-1/2 -translate-y-1/2 flex-col overflow-hidden rounded-xl bg-bg-elevated shadow-2xl"
          aria-describedby={undefined}
        >
          <div className="flex items-start justify-between border-b border-border px-6 py-4">
            <div>
              <Dialog.Title className="text-lg font-semibold text-nav">
                {title}
              </Dialog.Title>
              <p className="mt-0.5 text-sm text-fg-muted">
                Configure how this component transforms the page.
              </p>
            </div>
            <Dialog.Close
              aria-label="Close"
              className="rounded-md p-1 text-fg-subtle hover:bg-bg-overlay hover:text-fg"
            >
              <svg
                width="18"
                height="18"
                viewBox="0 0 24 24"
                fill="none"
                stroke="currentColor"
                strokeWidth="2"
                strokeLinecap="round"
                aria-hidden="true"
              >
                <path d="M18 6 6 18M6 6l12 12" />
              </svg>
            </Dialog.Close>
          </div>

          <div className="flex-1 overflow-y-auto px-6 py-5">
            {/* Type selector */}
            <div
              role="tablist"
              aria-label="Component type"
              className="mb-5 inline-flex rounded-lg border border-border bg-bg p-1"
            >
              {typeTabs.map((t) => {
                const selected = type === t.value;
                const locked = mode === "edit" && t.value !== type;
                return (
                  <button
                    key={t.value}
                    type="button"
                    role="tab"
                    aria-selected={selected}
                    disabled={locked}
                    onClick={() => {
                      if (mode === "edit") return;
                      setType(t.value);
                    }}
                    className={clsx(
                      "rounded-md px-3 py-1.5 text-sm font-medium transition-colors",
                      selected
                        ? "bg-bg-elevated text-brand-700 shadow-sm"
                        : "text-fg-muted hover:text-fg",
                      locked && "cursor-not-allowed opacity-40",
                    )}
                  >
                    {t.label}
                  </button>
                );
              })}
            </div>

            {/* Slug + placement */}
            <div className="mb-5 grid grid-cols-1 gap-4 sm:grid-cols-2">
              <div>
                <label
                  htmlFor="cc-slug"
                  className="mb-1 block text-sm font-medium text-fg"
                >
                  Slug
                </label>
                <input
                  id="cc-slug"
                  type="text"
                  value={slug}
                  onChange={(e) => setSlug(e.target.value)}
                  placeholder="e.g. paywall-banner"
                  aria-invalid={Boolean(slugError) || undefined}
                  aria-describedby={slugError ? "cc-slug-error" : undefined}
                  className={clsx(
                    "w-full rounded-md border px-3 py-2 text-sm focus:outline-none",
                    slugError
                      ? "border-danger focus:border-danger"
                      : "border-border focus:border-brand-500",
                  )}
                />
                {slugError && (
                  <p
                    id="cc-slug-error"
                    role="alert"
                    className="mt-1 text-xs text-danger"
                  >
                    {slugError}
                  </p>
                )}
              </div>

              <div>
                <label
                  htmlFor="cc-placement"
                  className="mb-1 block text-sm font-medium text-fg"
                >
                  Placement
                </label>
                <select
                  id="cc-placement"
                  value={placement}
                  onChange={(e) =>
                    setPlacement(Placement.parse(e.target.value))
                  }
                  className="w-full rounded-md border border-border px-3 py-2 text-sm focus:border-brand-500 focus:outline-none"
                >
                  {PLACEMENT_OPTIONS.map((p) => (
                    <option key={p.value} value={p.value}>
                      {p.label}
                    </option>
                  ))}
                </select>
              </div>
            </div>

            {/* Per-type config form (the submit button targets these via form id) */}
            {type === "html_injection" ? (
              <HtmlInjectionForm
                key="html_injection"
                formId={FORM_ID}
                initial={htmlInitial}
                onValidSubmit={handleValidConfig}
              />
            ) : type === "content_truncation" ? (
              <ContentTruncationForm
                key="content_truncation"
                formId={FORM_ID}
                initial={truncationInitial}
                onValidSubmit={handleValidConfig}
              />
            ) : type === "html_remove" ? (
              <HtmlRemoveForm
                key="html_remove"
                formId={FORM_ID}
                initial={removeInitial}
                onValidSubmit={handleValidConfig}
              />
            ) : type === "component_ref" || type === "component_ref_json" ? (
              <ComponentRefForm
                key={type}
                formId={FORM_ID}
                type={type}
                initial={componentRefInitial}
                onValidSubmit={handleValidConfig}
              />
            ) : (
              <JsonMutationForm
                key={type}
                formId={FORM_ID}
                type={type}
                initial={jsonInitial}
                onValidSubmit={handleValidConfig}
              />
            )}
          </div>

          <div className="flex justify-end gap-3 border-t border-border px-6 py-4">
            <Dialog.Close asChild>
              <button
                type="button"
                className="rounded-lg border border-border px-4 py-2 text-sm font-medium text-fg hover:bg-bg-overlay"
              >
                Cancel
              </button>
            </Dialog.Close>
            <button
              type="submit"
              form={FORM_ID}
              disabled={submitting}
              className="rounded-lg bg-action px-4 py-2 text-sm font-semibold text-white hover:bg-action-700 disabled:cursor-not-allowed disabled:opacity-60"
            >
              {submitting ? "Saving…" : "Save Component"}
            </button>
          </div>
        </Dialog.Content>
      </Dialog.Portal>
    </Dialog.Root>
  );
}
