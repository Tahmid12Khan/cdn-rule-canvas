"use client";

// JsonMutationForm (spec §4 / JSON mutation forms). Controlled form for a JSON
// outcome component config: json_remove / json_set / json_replace. Validation
// uses the zod schemas directly (safeParse) — same pattern as the HTML forms
// (react-hook-form is not declared in package.json).
//
//   - json_remove: target_path only.
//   - json_set / json_replace: target_path + a JSON value editor (textarea,
//     parsed to a value on submit; a parse error blocks submission inline).
//
// `target_path` is a SIMPLE path (dot + [index], e.g. $.user.premium) — NOT a
// filter expression; the proxy walks the parsed path.
import { useState, type FormEvent } from "react";
import clsx from "clsx";

import {
  JsonRemoveConfig,
  JsonReplaceConfig,
  JsonSetConfig,
  type JsonRemoveConfig as JsonRemoveConfigT,
  type JsonReplaceConfig as JsonReplaceConfigT,
  type JsonSetConfig as JsonSetConfigT,
} from "@/lib/schemas/components";

export type JsonMutationConfig =
  | JsonRemoveConfigT
  | JsonSetConfigT
  | JsonReplaceConfigT;

interface JsonMutationFormProps {
  formId: string;
  type: "json_remove" | "json_set" | "json_replace";
  initial: JsonMutationConfig;
  onValidSubmit: (config: JsonMutationConfig) => void;
  onDirtyChange?: (dirty: boolean) => void;
}

// Pretty-print the seeded value for editing; empty for a fresh null.
function seedValue(initial: JsonMutationConfig): string {
  if (initial.type === "json_remove") return "";
  if (initial.value === null || initial.value === undefined) return "";
  return JSON.stringify(initial.value, null, 2);
}

export function JsonMutationForm({
  formId,
  type,
  initial,
  onValidSubmit,
  onDirtyChange,
}: JsonMutationFormProps) {
  const [targetPath, setTargetPath] = useState(
    "target_path" in initial ? initial.target_path : "",
  );
  const [valueText, setValueText] = useState(() => seedValue(initial));
  const [pathError, setPathError] = useState<string | null>(null);
  const [valueError, setValueError] = useState<string | null>(null);

  const needsValue = type === "json_set" || type === "json_replace";

  function markDirty() {
    onDirtyChange?.(true);
  }

  function handleSubmit(e: FormEvent<HTMLFormElement>) {
    e.preventDefault();

    if (type === "json_remove") {
      const parsed = JsonRemoveConfig.safeParse({
        type: "json_remove",
        target_path: targetPath,
      });
      if (!parsed.success) {
        setPathError(parsed.error.issues[0]?.message ?? "Invalid path");
        return;
      }
      setPathError(null);
      onValidSubmit(parsed.data);
      return;
    }

    // json_set / json_replace: parse the value editor (empty = null).
    let value: unknown = null;
    if (valueText.trim()) {
      try {
        value = JSON.parse(valueText);
      } catch {
        setValueError("That isn't valid JSON. Fix the value and try again.");
        return;
      }
    }
    setValueError(null);

    const schema = type === "json_set" ? JsonSetConfig : JsonReplaceConfig;
    const parsed = schema.safeParse({ type, target_path: targetPath, value });
    if (!parsed.success) {
      const pathIssue = parsed.error.issues.find(
        (i) => i.path[0] === "target_path",
      );
      setPathError(pathIssue?.message ?? "Invalid path");
      return;
    }
    setPathError(null);
    onValidSubmit(parsed.data);
  }

  return (
    <form id={formId} onSubmit={handleSubmit} className="space-y-4" noValidate>
      <div>
        <label
          htmlFor="jm-target-path"
          className="mb-1 block text-sm font-medium text-fg"
        >
          Target path
        </label>
        <input
          id="jm-target-path"
          type="text"
          value={targetPath}
          onChange={(e) => {
            setTargetPath(e.target.value);
            markDirty();
          }}
          placeholder="e.g. $.user.premium"
          aria-invalid={Boolean(pathError) || undefined}
          aria-describedby={pathError ? "jm-target-path-error" : undefined}
          className={clsx(
            "w-full rounded-md border px-3 py-2 font-mono text-sm focus:outline-none",
            pathError
              ? "border-danger focus:border-danger"
              : "border-border focus:border-brand-500",
          )}
        />
        {pathError && (
          <p
            id="jm-target-path-error"
            role="alert"
            className="mt-1 text-xs text-danger"
          >
            {pathError}
          </p>
        )}
      </div>

      {needsValue && (
        <div>
          <label
            htmlFor="jm-value"
            className="mb-1 block text-sm font-medium text-fg"
          >
            Value (JSON)
          </label>
          <textarea
            id="jm-value"
            value={valueText}
            onChange={(e) => {
              setValueText(e.target.value);
              markDirty();
            }}
            rows={5}
            placeholder={'"premium"  or  { "tier": "gold" }  or  42'}
            aria-invalid={Boolean(valueError) || undefined}
            aria-describedby={valueError ? "jm-value-error" : undefined}
            className={clsx(
              "w-full rounded-md border px-3 py-2 font-mono text-sm focus:outline-none",
              valueError
                ? "border-danger focus:border-danger"
                : "border-border focus:border-brand-500",
            )}
          />
          <p className="mt-1 text-xs text-fg-muted">
            Leave empty for <code className="font-mono">null</code>. Strings must
            be quoted, e.g. <code className="font-mono">&quot;premium&quot;</code>
            .
          </p>
          {valueError && (
            <p id="jm-value-error" role="alert" className="mt-1 text-xs text-danger">
              {valueError}
            </p>
          )}
        </div>
      )}
    </form>
  );
}
