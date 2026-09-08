"use client";

// HtmlRemoveForm (FRONTEND CONTRACT §2.5).
//
// Controlled form for an `html_remove` component config. Validation uses the
// zod `HtmlRemoveConfig` schema (safeParse) rather than React Hook Form (not
// declared in package.json). Owns its own draft state, surfaces per-field
// errors on submit, and lifts a validated config to the parent modal.
//
// There is no html_body and no placement mode: this component only deletes.
import { useState, type FormEvent } from "react";
import clsx from "clsx";

import { HtmlRemoveConfig } from "@/lib/schemas/components";

interface HtmlRemoveFormProps {
  formId: string;
  initial: HtmlRemoveConfig;
  onValidSubmit: (config: HtmlRemoveConfig) => void;
  onDirtyChange?: (dirty: boolean) => void;
}

type FieldErrors = Partial<Record<"target_selector", string>>;

export function HtmlRemoveForm({
  formId,
  initial,
  onValidSubmit,
  onDirtyChange,
}: HtmlRemoveFormProps) {
  const [targetSelector, setTargetSelector] = useState(initial.target_selector);
  const [includeSelector, setIncludeSelector] = useState(
    initial.include_selector,
  );
  const [errors, setErrors] = useState<FieldErrors>({});

  function markDirty() {
    onDirtyChange?.(true);
  }

  function handleSubmit(e: FormEvent<HTMLFormElement>) {
    e.preventDefault();
    const parsed = HtmlRemoveConfig.safeParse({
      type: "html_remove" as const,
      target_selector: targetSelector,
      include_selector: includeSelector,
    });
    if (!parsed.success) {
      const next: FieldErrors = {};
      for (const issue of parsed.error.issues) {
        if (issue.path[0] === "target_selector") {
          next.target_selector = issue.message;
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
          htmlFor="hr-target-selector"
          className="mb-1 block text-sm font-medium text-fg"
        >
          Target selector
        </label>
        <input
          id="hr-target-selector"
          type="text"
          value={targetSelector}
          onChange={(e) => {
            setTargetSelector(e.target.value);
            markDirty();
          }}
          placeholder="e.g. #dn-content-ssr"
          aria-invalid={Boolean(errors.target_selector) || undefined}
          aria-describedby={
            errors.target_selector ? "hr-target-selector-error" : undefined
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
            id="hr-target-selector-error"
            role="alert"
            className="mt-1 text-xs text-danger"
          >
            {errors.target_selector}
          </p>
        )}
      </div>

      <label className="flex items-start gap-2 text-sm text-fg">
        <input
          type="checkbox"
          checked={includeSelector}
          onChange={(e) => {
            setIncludeSelector(e.target.checked);
            markDirty();
          }}
          className="mt-0.5 h-4 w-4 rounded border-border text-brand-600 focus:ring-brand-500"
        />
        <span>
          Remove the matched element too
          <span className="mt-0.5 block text-xs text-fg-muted">
            {includeSelector
              ? "The element and its contents are deleted."
              : "Only the contents are deleted — the element itself stays (e.g. <div id=“dn-content-ssr”></div>)."}
          </span>
        </span>
      </label>
    </form>
  );
}
