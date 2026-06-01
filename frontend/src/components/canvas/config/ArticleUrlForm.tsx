"use client";

// Article URL processor config form. Controlled inputs with live zod validation
// (processorSchemas.ts). The value input is required for every operator (it is a
// regex pattern when operator === "matches"). onChange reports validity + the
// current (possibly invalid) draft so the drawer can drive the Save button +
// dirty state.
import { useEffect, useState } from "react";

import {
  articleUrlSchema,
  type ArticleUrlFormValues,
} from "@/lib/canvas/processorSchemas";
import type { ArticleUrlOperator, ProcessorConfig } from "@/lib/canvas/types";

interface ArticleUrlFormProps {
  initial: Extract<ProcessorConfig, { type: "article_url" }>;
  onChange: (draft: ProcessorConfig, valid: boolean) => void;
  // View-only mode: inputs/selects render disabled/readOnly so the node's
  // contents are inspectable without being editable.
  disabled?: boolean;
}

const OPERATORS: ArticleUrlOperator[] = [
  "contains",
  "matches",
  "starts_with",
  "equals",
];

export function ArticleUrlForm({
  initial,
  onChange,
  disabled = false,
}: ArticleUrlFormProps) {
  const [operator, setOperator] = useState<ArticleUrlOperator>(
    initial.operator,
  );
  const [value, setValue] = useState(initial.value);

  const draft: ArticleUrlFormValues = {
    type: "article_url",
    operator,
    value,
  };
  const result = articleUrlSchema.safeParse(draft);
  const errors = result.success
    ? {}
    : Object.fromEntries(
        result.error.issues.map((i) => [String(i.path[0] ?? "_"), i.message]),
      );

  useEffect(() => {
    onChange(draft, result.success);
    // Report on every field change.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [operator, value]);

  return (
    <div className="space-y-4">
      <div>
        <label
          htmlFor="au-operator"
          className="mb-1 block text-sm font-medium text-nav"
        >
          Operator
        </label>
        <select
          id="au-operator"
          value={operator}
          onChange={(e) => setOperator(e.target.value as ArticleUrlOperator)}
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

      <div>
        <label
          htmlFor="au-value"
          className="mb-1 block text-sm font-medium text-nav"
        >
          {operator === "matches" ? "Pattern" : "Value"}
        </label>
        <input
          id="au-value"
          type="text"
          value={value}
          onChange={(e) => setValue(e.target.value)}
          readOnly={disabled}
          disabled={disabled}
          placeholder={operator === "matches" ? "e.g. \\.html$" : "e.g. /article"}
          className="w-full rounded-md border border-status-prevBg px-3 py-2 text-sm focus:border-brand-500 disabled:cursor-not-allowed disabled:opacity-60"
        />
        {errors.value && (
          <p className="mt-1 text-xs text-danger">{errors.value}</p>
        )}
      </div>
    </div>
  );
}
