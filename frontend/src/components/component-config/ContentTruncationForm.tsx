"use client";

// ContentTruncationForm (FRONTEND CONTRACT §2.5, Task 16).
//
// Controlled form for a `content_truncation` component config. Validation uses
// the zod `ContentTruncationConfig` schema (safeParse) rather than React Hook
// Form (not declared in package.json). Owns its own draft state, surfaces
// per-field errors on submit, and lifts a validated config to the parent modal.
import { useState, type FormEvent } from "react";
import clsx from "clsx";

import { ContentTruncationConfig } from "@/lib/schemas/components";

interface ContentTruncationFormProps {
  formId: string;
  initial: ContentTruncationConfig;
  onValidSubmit: (config: ContentTruncationConfig) => void;
  onDirtyChange?: (dirty: boolean) => void;
}

type FieldErrors = Partial<
  Record<"target_selector" | "word_count", string>
>;

export function ContentTruncationForm({
  formId,
  initial,
  onValidSubmit,
  onDirtyChange,
}: ContentTruncationFormProps) {
  const [targetSelector, setTargetSelector] = useState(initial.target_selector);
  // Kept as a string so the user can clear the field; coerced on submit.
  const [wordCount, setWordCount] = useState(String(initial.word_count));
  const [fadeOut, setFadeOut] = useState(initial.fade_out);
  const [errors, setErrors] = useState<FieldErrors>({});

  function markDirty() {
    onDirtyChange?.(true);
  }

  function handleSubmit(e: FormEvent<HTMLFormElement>) {
    e.preventDefault();
    const numeric = wordCount.trim() === "" ? Number.NaN : Number(wordCount);
    const candidate = {
      type: "content_truncation" as const,
      target_selector: targetSelector,
      word_count: numeric,
      fade_out: fadeOut,
    };
    const parsed = ContentTruncationConfig.safeParse(candidate);
    if (!parsed.success) {
      const next: FieldErrors = {};
      for (const issue of parsed.error.issues) {
        const key = issue.path[0];
        if (key === "target_selector" || key === "word_count") {
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
          htmlFor="ct-target-selector"
          className="mb-1 block text-sm font-medium text-fg"
        >
          Target selector
        </label>
        <input
          id="ct-target-selector"
          type="text"
          value={targetSelector}
          onChange={(e) => {
            setTargetSelector(e.target.value);
            markDirty();
          }}
          placeholder="e.g. .article-body"
          aria-invalid={Boolean(errors.target_selector) || undefined}
          aria-describedby={
            errors.target_selector ? "ct-target-selector-error" : undefined
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
            id="ct-target-selector-error"
            role="alert"
            className="mt-1 text-xs text-danger"
          >
            {errors.target_selector}
          </p>
        )}
      </div>

      <div>
        <label
          htmlFor="ct-word-count"
          className="mb-1 block text-sm font-medium text-fg"
        >
          Word count
        </label>
        <input
          id="ct-word-count"
          type="number"
          inputMode="numeric"
          min={1}
          max={10000}
          value={wordCount}
          onChange={(e) => {
            setWordCount(e.target.value);
            markDirty();
          }}
          aria-invalid={Boolean(errors.word_count) || undefined}
          aria-describedby={
            errors.word_count ? "ct-word-count-error" : undefined
          }
          className={clsx(
            "w-full rounded-md border px-3 py-2 text-sm focus:outline-none",
            errors.word_count
              ? "border-danger focus:border-danger"
              : "border-border focus:border-brand-500",
          )}
        />
        {errors.word_count && (
          <p
            id="ct-word-count-error"
            role="alert"
            className="mt-1 text-xs text-danger"
          >
            {errors.word_count}
          </p>
        )}
      </div>

      <label className="flex items-center gap-2 text-sm text-fg">
        <input
          type="checkbox"
          checked={fadeOut}
          onChange={(e) => {
            setFadeOut(e.target.checked);
            markDirty();
          }}
          className="h-4 w-4 rounded border-border text-brand-600 focus:ring-brand-500"
        />
        Fade out truncated content
      </label>
    </form>
  );
}
