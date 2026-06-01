"use client";

// Meta Tags processor config form (Task 13). Controlled inputs with live zod
// validation (processorSchemas.ts). Value input is hidden when operator ===
// "exists". onChange reports validity + the current (possibly invalid) draft so
// the drawer can drive the Save button + dirty state.
import { useEffect, useState } from "react";

import {
  metaTagsSchema,
  type MetaTagsFormValues,
} from "@/lib/canvas/processorSchemas";
import type { MetaTagsOperator, ProcessorConfig } from "@/lib/canvas/types";

interface MetaTagsFormProps {
  initial: Extract<ProcessorConfig, { type: "meta_tags" }>;
  onChange: (draft: ProcessorConfig, valid: boolean) => void;
  // View-only mode (Task D): inputs/selects render disabled/readOnly so the
  // node's contents are inspectable without being editable.
  disabled?: boolean;
}

const OPERATORS: MetaTagsOperator[] = ["contains", "equals", "exists"];

export function MetaTagsForm({
  initial,
  onChange,
  disabled = false,
}: MetaTagsFormProps) {
  const [tagName, setTagName] = useState(initial.tag_name);
  const [operator, setOperator] = useState<MetaTagsOperator>(initial.operator);
  const [value, setValue] = useState(initial.value ?? "");

  const draft: MetaTagsFormValues = {
    type: "meta_tags",
    tag_name: tagName,
    operator,
    value: operator === "exists" ? null : value,
  };
  const result = metaTagsSchema.safeParse(draft);
  const errors = result.success
    ? {}
    : Object.fromEntries(
        result.error.issues.map((i) => [String(i.path[0] ?? "_"), i.message]),
      );

  useEffect(() => {
    onChange(draft, result.success);
    // Report on every field change.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [tagName, operator, value]);

  return (
    <div className="space-y-4">
      <div>
        <label
          htmlFor="mt-tag-name"
          className="mb-1 block text-sm font-medium text-nav"
        >
          Tag name
        </label>
        <input
          id="mt-tag-name"
          type="text"
          value={tagName}
          onChange={(e) => setTagName(e.target.value)}
          readOnly={disabled}
          disabled={disabled}
          placeholder="e.g. paywall"
          className="w-full rounded-md border border-status-prevBg px-3 py-2 text-sm focus:border-brand-500 disabled:cursor-not-allowed disabled:opacity-60"
        />
        {errors.tag_name && (
          <p className="mt-1 text-xs text-danger">{errors.tag_name}</p>
        )}
      </div>

      <div>
        <label
          htmlFor="mt-operator"
          className="mb-1 block text-sm font-medium text-nav"
        >
          Operator
        </label>
        <select
          id="mt-operator"
          value={operator}
          onChange={(e) => setOperator(e.target.value as MetaTagsOperator)}
          disabled={disabled}
          className="w-full rounded-md border border-status-prevBg px-3 py-2 text-sm focus:border-brand-500 disabled:cursor-not-allowed disabled:opacity-60"
        >
          {OPERATORS.map((op) => (
            <option key={op} value={op}>
              {op}
            </option>
          ))}
        </select>
      </div>

      {operator !== "exists" && (
        <div>
          <label
            htmlFor="mt-value"
            className="mb-1 block text-sm font-medium text-nav"
          >
            Value
          </label>
          <input
            id="mt-value"
            type="text"
            value={value}
            onChange={(e) => setValue(e.target.value)}
            readOnly={disabled}
            disabled={disabled}
            placeholder="e.g. true"
            className="w-full rounded-md border border-status-prevBg px-3 py-2 text-sm focus:border-brand-500 disabled:cursor-not-allowed disabled:opacity-60"
          />
          {errors.value && (
            <p className="mt-1 text-xs text-danger">{errors.value}</p>
          )}
        </div>
      )}
    </div>
  );
}
