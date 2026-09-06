"use client";

// Shared "one input per declared variable" sub-form: renders a labelled text
// input for each ComponentVariable (title as label, description as helper
// text). Extracted from GenericNodeForm's Component Variables sub-form
// (component-editor design §5.4 item 6) so the Outcomes Library editor
// (SavedOutcomeEditorPage) can reuse the identical rendering.
import type { ComponentVariable } from "@/lib/schemas/componentTemplates";

interface VariablesFormProps {
  declaredVars: ComponentVariable[];
  values: Record<string, string>;
  onChange: (name: string, value: string) => void;
  disabled?: boolean;
}

export function VariablesForm({
  declaredVars,
  values,
  onChange,
  disabled = false,
}: VariablesFormProps) {
  if (declaredVars.length === 0) {
    return (
      <p className="text-xs text-status-prevFg">
        This component has no variables.
      </p>
    );
  }

  return (
    <>
      {declaredVars.map((v) => {
        const id = `var-${v.name}`;
        return (
          <div key={v.name}>
            <label
              htmlFor={id}
              className="mb-1 block text-sm font-medium text-nav"
            >
              {v.title}
            </label>
            <input
              id={id}
              type="text"
              value={values[v.name] ?? ""}
              onChange={(e) => onChange(v.name, e.target.value)}
              readOnly={disabled}
              disabled={disabled}
              placeholder={v.title}
              className="w-full rounded-md border border-status-prevBg px-3 py-2 text-sm focus:border-brand-500 disabled:cursor-not-allowed disabled:opacity-60"
            />
            {v.description && (
              <p className="mt-1 text-xs text-status-prevFg">
                {v.description}
              </p>
            )}
          </div>
        );
      })}
    </>
  );
}
