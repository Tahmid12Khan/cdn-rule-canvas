"use client";

// SavedOutcomeEditorPage (Outcomes Library design). Client orchestrator for the
// Outcomes Library editor — simpler than ComponentEditorPage since there's no
// HTML editing (the HTML lives on the referenced Component, not here):
//   - A component picker showing the referenced component (locked: the backend
//     SavedOutcomeUpdate DTO has no component_id field, so the reference can't
//     change after creation — pick a different component by creating a new
//     saved outcome instead).
//   - A version picker: "Latest" (version_number: null, follows the component's
//     own default) or a pinned version_number.
//   - A Variables sub-form (shared VariablesForm) driven by the resolved
//     (component, version)'s declared variables, plus a live ComponentPreview.
//   - Save (PATCH) persists name + version_number + variables.
//
// The route param is the SAVED OUTCOME'S SLUG; the backend only exposes
// id-keyed saved-outcome routes (no by-slug lookup — Task 4 scope), so we
// resolve slug -> outcome by fetching the list and matching client-side.
import { useEffect, useMemo, useRef, useState } from "react";
import { useRouter } from "next/navigation";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";

import { ComponentPreview } from "@/components/component-library/ComponentPreview";
import { VariablesForm } from "@/components/shared/VariablesForm";
import { ErrorBanner } from "@/components/ui/ErrorBanner";
import {
  componentKeys,
  getComponentTemplate,
  listComponentTemplates,
  resolveComponent,
} from "@/lib/api/componentTemplates";
import {
  listSavedOutcomes,
  savedOutcomeKeys,
  updateSavedOutcome,
  type SavedOutcomeUpdate,
} from "@/lib/api/savedOutcomes";
import { toUserError, type UserError } from "@/lib/errors/userError";

interface SavedOutcomeEditorPageProps {
  slug: string;
}

// No by-slug endpoint exists (see module comment) — page over the full list.
const LIST_PAGE_SIZE = 200;

export function SavedOutcomeEditorPage({ slug }: SavedOutcomeEditorPageProps) {
  const router = useRouter();
  const queryClient = useQueryClient();

  const listParams = { page: 1, page_size: LIST_PAGE_SIZE };
  const listQuery = useQuery({
    queryKey: savedOutcomeKeys.list(listParams),
    queryFn: () => listSavedOutcomes(listParams),
  });
  const outcome = listQuery.data?.items.find((o) => o.slug === slug);
  const componentId = outcome?.component_id ?? "";

  // Local draft, seeded once per loaded outcome.
  const [nameDraft, setNameDraft] = useState("");
  const [versionDraft, setVersionDraft] = useState<"latest" | number>("latest");
  const [variables, setVariables] = useState<Record<string, string>>({});
  const [saveError, setSaveError] = useState<UserError | null>(null);
  const seeded = useRef<string | null>(null);
  useEffect(() => {
    if (!outcome || seeded.current === outcome.id) return;
    seeded.current = outcome.id;
    setNameDraft(outcome.name);
    setVersionDraft(outcome.version_number ?? "latest");
    setVariables(outcome.variables);
    setSaveError(null);
  }, [outcome]);

  // Component list — only for showing the (locked) referenced component's name.
  const componentsQuery = useQuery({
    queryKey: componentKeys.list({ page: 1, page_size: 100 }),
    queryFn: () => listComponentTemplates({ page: 1, page_size: 100 }),
  });
  const componentOptions = componentsQuery.data?.items ?? [];

  // Version options: each version_number of the referenced component.
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

  // Resolved (component, version) -> declared variables + html for preview.
  // "Latest" maps to the component's own "default" selector (the same
  // resolution the backend's resolve endpoint uses for version_number: None).
  const resolveSelector = versionDraft === "latest" ? "default" : versionDraft;
  const resolveQuery = useQuery({
    queryKey: [
      "component-templates",
      "resolve",
      componentId,
      resolveSelector,
    ],
    queryFn: () => resolveComponent(componentId, resolveSelector),
    enabled: componentId !== "",
  });
  const resolved = resolveQuery.data;
  const declaredVars = useMemo(() => resolved?.variables ?? [], [resolved]);

  // Only variable names declared by the currently-resolved version are
  // persisted (mirrors GenericNodeForm's Component Variables sub-form: drop
  // names not in the resolved set).
  const persistedVariables = useMemo(() => {
    const out: Record<string, string> = {};
    for (const v of declaredVars) out[v.name] = variables[v.name] ?? "";
    return out;
  }, [declaredVars, variables]);

  const dirty = useMemo(() => {
    if (!outcome || !resolved) return false;
    if (nameDraft.trim() !== outcome.name) return true;
    const normalizedVersion = versionDraft === "latest" ? null : versionDraft;
    if (normalizedVersion !== outcome.version_number) return true;
    return (
      JSON.stringify(persistedVariables) !== JSON.stringify(outcome.variables)
    );
  }, [outcome, resolved, nameDraft, versionDraft, persistedVariables]);

  const updateMutation = useMutation({
    mutationFn: (body: SavedOutcomeUpdate) =>
      updateSavedOutcome(outcome?.id ?? "", body),
    onSuccess: () => {
      void queryClient.invalidateQueries({ queryKey: ["saved-outcomes"] });
      setVariables(persistedVariables);
    },
    onError: (err: unknown) => setSaveError(toUserError(err, { surface: "save" })),
  });

  function handleSave() {
    if (!outcome) return;
    setSaveError(null);
    updateMutation.mutate({
      name: nameDraft.trim(),
      version_number: versionDraft === "latest" ? null : versionDraft,
      variables: persistedVariables,
    });
  }

  function setVariable(name: string, value: string) {
    setVariables((prev) => ({ ...prev, [name]: value }));
  }

  const previewValues = declaredVars.length > 0 ? persistedVariables : variables;

  // ── render states ──────────────────────────────────────────────────────────
  if (listQuery.isError) {
    return (
      <ErrorBanner
        error={toUserError(listQuery.error, { surface: "load" })}
        onRetry={() => void listQuery.refetch()}
      />
    );
  }
  if (!listQuery.isPending && !outcome) {
    return (
      <ErrorBanner
        error={{
          title: "Outcome not found",
          why: `No saved outcome with slug "${slug}" exists.`,
          howToFix: "Go back to the Outcomes library and pick a valid entry.",
        }}
      />
    );
  }
  if (!outcome) {
    return (
      <div className="space-y-4">
        <div className="h-7 w-48 animate-pulse rounded bg-bg-overlay" />
        <div className="h-96 animate-pulse rounded-lg bg-bg-overlay" />
      </div>
    );
  }

  return (
    <div className="flex flex-col gap-5">
      <div className="flex items-start justify-between gap-4">
        <div>
          <button
            type="button"
            onClick={() => router.push("/products/outcomes")}
            className="text-sm text-accent-onMuted hover:underline"
          >
            ← Outcomes
          </button>
          <h1 className="mt-1 text-2xl font-bold text-fg">{outcome.name}</h1>
          <code className="text-xs text-fg-muted">{outcome.slug}</code>
        </div>
        <button
          type="button"
          onClick={handleSave}
          disabled={!dirty || updateMutation.isPending}
          className="inline-flex items-center rounded-md bg-accent px-4 py-2 text-sm font-medium text-accent-fg shadow-sm transition hover:opacity-90 disabled:cursor-not-allowed disabled:opacity-50"
        >
          {updateMutation.isPending ? "Saving…" : "Save"}
        </button>
      </div>

      {saveError && <ErrorBanner error={saveError} />}

      <div className="grid grid-cols-1 gap-5 lg:grid-cols-2">
        {/* LEFT: name + component (locked) + version */}
        <section className="flex flex-col gap-5">
          <div>
            <label
              htmlFor="outcome-name"
              className="mb-2 block text-sm font-semibold uppercase tracking-wide text-fg-muted"
            >
              Name
            </label>
            <input
              id="outcome-name"
              value={nameDraft}
              onChange={(e) => setNameDraft(e.target.value)}
              className="w-full rounded-md border border-border bg-bg px-3 py-2 text-sm text-fg shadow-sm focus:border-accent focus:outline-none focus:ring-1 focus:ring-accent"
            />
          </div>

          <div>
            <label
              htmlFor="outcome-component"
              className="mb-2 block text-sm font-semibold uppercase tracking-wide text-fg-muted"
            >
              Component
            </label>
            <select
              id="outcome-component"
              value={componentId}
              onChange={() => undefined}
              disabled
              className="w-full rounded-md border border-status-prevBg px-3 py-2 text-sm disabled:cursor-not-allowed disabled:opacity-60"
            >
              {componentOptions.map((c) => (
                <option key={c.id} value={c.id}>
                  {c.name}
                </option>
              ))}
            </select>
            <p className="mt-1 text-xs text-fg-muted">
              The referenced component can&apos;t be changed. Create a new
              outcome to reference a different one.
            </p>
          </div>

          <div>
            <label
              htmlFor="outcome-version"
              className="mb-2 block text-sm font-semibold uppercase tracking-wide text-fg-muted"
            >
              Version
            </label>
            <select
              id="outcome-version"
              value={String(versionDraft)}
              onChange={(e) => {
                const raw = e.target.value;
                setVersionDraft(raw === "latest" ? "latest" : Number(raw));
              }}
              disabled={componentId === ""}
              className="w-full rounded-md border border-status-prevBg px-3 py-2 text-sm focus:border-brand-500 disabled:cursor-not-allowed disabled:opacity-60"
            >
              <option value="latest">Latest (follows component)</option>
              {versionNumbers.map((n) => (
                <option key={n} value={n}>
                  v{n}
                </option>
              ))}
            </select>
          </div>
        </section>

        {/* RIGHT: variables + live preview */}
        <section className="flex flex-col gap-5">
          <div>
            <h2 className="mb-2 text-sm font-semibold uppercase tracking-wide text-fg-muted">
              Variables
            </h2>
            {resolveQuery.isLoading && (
              <p className="text-xs text-fg-muted">Loading variables…</p>
            )}
            {!resolveQuery.isLoading && (
              <div className="space-y-3">
                <VariablesForm
                  declaredVars={declaredVars}
                  values={variables}
                  onChange={setVariable}
                />
              </div>
            )}
          </div>

          {resolved && (
            <div>
              <h2 className="mb-2 text-sm font-semibold uppercase tracking-wide text-fg-muted">
                Preview
              </h2>
              <div className="rounded-lg border border-border bg-bg p-4">
                <ComponentPreview
                  html={resolved.html_body}
                  values={previewValues}
                />
              </div>
            </div>
          )}
        </section>
      </div>
    </div>
  );
}
