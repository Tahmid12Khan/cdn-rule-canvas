"use client";

// OutcomeEditorPage (FRONTEND CONTRACT §2.5, Task 15).
//
// Client orchestrator for the Edit Outcome screen. Owns:
//   - the server query `['outcome', oid]` (TanStack Query),
//   - local draft state for the title/description + the ordered component list
//     (adds, edits, deletes, reorders are staged locally, never optimistically
//     written to the query cache — Save commits them in one batch),
//   - dirty tracking + a discard-confirm on Cancel,
//   - the batched Save: PATCH outcome details + POST new / PATCH edited /
//     DELETE removed components + reorder, then invalidate the relevant keys.
//
// The per-component config editing is delegated to Task 16's
// <ComponentConfigModal/>, which lifts a validated { slug, type, config,
// placement } payload back here.
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { useRouter } from "next/navigation";
import {
  useMutation,
  useQuery,
  useQueryClient,
} from "@tanstack/react-query";

import {
  ComponentConfigModal,
  type ComponentConfigSubmit,
} from "@/components/component-config/ComponentConfigModal";
import { AddComponentDrawer } from "@/components/outcome/AddComponentDrawer";
import type { DraftComponent } from "@/components/outcome/ComponentRow";
import {
  OutcomeDetailsForm,
  validateOutcomeDetails,
  type OutcomeDetailsValue,
} from "@/components/outcome/OutcomeDetailsForm";
import { OutcomeEditorFooter } from "@/components/outcome/OutcomeEditorFooter";
import { PreviewModal } from "@/components/outcome/PreviewModal";
import { SortableComponentList } from "@/components/outcome/SortableComponentList";
import { ErrorBanner } from "@/components/ui/ErrorBanner";
import type { Placement } from "@/lib/api/enums";
import { toUserError, type UserError } from "@/lib/errors/userError";
import { getVersion, type VersionRead } from "@/lib/api/canvasVersions";
import { useOnboardingStore } from "@/state/onboardingStore";
import {
  addComponent,
  deleteComponent,
  updateComponent,
  type ComponentRead,
} from "@/lib/api/components";
import {
  getOutcome,
  reorderComponents,
  updateOutcome,
  type OutcomeRead,
} from "@/lib/api/outcomes";
import {
  ComponentConfig,
  ComponentType,
  defaultConfigFor,
} from "@/lib/schemas/components";

export interface OutcomeEditorPageProps {
  featureType: string;
  featureSlug: string;
  vnum: string;
  outcomeId: string;
  // SSR-prefetched seeds: used as TanStack initialData so the editor
  // hydrates without a client-side fetch waterfall.
  initialOutcome?: OutcomeRead;
  initialVersion?: VersionRead;
}

// A staged component: the server row plus local edit bookkeeping.
interface StagedComponent extends DraftComponent {
  /** A locally-edited or newly-added config (overrides the server `config`). */
  pendingConfig?: ComponentConfig;
  /** True if slug/config/placement changed for an existing row. */
  edited?: boolean;
}

type ConfigTarget =
  | { kind: "create"; type: ComponentType; placement: Placement }
  | { kind: "edit"; componentId: string };

let tempCounter = 0;
function makeTempId(): string {
  tempCounter += 1;
  return `temp-${tempCounter}-${Date.now()}`;
}

function toStaged(c: ComponentRead): StagedComponent {
  return { ...c };
}

function resolvedConfig(c: StagedComponent): ComponentConfig | undefined {
  if (c.pendingConfig) return c.pendingConfig;
  const parsed = ComponentConfig.safeParse(c.config);
  return parsed.success ? parsed.data : undefined;
}

export function OutcomeEditorPage({
  featureType,
  featureSlug,
  vnum,
  outcomeId,
  initialOutcome,
  initialVersion,
}: OutcomeEditorPageProps) {
  const router = useRouter();
  const queryClient = useQueryClient();
  const completeOnboarding = useOnboardingStore((s) => s.complete);

  // Narrow the decorative route param to the feature content kind: it picks the
  // component types offered (HTML injection/truncation vs JSON remove/set/replace).
  const kind: "html" | "json" = featureType === "json" ? "json" : "html";

  const query = useQuery({
    queryKey: ["outcome", outcomeId],
    queryFn: () => getOutcome(outcomeId),
    initialData: initialOutcome,
  });

  // Reuse the same ['version', fid, vnum] cache RuleBuilderClient warms so the
  // editor knows whether the version is editable (DRAFT) — outcome/component
  // writes are rejected by the backend (409 VERSION_EDIT_LOCKED) otherwise, so
  // we must gate the form read-only to avoid staging edits that silently vanish.
  const vnumNumber = Number(vnum);
  const versionQuery = useQuery({
    queryKey: ["version", featureSlug, vnumNumber],
    queryFn: () => getVersion(featureSlug, vnumNumber),
    enabled: Number.isFinite(vnumNumber),
    initialData: initialVersion,
  });
  const editable = versionQuery.data?.status === "draft";
  const readOnly = versionQuery.data !== undefined && !editable;

  // Local draft state, seeded once from the query result.
  const [details, setDetails] = useState<OutcomeDetailsValue | null>(null);
  const [components, setComponents] = useState<StagedComponent[] | null>(null);
  const [deletedIds, setDeletedIds] = useState<string[]>([]);
  const [configTarget, setConfigTarget] = useState<ConfigTarget | null>(null);
  const [saveError, setSaveError] = useState<UserError | null>(null);

  // Seed local state once per loaded outcome id. The ref guards against
  // re-seeding on re-render (which would clobber edits); the effect re-runs
  // when a new outcome resolves.
  const seededId = useRef<string | null>(null);
  const loadedId = query.data?.id ?? null;
  useEffect(() => {
    const outcome = query.data;
    if (!outcome || seededId.current === outcome.id) return;
    seededId.current = outcome.id;
    setDetails({
      title: outcome.title,
      description: outcome.description ?? "",
    });
    setComponents(
      [...outcome.components]
        .sort((a, b) => a.order_index - b.order_index)
        .map(toStaged),
    );
    setDeletedIds([]);
    // Intentionally keyed on the loaded outcome id; seed inputs are stable for
    // a given outcome. The editor unmounts on Save (router.push), so no
    // post-Save re-seed is needed.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [loadedId]);

  const versionPath = `/products/features/${featureType}/${featureSlug}/${vnum}`;

  const saveMutation = useMutation({
    mutationFn: async () => {
      const outcome = query.data;
      if (!outcome || !details || !components) return;

      // 1. PATCH outcome details if changed.
      const detailsDirty =
        details.title !== outcome.title ||
        details.description !== (outcome.description ?? "");
      if (detailsDirty) {
        await updateOutcome(outcome.id, {
          title: details.title.trim(),
          description: details.description.trim() || undefined,
        });
      }

      // 2. DELETE removed (persisted) components.
      for (const id of deletedIds) {
        if (!id.startsWith("temp-")) {
          await deleteComponent(id);
        }
      }

      // 3. POST new + PATCH edited components. Build an id map so reorder can
      //    address the freshly-created server ids.
      const idMap = new Map<string, string>();
      for (let i = 0; i < components.length; i += 1) {
        const c = components[i];
        const cfg = resolvedConfig(c);
        if (c.isNew) {
          if (!cfg) continue;
          const created = await addComponent(outcome.id, {
            slug: c.slug,
            type: c.type,
            config: cfg,
            placement: c.placement,
            order_index: i,
          });
          idMap.set(c.id, created.id);
        } else if (c.edited) {
          await updateComponent(c.id, {
            slug: c.slug,
            config: cfg,
            placement: c.placement,
          });
          idMap.set(c.id, c.id);
        } else {
          idMap.set(c.id, c.id);
        }
      }

      // 4. Reorder if the persisted order changed (or anything was added).
      const items = components
        .map((c, index) => {
          const serverId = idMap.get(c.id) ?? c.id;
          return serverId.startsWith("temp-")
            ? null
            : { id: serverId, order_index: index };
        })
        .filter((x): x is { id: string; order_index: number } => x !== null);
      if (items.length > 0) {
        await reorderComponents(outcome.id, items);
      }
    },
    onSuccess: async () => {
      setSaveError(null);
      completeOnboarding("save");
      await queryClient.invalidateQueries({ queryKey: ["outcome", outcomeId] });
      if (query.data) {
        await queryClient.invalidateQueries({
          queryKey: ["outcomes", query.data.version_id],
        });
      }
      router.push(versionPath);
    },
    onError: (err: unknown) => {
      setSaveError(toUserError(err, { surface: "save" }));
    },
  });

  const detailErrors = useMemo(
    () => (details ? validateOutcomeDetails(details) : {}),
    [details],
  );

  const dirty = useMemo(() => {
    const outcome = query.data;
    if (!outcome || !details || !components) return false;
    if (
      details.title !== outcome.title ||
      details.description !== (outcome.description ?? "")
    ) {
      return true;
    }
    if (deletedIds.length > 0) return true;
    if (components.some((c) => c.isNew || c.edited)) return true;
    const originalOrder = [...outcome.components]
      .sort((a, b) => a.order_index - b.order_index)
      .map((c) => c.id);
    const currentOrder = components.map((c) => c.id);
    return originalOrder.join("|") !== currentOrder.join("|");
  }, [query.data, details, components, deletedIds]);

  const canSave =
    !!editable &&
    Object.keys(detailErrors).length === 0 &&
    dirty &&
    !saveMutation.isPending;

  // ---- handlers ------------------------------------------------------------

  const handleReorder = useCallback((next: StagedComponent[]) => {
    setComponents(next);
  }, []);

  const handleDelete = useCallback((id: string) => {
    setComponents((prev) => (prev ? prev.filter((c) => c.id !== id) : prev));
    setDeletedIds((prev) => (id.startsWith("temp-") ? prev : [...prev, id]));
  }, []);

  const handleEditClick = useCallback((id: string) => {
    setConfigTarget({ kind: "edit", componentId: id });
  }, []);

  const handleAddType = useCallback((type: ComponentType) => {
    setConfigTarget({ kind: "create", type, placement: "inline" });
  }, []);

  function handleConfigSubmit(payload: ComponentConfigSubmit) {
    if (!configTarget) return;
    if (configTarget.kind === "create") {
      const tempId = makeTempId();
      const now = new Date().toISOString();
      const draft: StagedComponent = {
        id: tempId,
        outcome_id: outcomeId,
        slug: payload.slug,
        type: payload.type,
        config: payload.config,
        placement: payload.placement,
        order_index: components ? components.length : 0,
        created_at: now,
        updated_at: now,
        isNew: true,
        pendingConfig: payload.config,
      };
      setComponents((prev) => (prev ? [...prev, draft] : [draft]));
    } else {
      const { componentId } = configTarget;
      setComponents((prev) =>
        prev
          ? prev.map((c) =>
              c.id === componentId
                ? {
                    ...c,
                    slug: payload.slug,
                    type: payload.type,
                    placement: payload.placement,
                    pendingConfig: payload.config,
                    edited: true,
                  }
                : c,
            )
          : prev,
      );
    }
    setConfigTarget(null);
  }

  function handleCancel() {
    router.push(versionPath);
  }

  // ---- render --------------------------------------------------------------

  if (query.isLoading || versionQuery.isLoading || !details || !components) {
    return (
      <div className="px-6 py-10">
        <div className="mx-auto max-w-3xl space-y-4">
          <div className="h-7 w-48 animate-pulse rounded bg-status-prevBg" />
          <div className="h-40 animate-pulse rounded-lg bg-status-prevBg" />
          <div className="h-64 animate-pulse rounded-lg bg-status-prevBg" />
        </div>
      </div>
    );
  }

  if (query.isError) {
    return (
      <div className="px-6 py-10">
        <div className="mx-auto max-w-3xl">
          <ErrorBanner
            error={toUserError(query.error, { surface: "load" })}
            onRetry={() => query.refetch()}
          />
        </div>
      </div>
    );
  }

  if (!query.data) {
    return null;
  }
  const outcome: OutcomeRead = query.data;
  const editingComponent =
    configTarget?.kind === "edit"
      ? components.find((c) => c.id === configTarget.componentId)
      : undefined;
  const editingType = ComponentType.safeParse(editingComponent?.type);
  const defaultCreateType: ComponentType =
    kind === "json" ? "json_remove" : "html_injection";
  const initialModalType: ComponentType =
    configTarget?.kind === "create"
      ? configTarget.type
      : editingType.success
        ? editingType.data
        : defaultCreateType;

  return (
    <div className="-mx-6 -my-8 flex min-h-[calc(100vh-8rem)] flex-col bg-status-prevBg/20">
      <div className="mx-auto w-full max-w-3xl flex-1 px-6 py-8">
        {/* Title row */}
        <div className="flex items-start justify-between gap-4">
          <div>
            <h1 className="text-2xl font-bold text-nav">
              {readOnly ? "View an Outcome" : "Edit an Outcome"}
            </h1>
            <p className="mt-1 text-sm text-status-prevFg">
              {outcome.is_builtin
                ? "Built-in outcome"
                : "Configure this outcome's components."}
            </p>
          </div>
          <PreviewModal />
        </div>

        {readOnly && (
          <div
            role="status"
            className="mt-4 rounded-lg border border-status-draftBorder bg-status-prevBg/40 px-4 py-3 text-sm text-status-prevFg"
          >
            This version is published and locked. Duplicate it to a new draft to
            edit its outcomes.
          </div>
        )}

        {saveError && (
          <div className="mt-4">
            <ErrorBanner error={saveError} />
          </div>
        )}

        {/* Details */}
        <section className="mt-6 rounded-xl border border-status-prevBg bg-bg-elevated p-6 shadow-sm">
          <h2 className="mb-4 text-sm font-semibold uppercase tracking-wide text-status-prevFg">
            Details
          </h2>
          <OutcomeDetailsForm
            value={details}
            onChange={setDetails}
            disabled={readOnly}
          />
        </section>

        {/* Components */}
        <section className="mt-6 rounded-xl border border-status-prevBg bg-bg-elevated p-6 shadow-sm">
          <h2 className="mb-4 text-sm font-semibold uppercase tracking-wide text-status-prevFg">
            Components
          </h2>

          <SortableComponentList
            components={components}
            onReorder={handleReorder}
            onEdit={handleEditClick}
            onDelete={handleDelete}
            disabled={readOnly}
          />

          <div className="mt-4">
            <AddComponentDrawer
              onPick={handleAddType}
              disabled={readOnly}
              featureType={kind}
            />
          </div>
        </section>
      </div>

      <OutcomeEditorFooter
        dirty={dirty}
        canSave={canSave}
        saving={saveMutation.isPending}
        showSave={!readOnly}
        onCancel={handleCancel}
        onSave={() => saveMutation.mutate()}
      />

      {configTarget && (
        <ComponentConfigModal
          open
          onOpenChange={(open) => {
            if (!open) setConfigTarget(null);
          }}
          mode={configTarget.kind}
          featureType={kind}
          initialSlug={editingComponent?.slug ?? ""}
          initialType={initialModalType}
          initialPlacement={
            configTarget.kind === "create"
              ? configTarget.placement
              : (editingComponent?.placement ?? "inline")
          }
          initialConfig={
            editingComponent
              ? resolvedConfig(editingComponent)
              : configTarget.kind === "create"
                ? defaultConfigFor(configTarget.type)
                : undefined
          }
          onSubmit={handleConfigSubmit}
        />
      )}
    </div>
  );
}
