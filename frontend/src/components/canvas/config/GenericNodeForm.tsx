"use client";

// Generic, manifest-driven processor config form (spec Part E). Renders one
// control per spec field (select / text / number) in fields[] order, validating
// `required` / `required_unless` / select-option membership per the LOCKED
// rules (lib/canvas/manifest.ts). Replaces the per-type MetaTags/DeviceType/
// ArticleUrl forms. onChange reports the current draft + validity upward so the
// drawer can drive the Save button + dirty state.
import { useEffect, useState } from "react";

import { SiteSelectControl } from "@/components/canvas/config/SiteSelectControl";
import type { NodeFieldSpec, NodeTypeSpec } from "@/lib/api/nodeTypes";
import {
  isFieldRequired,
  validateProcessor,
} from "@/lib/canvas/manifest";
import type { ProcessorConfig } from "@/lib/canvas/types";

// One option for a dynamic `outcome_select` control (expression-nodes-spec §2).
export interface OutcomeSelectOption {
  id: string;
  title: string;
}

interface GenericNodeFormProps {
  spec: NodeTypeSpec;
  initial: ProcessorConfig;
  onChange: (draft: ProcessorConfig, valid: boolean) => void;
  // View-only mode: controls render disabled/readOnly so the node's contents are
  // inspectable without being editable.
  disabled?: boolean;
  // Options for any `outcome_select` field (the version's outcomes). Not in the
  // manifest — supplied by the caller.
  outcomes?: OutcomeSelectOption[];
}

// Render a single field's value as a controlled string for the input/select.
function asInputValue(value: unknown): string {
  if (value === undefined || value === null) return "";
  return String(value);
}

export function GenericNodeForm({
  spec,
  initial,
  onChange,
  disabled = false,
  outcomes = [],
}: GenericNodeFormProps) {
  // The draft keeps the open processor map (type + fields). Seeded from initial.
  const [draft, setDraft] = useState<ProcessorConfig>(initial);

  const errors = validateProcessor(spec, draft);
  const valid = Object.keys(errors).length === 0;

  useEffect(() => {
    onChange(draft, valid);
    // Report on every field change.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [draft]);

  function setField(field: NodeFieldSpec, raw: string) {
    setDraft((prev) => {
      const next: ProcessorConfig = { ...prev };
      if (field.control === "number") {
        next[field.name] = raw === "" ? "" : Number(raw);
      } else {
        next[field.name] = raw;
      }
      return next;
    });
  }

  return (
    <div className="space-y-4" data-testid="generic-node-form">
      {spec.fields.map((field) => {
        const required = isFieldRequired(field, draft);
        const error = errors[field.name];
        const inputId = `field-${field.name}`;
        return (
          <div key={field.name}>
            <label
              htmlFor={inputId}
              className="mb-1 block text-sm font-medium text-nav"
            >
              {field.label}
              {required && <span className="ml-0.5 text-danger">*</span>}
            </label>

            {field.control === "site_select" ? (
              <SiteSelectControl
                id={inputId}
                value={asInputValue(draft[field.name])}
                onChange={(slug) => setField(field, slug)}
                disabled={disabled}
                placeholder={field.placeholder}
              />
            ) : field.control === "outcome_select" ? (
              <select
                id={inputId}
                value={asInputValue(draft[field.name])}
                onChange={(e) => setField(field, e.target.value)}
                disabled={disabled}
                className="w-full rounded-md border border-status-prevBg px-3 py-2 text-sm focus:border-brand-500 disabled:cursor-not-allowed disabled:opacity-60"
              >
                <option value="">
                  {outcomes.length === 0 ? "No outcomes yet" : "Select an outcome…"}
                </option>
                {outcomes.map((o) => (
                  <option key={o.id} value={o.id}>
                    {o.title}
                  </option>
                ))}
              </select>
            ) : field.control === "select" ? (
              <select
                id={inputId}
                value={asInputValue(draft[field.name])}
                onChange={(e) => setField(field, e.target.value)}
                disabled={disabled}
                className="w-full rounded-md border border-status-prevBg px-3 py-2 text-sm focus:border-brand-500 disabled:cursor-not-allowed disabled:opacity-60"
              >
                {/* Empty placeholder so a freshly-dropped node (no default) is
                    intentionally incomplete and fails `required` until edited. */}
                {asInputValue(draft[field.name]) === "" && (
                  <option value="">Select…</option>
                )}
                {(field.options ?? []).map((opt) => (
                  <option key={String(opt.value)} value={String(opt.value)}>
                    {opt.label}
                  </option>
                ))}
              </select>
            ) : (
              <input
                id={inputId}
                type={field.control === "number" ? "number" : "text"}
                value={asInputValue(draft[field.name])}
                onChange={(e) => setField(field, e.target.value)}
                readOnly={disabled}
                disabled={disabled}
                placeholder={field.placeholder}
                className="w-full rounded-md border border-status-prevBg px-3 py-2 text-sm focus:border-brand-500 disabled:cursor-not-allowed disabled:opacity-60"
              />
            )}

            {error && <p className="mt-1 text-xs text-danger">{error}</p>}
          </div>
        );
      })}
    </div>
  );
}
