"use client";

// Variables panel (design §5.3 right pane). Lists every `{{name}}` extracted
// from the body and lets the author annotate `title` + `description`. Two drift
// signals are surfaced:
//   - a name present in the body but not yet annotated is auto-added (handled by
//     the parent's merge in ComponentEditorPage),
//   - an annotated name that is absent from the body is flagged "unused".
// The author may delete an unused row. Names are read-only here (they come from
// the body); editing a name means editing the template.
import type { ComponentVariable } from "@/lib/schemas/componentTemplates";

const inputClass =
  "w-full rounded-md border border-border bg-bg px-2.5 py-1.5 text-sm text-fg shadow-sm focus:border-accent focus:outline-none focus:ring-1 focus:ring-accent disabled:cursor-not-allowed disabled:opacity-60";
const labelClass = "text-xs font-medium text-fg-muted";

interface VariablesPanelProps {
  variables: ComponentVariable[];
  // Names currently present in the body (drives the "unused" flag).
  declaredInBody: string[];
  onChange: (next: ComponentVariable[]) => void;
  disabled?: boolean;
}

export function VariablesPanel({
  variables,
  declaredInBody,
  onChange,
  disabled = false,
}: VariablesPanelProps) {
  const bodySet = new Set(declaredInBody);

  function patch(name: string, field: "title" | "description", value: string) {
    onChange(
      variables.map((v) =>
        v.name === name ? { ...v, [field]: value } : v,
      ),
    );
  }

  function remove(name: string) {
    onChange(variables.filter((v) => v.name !== name));
  }

  if (variables.length === 0) {
    return (
      <p className="text-sm text-fg-muted">
        No variables yet. Add a <code className="font-mono">{"{{name}}"}</code>{" "}
        placeholder in the HTML and it will appear here.
      </p>
    );
  }

  return (
    <ul className="flex flex-col gap-4">
      {variables.map((v) => {
        const unused = !bodySet.has(v.name);
        return (
          <li
            key={v.name}
            className="flex flex-col gap-2 rounded-md border border-border bg-bg-elevated p-3"
          >
            <div className="flex items-center justify-between gap-2">
              <code className="rounded bg-bg-overlay px-1.5 py-0.5 font-mono text-xs text-accent-onMuted">
                {`{{${v.name}}}`}
              </code>
              {unused && (
                <span className="rounded-full bg-danger-bg px-2 py-0.5 text-xs font-medium text-danger">
                  unused
                </span>
              )}
            </div>

            <label className="flex flex-col gap-1">
              <span className={labelClass}>Title</span>
              <input
                value={v.title}
                onChange={(e) => patch(v.name, "title", e.target.value)}
                placeholder="Human-friendly label"
                disabled={disabled}
                aria-label={`Title for ${v.name}`}
                className={inputClass}
              />
            </label>

            <label className="flex flex-col gap-1">
              <span className={labelClass}>Description (optional)</span>
              <textarea
                value={v.description ?? ""}
                onChange={(e) => patch(v.name, "description", e.target.value)}
                placeholder="What goes here, who fills it in the rule"
                rows={2}
                disabled={disabled}
                aria-label={`Description for ${v.name}`}
                className={inputClass}
              />
            </label>

            {unused && !disabled && (
              <button
                type="button"
                onClick={() => remove(v.name)}
                className="self-start text-xs font-medium text-danger hover:underline"
              >
                Remove unused variable
              </button>
            )}
          </li>
        );
      })}
    </ul>
  );
}
