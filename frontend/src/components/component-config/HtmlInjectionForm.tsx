"use client";

// HtmlInjectionForm (FRONTEND CONTRACT §2.5, Task 16).
//
// Controlled form for an `html_injection` component config. Validation uses the
// zod `HtmlInjectionConfig` schema directly (safeParse) rather than React Hook
// Form, because react-hook-form / @hookform/resolvers are not declared in
// package.json. The form owns its own draft state, surfaces per-field errors on
// submit, and lifts a fully-validated `HtmlInjectionConfig` to the parent via
// `onValidSubmit`. The parent modal owns the actual mutation + slug/placement.
import { useState, type FormEvent } from "react";
import clsx from "clsx";

import { RichTextEditor } from "@/components/component-config/RichTextEditor";
import {
  HtmlInjectionConfig,
  HtmlPlacementMode,
} from "@/lib/schemas/components";

type PlacementModeValue = HtmlInjectionConfig["placement_mode"];

const PLACEMENT_MODE_OPTIONS: { value: PlacementModeValue; label: string }[] = [
  { value: "replace", label: "Replace" },
  { value: "append", label: "Append" },
  { value: "prepend", label: "Prepend" },
  { value: "before", label: "Before" },
  { value: "after", label: "After" },
];

interface HtmlInjectionFormProps {
  formId: string;
  initial: HtmlInjectionConfig;
  onValidSubmit: (config: HtmlInjectionConfig) => void;
  onDirtyChange?: (dirty: boolean) => void;
}

type FieldErrors = Partial<
  Record<"target_selector" | "placement_mode" | "html_body", string>
>;

export function HtmlInjectionForm({
  formId,
  initial,
  onValidSubmit,
  onDirtyChange,
}: HtmlInjectionFormProps) {
  const [targetSelector, setTargetSelector] = useState(initial.target_selector);
  const [placementMode, setPlacementMode] = useState<PlacementModeValue>(
    initial.placement_mode,
  );
  const [htmlBody, setHtmlBody] = useState(initial.html_body);
  const [errors, setErrors] = useState<FieldErrors>({});

  function markDirty() {
    onDirtyChange?.(true);
  }

  function handleSubmit(e: FormEvent<HTMLFormElement>) {
    e.preventDefault();
    const candidate = {
      type: "html_injection" as const,
      target_selector: targetSelector,
      placement_mode: placementMode,
      html_body: htmlBody,
      theme: initial.theme ?? null,
    };
    const parsed = HtmlInjectionConfig.safeParse(candidate);
    if (!parsed.success) {
      const next: FieldErrors = {};
      for (const issue of parsed.error.issues) {
        const key = issue.path[0];
        if (
          key === "target_selector" ||
          key === "placement_mode" ||
          key === "html_body"
        ) {
          next[key] = issue.message;
        }
      }
      setErrors(next);
      return;
    }
    setErrors({});
    onValidSubmit(parsed.data);
  }

  return (
    <form id={formId} onSubmit={handleSubmit} className="space-y-4" noValidate>
      <div>
        <label
          htmlFor="hi-target-selector"
          className="mb-1 block text-sm font-medium text-fg"
        >
          Target selector
        </label>
        <input
          id="hi-target-selector"
          type="text"
          value={targetSelector}
          onChange={(e) => {
            setTargetSelector(e.target.value);
            markDirty();
          }}
          placeholder="e.g. .article-body"
          aria-invalid={Boolean(errors.target_selector) || undefined}
          aria-describedby={
            errors.target_selector ? "hi-target-selector-error" : undefined
          }
          className={clsx(
            "w-full rounded-md border px-3 py-2 text-sm focus:outline-none",
            errors.target_selector
              ? "border-danger focus:border-danger"
              : "border-border focus:border-brand-500",
          )}
        />
        {errors.target_selector && (
          <p
            id="hi-target-selector-error"
            role="alert"
            className="mt-1 text-xs text-danger"
          >
            {errors.target_selector}
          </p>
        )}
      </div>

      <div>
        <label
          htmlFor="hi-placement-mode"
          className="mb-1 block text-sm font-medium text-fg"
        >
          Placement mode
        </label>
        <select
          id="hi-placement-mode"
          value={placementMode}
          onChange={(e) => {
            const next = HtmlPlacementMode.parse(e.target.value);
            setPlacementMode(next);
            markDirty();
          }}
          className="w-full rounded-md border border-border px-3 py-2 text-sm focus:border-brand-500 focus:outline-none"
        >
          {PLACEMENT_MODE_OPTIONS.map((o) => (
            <option key={o.value} value={o.value}>
              {o.label}
            </option>
          ))}
        </select>
      </div>

      <div>
        <label
          htmlFor="hi-html-body"
          className="mb-1 block text-sm font-medium text-fg"
        >
          HTML content
        </label>
        <RichTextEditor
          id="hi-html-body"
          value={htmlBody}
          onChange={(next) => {
            setHtmlBody(next);
            markDirty();
          }}
          invalid={Boolean(errors.html_body)}
          describedBy={errors.html_body ? "hi-html-body-error" : undefined}
        />
        {errors.html_body && (
          <p
            id="hi-html-body-error"
            role="alert"
            className="mt-1 text-xs text-danger"
          >
            {errors.html_body}
          </p>
        )}
      </div>
    </form>
  );
}
