"use client";

// ComponentEditorPage (design §5.3). Client orchestrator for the Component
// Editor split layout:
//   LEFT  — CodeMirror 6 HTML editor (html() lang) with the advisory lint gutter
//           fed by htmlLintEngine (NEVER blocks Save, design item 8).
//   RIGHT — Variables panel (auto-extracted names, annotate title+description,
//           flag unused/undeclared) + live ComponentPreview.
//   TOP   — version bar (switch / create / delete / set-default).
//
// The route param is the component SLUG; the backend keys detail/version routes
// on the UUID, so we resolve slug → id via the by-slug endpoint.
//
// Local draft state (html_body / variables / description) is staged and
// committed on Save (PATCH version). Body-extracted variable names are merged
// into the annotated list on every edit so the Variables panel stays in sync.
import { useEffect, useMemo, useRef, useState } from "react";
import dynamic from "next/dynamic";
import { useRouter } from "next/navigation";
import { useQuery } from "@tanstack/react-query";

import { ComponentPreview } from "@/components/component-library/ComponentPreview";
import { VariablesPanel } from "@/components/component-library/VariablesPanel";
import { VersionBar } from "@/components/component-library/VersionBar";
import { ErrorBanner } from "@/components/ui/ErrorBanner";
import {
  componentKeys,
  getComponentTemplateBySlug,
  getVersion,
  useComponentTemplate,
  useUpdateVersion,
} from "@/lib/api/componentTemplates";
import { extractVariables } from "@/lib/canvas/extractVariables";
import { toUserError, type UserError } from "@/lib/errors/userError";
import type { ComponentVariable } from "@/lib/schemas/componentTemplates";

// CodeMirror touches window/document at import → load client-only.
const HtmlCodeEditor = dynamic(
  () =>
    import("@/components/component-library/HtmlCodeEditor").then(
      (m) => m.HtmlCodeEditor,
    ),
  { ssr: false },
);

interface ComponentEditorPageProps {
  slug: string;
}

// Merge the body's extracted variable names with the annotated list: keep
// existing annotations, append newly-seen names (in body order), and preserve
// annotated-but-unused names (so the panel can flag them rather than silently
// dropping the author's metadata).
function mergeVariables(
  bodyNames: string[],
  annotated: ComponentVariable[],
): ComponentVariable[] {
  const byName = new Map(annotated.map((v) => [v.name, v]));
  const out: ComponentVariable[] = [];
  const placed = new Set<string>();
  for (const name of bodyNames) {
    out.push(byName.get(name) ?? { name, title: name });
    placed.add(name);
  }
  // Trailing annotated-but-unused names (flagged in the panel).
  for (const v of annotated) {
    if (!placed.has(v.name)) out.push(v);
  }
  return out;
}

export function ComponentEditorPage({ slug }: ComponentEditorPageProps) {
  const router = useRouter();

  // Resolve slug → component id via the dedicated by-slug endpoint (404s on an
  // unknown slug → surfaced as a load error below).
  const idQuery = useQuery({
    queryKey: ["component-templates", "by-slug", slug],
    queryFn: async () => {
      const component = await getComponentTemplateBySlug(slug);
      return component.id;
    },
  });
  const componentId = idQuery.data ?? "";

  const detailQuery = useComponentTemplate(componentId, componentId !== "");
  const component = detailQuery.data;

  // Selected version: defaults to the component's default (or latest).
  const [selectedVnum, setSelectedVnum] = useState<number | null>(null);
  useEffect(() => {
    if (component && selectedVnum === null) {
      setSelectedVnum(
        component.default_version_number ?? component.latest_version_number,
      );
    }
  }, [component, selectedVnum]);

  const versionQuery = useQuery({
    queryKey:
      componentId && selectedVnum !== null
        ? componentKeys.version(componentId, selectedVnum)
        : ["component-templates", "version", "pending"],
    queryFn: () => getVersion(componentId, selectedVnum as number),
    enabled: componentId !== "" && selectedVnum !== null,
  });
  const version = versionQuery.data;

  // Local draft, seeded once per loaded version.
  const [htmlBody, setHtmlBody] = useState("");
  const [variables, setVariables] = useState<ComponentVariable[]>([]);
  // Per-version description is editable in place (design §5.3). "" === unset.
  const [descriptionDraft, setDescriptionDraft] = useState("");
  const [saveError, setSaveError] = useState<UserError | null>(null);
  const seeded = useRef<string | null>(null);
  useEffect(() => {
    if (!version) return;
    const key = `${version.id}`;
    if (seeded.current === key) return;
    seeded.current = key;
    setHtmlBody(version.html_body);
    setVariables(version.variables);
    setDescriptionDraft(version.description ?? "");
    setSaveError(null);
  }, [version]);

  // Re-seed when switching versions (id changes → effect re-runs).
  const bodyNames = useMemo(() => extractVariables(htmlBody), [htmlBody]);

  function handleHtmlChange(next: string) {
    setHtmlBody(next);
    setVariables((prev) => mergeVariables(extractVariables(next), prev));
  }

  const updateVersion = useUpdateVersion(componentId);
  const dirty = useMemo(() => {
    if (!version) return false;
    if (htmlBody !== version.html_body) return true;
    if (descriptionDraft.trim() !== (version.description ?? "").trim())
      return true;
    return JSON.stringify(variables) !== JSON.stringify(version.variables);
  }, [version, htmlBody, variables, descriptionDraft]);

  function handleSave() {
    if (selectedVnum === null) return;
    setSaveError(null);
    // Only persist variables that are actually present in the body; unused
    // (drift) rows are dropped on save.
    const bodySet = new Set(bodyNames);
    const persisted = variables.filter((v) => bodySet.has(v.name));
    updateVersion.mutate(
      {
        vnum: selectedVnum,
        body: {
          html_body: htmlBody,
          variables: persisted,
          description: descriptionDraft.trim() || undefined,
        },
      },
      {
        onSuccess: () => {
          // Sync local state to the FILTERED/persisted set so the dirty memo
          // (which compares against the now-refetched version) doesn't flip back
          // to true on dropped (drift) rows that were never persisted.
          setVariables(persisted);
        },
        onError: (err) => setSaveError(toUserError(err, { surface: "save" })),
      },
    );
  }

  // Preview uses each variable's title as the placeholder sample value.
  const previewValues = useMemo(() => {
    const values: Record<string, string> = {};
    for (const v of variables) values[v.name] = v.title || v.name;
    return values;
  }, [variables]);

  // ── render states ──────────────────────────────────────────────────────────
  if (idQuery.isError) {
    return (
      <ErrorBanner
        error={toUserError(idQuery.error, { surface: "load" })}
        onRetry={() => void idQuery.refetch()}
      />
    );
  }
  if (detailQuery.isError) {
    return (
      <ErrorBanner
        error={toUserError(detailQuery.error, { surface: "load" })}
        onRetry={() => void detailQuery.refetch()}
      />
    );
  }
  if (!component || selectedVnum === null) {
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
            onClick={() => router.push("/products/components")}
            className="text-sm text-accent-onMuted hover:underline"
          >
            ← Components
          </button>
          <h1 className="mt-1 text-2xl font-bold text-fg">{component.name}</h1>
          <code className="text-xs text-fg-muted">{component.slug}</code>
        </div>
        <button
          type="button"
          onClick={handleSave}
          disabled={!dirty || updateVersion.isPending}
          className="inline-flex items-center rounded-md bg-accent px-4 py-2 text-sm font-medium text-accent-fg shadow-sm transition hover:opacity-90 disabled:cursor-not-allowed disabled:opacity-50"
        >
          {updateVersion.isPending ? "Saving…" : "Save"}
        </button>
      </div>

      {saveError && <ErrorBanner error={saveError} />}

      <VersionBar
        component={component}
        selectedVnum={selectedVnum}
        onSelectVersion={(vnum) => {
          // Reset the seed guard so the new version's body re-seeds the draft.
          seeded.current = null;
          setSelectedVnum(vnum);
        }}
        dirty={dirty}
      />

      <div className="grid grid-cols-1 gap-5 lg:grid-cols-2">
        {/* LEFT: HTML editor */}
        <section className="flex flex-col gap-2">
          <h2 className="text-sm font-semibold uppercase tracking-wide text-fg-muted">
            HTML
          </h2>
          <div className="h-[28rem]">
            <HtmlCodeEditor value={htmlBody} onChange={handleHtmlChange} />
          </div>
          <p className="text-xs text-fg-muted">
            Use <code className="font-mono">{"{{name}}"}</code> for escaped
            values or <code className="font-mono">{"{{{name}}}"}</code> for raw
            HTML. Lint issues are advisory and never block Save.
          </p>
        </section>

        {/* RIGHT: version description + variables + live preview */}
        <section className="flex flex-col gap-5">
          <div>
            <label
              htmlFor="version-description"
              className="mb-2 block text-sm font-semibold uppercase tracking-wide text-fg-muted"
            >
              Version description
            </label>
            <input
              id="version-description"
              value={descriptionDraft}
              onChange={(e) => setDescriptionDraft(e.target.value)}
              placeholder="e.g. Summer headline experiment"
              className="w-full rounded-md border border-border bg-bg px-3 py-2 text-sm text-fg shadow-sm focus:border-accent focus:outline-none focus:ring-1 focus:ring-accent"
            />
            <p className="mt-1 text-xs text-fg-muted">
              A short note about this version. Saved with the version on Save.
            </p>
          </div>

          <div>
            <h2 className="mb-2 text-sm font-semibold uppercase tracking-wide text-fg-muted">
              Variables
            </h2>
            <VariablesPanel
              variables={variables}
              declaredInBody={bodyNames}
              onChange={setVariables}
            />
          </div>

          <div>
            <h2 className="mb-2 text-sm font-semibold uppercase tracking-wide text-fg-muted">
              Preview
            </h2>
            <div className="rounded-lg border border-border bg-bg p-4">
              <ComponentPreview html={htmlBody} values={previewValues} />
            </div>
            <p className="mt-1 text-xs text-fg-muted">
              Sample values use each variable&apos;s title. Final values are set
              per-rule.
            </p>
          </div>
        </section>
      </div>
    </div>
  );
}
