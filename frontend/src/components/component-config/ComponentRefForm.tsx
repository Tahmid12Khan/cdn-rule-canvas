"use client";

// ComponentRefForm (component-editor design §5). Controlled form for a
// `component_ref` (HTML feature) or `component_ref_json` (JSON feature) outcome
// component config. It references a GLOBAL library component
// (lib/api/componentTemplates.ts) by id + version, populates the component's
// declared variables, and sets the placement target:
//   - HTML (component_ref):      target_selector + placement_mode
//   - JSON (component_ref_json): target_path
//
// Validation uses the zod ComponentRefConfig / ComponentRefJsonConfig schemas
// directly (safeParse) — same pattern as the sibling HtmlInjectionForm /
// JsonMutationForm (react-hook-form is not declared in package.json). On a valid
// submit it lifts the fully-validated config to the parent modal, which owns the
// slug / placement and the actual mutation.
//
// The component picker (listComponentTemplates), version select (each
// version_number from getComponentTemplate + a "Default" option), variables
// sub-form (resolveComponent → one input per declared variable) and live
// ComponentPreview mirror the rule-node integration in canvas/config/
// GenericNodeForm. Changing the chosen component resets the version to default
// and clears the variable values so the form starts clean.
import { useEffect, useMemo, useState, type FormEvent } from "react";
import { useQuery } from "@tanstack/react-query";
import clsx from "clsx";

import { ComponentPreview } from "@/components/component-library/ComponentPreview";
import {
  componentKeys,
  getComponentTemplate,
  listComponentTemplates,
  resolveComponent,
} from "@/lib/api/componentTemplates";
import {
  ComponentRefConfig,
  ComponentRefJsonConfig,
  HtmlPlacementMode,
  type ComponentRefConfig as ComponentRefConfigT,
  type ComponentRefJsonConfig as ComponentRefJsonConfigT,
} from "@/lib/schemas/components";

export type ComponentRefAnyConfig =
  | ComponentRefConfigT
  | ComponentRefJsonConfigT;

type PlacementModeValue = ComponentRefConfigT["placement_mode"];

const PLACEMENT_MODE_OPTIONS: { value: PlacementModeValue; label: string }[] = [
  { value: "replace", label: "Replace" },
  { value: "append", label: "Append" },
  { value: "prepend", label: "Prepend" },
  { value: "before", label: "Before" },
  { value: "after", label: "After" },
];

interface ComponentRefFormProps {
  formId: string;
  // "component_ref" → HTML target (selector + placement_mode);
  // "component_ref_json" → JSON target (target_path).
  type: "component_ref" | "component_ref_json";
  initial: ComponentRefAnyConfig;
  onValidSubmit: (config: ComponentRefAnyConfig) => void;
  onDirtyChange?: (dirty: boolean) => void;
}

type FieldErrors = Partial<
  Record<
    "component_id" | "version" | "target_selector" | "target_path",
    string
  >
>;

// The stored version: the literal "default" or a positive version_number.
function coerceVersion(raw: string): "default" | number {
  if (raw === "default" || raw === "") return "default";
  const n = Number(raw);
  return Number.isFinite(n) ? n : "default";
}

export function ComponentRefForm({
  formId,
  type,
  initial,
  onValidSubmit,
  onDirtyChange,
}: ComponentRefFormProps) {
  const isHtml = type === "component_ref";

  const [componentId, setComponentId] = useState(initial.component_id);
  const [version, setVersion] = useState<"default" | number>(initial.version);
  const [variables, setVariables] = useState<Record<string, string>>(
    initial.variables ?? {},
  );
  const [targetSelector, setTargetSelector] = useState(
    initial.type === "component_ref" ? initial.target_selector : "",
  );
  const [placementMode, setPlacementMode] = useState<PlacementModeValue>(
    initial.type === "component_ref" ? initial.placement_mode : "append",
  );
  const [targetPath, setTargetPath] = useState(
    initial.type === "component_ref_json" ? initial.target_path : "",
  );
  const [errors, setErrors] = useState<FieldErrors>({});

  function markDirty() {
    onDirtyChange?.(true);
  }

  // ── Component-select options (the library list) ────────────────────────────
  const listQuery = useQuery({
    queryKey: componentKeys.list({ page: 1, page_size: 100 }),
    queryFn: () => listComponentTemplates({ page: 1, page_size: 100 }),
  });
  const componentOptions = listQuery.data?.items ?? [];

  // ── Version-select options (the chosen component's versions) ───────────────
  const detailQuery = useQuery({
    queryKey: componentKeys.detail(componentId),
    queryFn: () => getComponentTemplate(componentId),
    enabled: componentId !== "",
  });
  const versionNumbers = useMemo(
    () =>
      (detailQuery.data?.versions ?? [])
        .map((v) => v.version_number)
        .sort((a, b) => b - a),
    [detailQuery.data],
  );

  // ── Resolved (component, version) → declared variables + html for preview ──
  const resolveQuery = useQuery({
    queryKey: ["component-templates", "resolve", componentId, version],
    queryFn: () => resolveComponent(componentId, version),
    enabled: componentId !== "",
  });
  const resolved = resolveQuery.data;
  const declaredVars = useMemo(() => resolved?.variables ?? [], [resolved]);

  // Once the declared set resolves, drop any stored variable value whose name is
  // no longer declared, mirroring the rule-node sub-form (design §5.4).
  const declaredKey = useMemo(
    () => declaredVars.map((v) => v.name).join(" "),
    [declaredVars],
  );
  const hasResolved = Boolean(resolved);
  useEffect(() => {
    if (!hasResolved) return;
    const allowed = new Set(declaredKey.split(" ").filter(Boolean));
    setVariables((prev) => {
      const next: Record<string, string> = {};
      let changed = false;
      for (const [k, val] of Object.entries(prev)) {
        if (allowed.has(k)) next[k] = val;
        else changed = true;
      }
      if (!changed) return prev;
      return next;
    });
  }, [declaredKey, hasResolved]);

  // Changing the component resets the version to "default" (a version of the
  // previous component is meaningless for the new one) and clears any stored
  // variable values, so the preview + sub-form start clean immediately.
  function handleComponentChange(id: string) {
    setComponentId(id);
    setVersion("default");
    setVariables({});
    markDirty();
  }

  function setVariable(name: string, value: string) {
    setVariables((prev) => ({ ...prev, [name]: value }));
    markDirty();
  }

  function handleSubmit(e: FormEvent<HTMLFormElement>) {
    e.preventDefault();

    if (isHtml) {
      const candidate = {
        type: "component_ref" as const,
        component_id: componentId,
        version,
        variables,
        target_selector: targetSelector,
        placement_mode: placementMode,
      };
      const parsed = ComponentRefConfig.safeParse(candidate);
      if (!parsed.success) {
        const next: FieldErrors = {};
        for (const issue of parsed.error.issues) {
          const key = issue.path[0];
          if (
            key === "component_id" ||
            key === "version" ||
            key === "target_selector"
          ) {
            next[key] = issue.message;
          }
        }
        setErrors(next);
        return;
      }
      setErrors({});
      onValidSubmit(parsed.data);
      return;
    }

    const candidate = {
      type: "component_ref_json" as const,
      component_id: componentId,
      version,
      variables,
      target_path: targetPath,
    };
    const parsed = ComponentRefJsonConfig.safeParse(candidate);
    if (!parsed.success) {
      const next: FieldErrors = {};
      for (const issue of parsed.error.issues) {
        const key = issue.path[0];
        if (key === "component_id" || key === "version" || key === "target_path") {
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
      {/* Component picker */}
      <div>
        <label
          htmlFor="cr-component"
          className="mb-1 block text-sm font-medium text-fg"
        >
          Component
        </label>
        <select
          id="cr-component"
          value={componentId}
          onChange={(e) => handleComponentChange(e.target.value)}
          aria-invalid={Boolean(errors.component_id) || undefined}
          aria-describedby={
            errors.component_id ? "cr-component-error" : undefined
          }
          className={clsx(
            "w-full rounded-md border px-3 py-2 text-sm focus:outline-none",
            errors.component_id
              ? "border-danger focus:border-danger"
              : "border-border focus:border-brand-500",
          )}
        >
          <option value="">
            {componentOptions.length === 0
              ? "No components yet"
              : "Select a component…"}
          </option>
          {componentOptions.map((c) => (
            <option key={c.id} value={c.id}>
              {c.name}
            </option>
          ))}
        </select>
        {errors.component_id && (
          <p
            id="cr-component-error"
            role="alert"
            className="mt-1 text-xs text-danger"
          >
            {errors.component_id}
          </p>
        )}
      </div>

      {/* Version select */}
      <div>
        <label
          htmlFor="cr-version"
          className="mb-1 block text-sm font-medium text-fg"
        >
          Version
        </label>
        <select
          id="cr-version"
          value={version === "default" ? "default" : String(version)}
          onChange={(e) => {
            setVersion(coerceVersion(e.target.value));
            markDirty();
          }}
          disabled={componentId === ""}
          className="w-full rounded-md border border-border px-3 py-2 text-sm focus:border-brand-500 focus:outline-none disabled:cursor-not-allowed disabled:opacity-60"
        >
          <option value="default">Default (follows component)</option>
          {versionNumbers.map((n) => (
            <option key={n} value={n}>
              v{n}
            </option>
          ))}
        </select>
      </div>

      {/* Placement target — HTML: selector + mode; JSON: path */}
      {isHtml ? (
        <>
          <div>
            <label
              htmlFor="cr-target-selector"
              className="mb-1 block text-sm font-medium text-fg"
            >
              Target selector
            </label>
            <input
              id="cr-target-selector"
              type="text"
              value={targetSelector}
              onChange={(e) => {
                setTargetSelector(e.target.value);
                markDirty();
              }}
              placeholder="e.g. .article-body"
              aria-invalid={Boolean(errors.target_selector) || undefined}
              aria-describedby={
                errors.target_selector ? "cr-target-selector-error" : undefined
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
                id="cr-target-selector-error"
                role="alert"
                className="mt-1 text-xs text-danger"
              >
                {errors.target_selector}
              </p>
            )}
          </div>

          <div>
            <label
              htmlFor="cr-placement-mode"
              className="mb-1 block text-sm font-medium text-fg"
            >
              Placement mode
            </label>
            <select
              id="cr-placement-mode"
              value={placementMode}
              onChange={(e) => {
                setPlacementMode(HtmlPlacementMode.parse(e.target.value));
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
        </>
      ) : (
        <div>
          <label
            htmlFor="cr-target-path"
            className="mb-1 block text-sm font-medium text-fg"
          >
            Target path
          </label>
          <input
            id="cr-target-path"
            type="text"
            value={targetPath}
            onChange={(e) => {
              setTargetPath(e.target.value);
              markDirty();
            }}
            placeholder="e.g. $.content.html"
            aria-invalid={Boolean(errors.target_path) || undefined}
            aria-describedby={
              errors.target_path ? "cr-target-path-error" : undefined
            }
            className={clsx(
              "w-full rounded-md border px-3 py-2 font-mono text-sm focus:outline-none",
              errors.target_path
                ? "border-danger focus:border-danger"
                : "border-border focus:border-brand-500",
            )}
          />
          {errors.target_path && (
            <p
              id="cr-target-path-error"
              role="alert"
              className="mt-1 text-xs text-danger"
            >
              {errors.target_path}
            </p>
          )}
        </div>
      )}

      {/* Variables sub-form + live preview — only once a component is chosen and
          its declared variables have resolved (design §5 / §7). */}
      {componentId !== "" && (
        <div
          className="space-y-3 border-t border-border pt-4"
          data-testid="component-ref-variables"
        >
          <p className="text-sm font-medium text-fg">Variables</p>

          {resolveQuery.isLoading && (
            <p className="text-xs text-fg-muted">Loading variables…</p>
          )}

          {!resolveQuery.isLoading && declaredVars.length === 0 && (
            <p className="text-xs text-fg-muted">
              This component has no variables.
            </p>
          )}

          {declaredVars.map((v) => {
            const id = `cr-var-${v.name}`;
            return (
              <div key={v.name}>
                <label
                  htmlFor={id}
                  className="mb-1 block text-sm font-medium text-fg"
                >
                  {v.title}
                </label>
                <input
                  id={id}
                  type="text"
                  value={variables[v.name] ?? ""}
                  onChange={(e) => setVariable(v.name, e.target.value)}
                  placeholder={v.title}
                  className="w-full rounded-md border border-border px-3 py-2 text-sm focus:border-brand-500 focus:outline-none"
                />
                {v.description && (
                  <p className="mt-1 text-xs text-fg-muted">{v.description}</p>
                )}
              </div>
            );
          })}

          {resolved && (
            <div data-testid="component-ref-preview-wrap">
              <p className="mb-1 text-sm font-medium text-fg">Preview</p>
              <div className="rounded-md border border-border bg-bg p-3">
                <ComponentPreview html={resolved.html_body} values={variables} />
              </div>
            </div>
          )}
        </div>
      )}
    </form>
  );
}
