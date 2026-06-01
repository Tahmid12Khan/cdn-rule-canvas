"use client";

// OutcomeDetailsForm (FRONTEND CONTRACT §2.5, Task 15).
//
// Controlled title/description form for the outcome being edited. Validation
// uses the zod `OutcomeUpdate` constraints directly (title required, ≤100;
// description ≤500) rather than react-hook-form, which is not a declared
// dependency. The form is controlled by the parent (lifts changes up + reports
// validity) so the footer can disable Save when the title is invalid.
import { useId } from "react";
import clsx from "clsx";

const TITLE_MAX = 100;
const DESCRIPTION_MAX = 500;

export interface OutcomeDetailsValue {
  title: string;
  description: string;
}

interface OutcomeDetailsFormProps {
  value: OutcomeDetailsValue;
  onChange: (value: OutcomeDetailsValue) => void;
  disabled?: boolean;
}

export function validateOutcomeDetails(
  value: OutcomeDetailsValue,
): Partial<Record<keyof OutcomeDetailsValue, string>> {
  const errors: Partial<Record<keyof OutcomeDetailsValue, string>> = {};
  const title = value.title.trim();
  if (title.length === 0) {
    errors.title = "Give this outcome a title so it's easy to find";
  } else if (title.length > TITLE_MAX) {
    errors.title = `Title is too long — keep it under ${TITLE_MAX} characters`;
  }
  if (value.description.length > DESCRIPTION_MAX) {
    errors.description = `Description is too long — trim it to under ${DESCRIPTION_MAX} characters`;
  }
  return errors;
}

export function OutcomeDetailsForm({
  value,
  onChange,
  disabled = false,
}: OutcomeDetailsFormProps) {
  const titleId = useId();
  const descId = useId();
  const errors = validateOutcomeDetails(value);

  return (
    <div className="space-y-5">
      <div>
        <label
          htmlFor={titleId}
          className="mb-1 block text-sm font-medium text-nav"
        >
          Title <span className="text-status-stagingFg">*</span>
        </label>
        <input
          id={titleId}
          type="text"
          value={value.title}
          disabled={disabled}
          maxLength={TITLE_MAX}
          onChange={(e) => onChange({ ...value, title: e.target.value })}
          aria-invalid={Boolean(errors.title) || undefined}
          aria-describedby={errors.title ? `${titleId}-error` : undefined}
          placeholder="e.g. DN Regwall 1.0"
          className={clsx(
            "w-full rounded-md border px-3 py-2 text-sm focus:outline-none disabled:bg-status-prevBg/40",
            errors.title
              ? "border-danger focus:border-danger"
              : "border-status-prevBg focus:border-brand-500",
          )}
        />
        {errors.title && (
          <p
            id={`${titleId}-error`}
            role="alert"
            className="mt-1 text-xs text-danger"
          >
            {errors.title}
          </p>
        )}
      </div>

      <div>
        <label
          htmlFor={descId}
          className="mb-1 block text-sm font-medium text-nav"
        >
          Description
        </label>
        <textarea
          id={descId}
          rows={3}
          value={value.description}
          disabled={disabled}
          maxLength={DESCRIPTION_MAX}
          onChange={(e) => onChange({ ...value, description: e.target.value })}
          aria-invalid={Boolean(errors.description) || undefined}
          aria-describedby={errors.description ? `${descId}-error` : undefined}
          placeholder="Optional description of this outcome"
          className={clsx(
            "w-full resize-y rounded-md border px-3 py-2 text-sm focus:outline-none disabled:bg-status-prevBg/40",
            errors.description
              ? "border-danger focus:border-danger"
              : "border-status-prevBg focus:border-brand-500",
          )}
        />
        <div className="mt-1 flex items-center justify-between">
          {errors.description ? (
            <p
              id={`${descId}-error`}
              role="alert"
              className="text-xs text-danger"
            >
              {errors.description}
            </p>
          ) : (
            <span />
          )}
          <span className="text-xs text-status-prev">
            {value.description.length}/{DESCRIPTION_MAX}
          </span>
        </div>
      </div>
    </div>
  );
}
