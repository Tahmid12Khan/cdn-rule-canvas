"use client";

// Save toolbar (Task 14 / Phase 2 W1+W2). In edit mode for a DRAFT version it
// shows Save (PATCH rule_graph) + "Saved · {ts}". "Save as New Version" is
// always available and now offers a Draft/Live choice: Draft creates an
// editable draft; Live creates the draft then immediately publishes it to
// production. On a 422 the backend validation details are mapped to per-node
// markers via the store's nodeErrors and surfaced as a UserError.
//
// SAFETY (W1): the inline Save button stays gated behind `isEditing && isDraft`,
// so editing a published version writes NOTHING to the server — "Save as New
// Version" is the only persist path there.
import { useRef, useState } from "react";
import { useRouter } from "next/navigation";
import { useMutation, useQueryClient } from "@tanstack/react-query";

import {
  SaveAsNewVersionDialog,
  type NewVersionStatus,
} from "@/components/canvas/SaveAsNewVersionDialog";
import { Button } from "@/components/ui/Button";
import { ErrorBanner } from "@/components/ui/ErrorBanner";
import { ApiError } from "@/lib/api/client";
import {
  createVersionFromGraph,
  patchRuleGraph,
  type VersionRead,
} from "@/lib/api/canvasVersions";
import type { Applicability } from "@/lib/api/ruleGraph";
import { publishVersion } from "@/lib/api/versions";
import {
  buildClientValidationUserError,
  validateAllCanvases,
} from "@/lib/canvas/graphValidation";
import {
  buildValidationUserError,
  mapValidationErrors,
} from "@/lib/canvas/validationMapping";
import { serializeRuleGraph } from "@/lib/canvas/serialize";
import { toUserError, type UserError } from "@/lib/errors/userError";
import { useOnboardingStore } from "@/state/onboardingStore";
import { useRuleBuilderStore } from "@/state/ruleBuilderStore";

interface SaveBarProps {
  fid: string;
  vnum: number;
  // Path base to navigate after Save as New Version, e.g.
  // /products/features/{type}/{slug}
  featureBase: string;
  // The current version's applicability gate, forwarded to "Save as New
  // Version" so the new draft keeps the same targeting (omitting it would reset
  // the new version to "always apply").
  applicability?: Applicability;
}

function formatSavedAt(ts: number | null): string {
  if (ts == null) return "";
  const seconds = Math.round((Date.now() - ts) / 1000);
  if (seconds < 10) return "just now";
  if (seconds < 60) return `${seconds}s ago`;
  return new Date(ts).toLocaleTimeString();
}

export function SaveBar({
  fid,
  vnum,
  featureBase,
  applicability,
}: SaveBarProps) {
  const router = useRouter();
  const queryClient = useQueryClient();

  const isEditing = useRuleBuilderStore((s) => s.isEditing);
  const versionStatus = useRuleBuilderStore((s) => s.versionStatus);
  // NOTE: do NOT subscribe to s.canvases — it churns identity on every drag
  // frame and would re-render the SaveBar continuously (WS2). The canvas is
  // only needed at mutate time, so read it lazily via getState().
  const lastSavedAt = useRuleBuilderStore((s) => s.lastSavedAt);
  const dirty = useRuleBuilderStore((s) => s.dirty);
  const markSaved = useRuleBuilderStore((s) => s.markSaved);
  const setNodeErrors = useRuleBuilderStore((s) => s.setNodeErrors);
  const clearNodeErrors = useRuleBuilderStore((s) => s.clearNodeErrors);

  const completeOnboarding = useOnboardingStore((s) => s.complete);

  const [error, setError] = useState<UserError | null>(null);
  // Raw server body (ApiError.rawBody) for the ErrorBanner "Show full server
  // response" accordion (req 3). Cleared whenever a new error/success occurs.
  const [rawResponse, setRawResponse] = useState<string | undefined>(undefined);
  const [dialogOpen, setDialogOpen] = useState(false);
  // Holds the description from the dialog's confirm so the async mutationFn
  // (which runs after the synchronous handler) can read it.
  const descriptionRef = useRef("");

  const isDraft = versionStatus === "draft";

  // Client pre-flight validation gate (req 2): run the same cycle / dead-end
  // rules the server enforces BEFORE the round-trip. Returns true when the graph
  // is clean (proceed), false when blocked (markers + banner set, do NOT mutate).
  // The server stays authoritative on 422.
  function preflight(): boolean {
    const canvases = useRuleBuilderStore.getState().canvases;
    const result = validateAllCanvases(canvases);
    if (result.problems.length > 0) {
      setNodeErrors(result.nodeErrors);
      setError(buildClientValidationUserError(result.problems));
      setRawResponse(undefined);
      return false;
    }
    return true;
  }

  function refreshVersionCaches() {
    // The feature query holds live_version_id used by DeploymentStatusRow, so
    // refresh it alongside the versions list.
    void queryClient.invalidateQueries({ queryKey: ["versions", fid] });
    void queryClient.invalidateQueries({ queryKey: ["feature", fid] });
  }

  const save = useMutation({
    mutationFn: () => {
      const rg = serializeRuleGraph(useRuleBuilderStore.getState().canvases);
      return patchRuleGraph(fid, vnum, rg).then((res) => ({ res, rg }));
    },
    onSuccess: ({ rg }) => {
      clearNodeErrors();
      setError(null);
      setRawResponse(undefined);
      markSaved(rg);
      completeOnboarding("save");
      void queryClient.invalidateQueries({ queryKey: ["version", fid, vnum] });
    },
    onError: (err) => {
      setRawResponse(err instanceof ApiError ? err.rawBody : undefined);
      if (err instanceof ApiError && err.isValidation) {
        const canvases = useRuleBuilderStore.getState().canvases;
        const sent = serializeRuleGraph(canvases);
        setNodeErrors(mapValidationErrors(err.details, sent));
        setError(buildValidationUserError(err.details, sent, canvases));
      } else {
        setError(toUserError(err, { surface: "save" }));
      }
    },
  });

  const saveAsNew = useMutation({
    mutationFn: async (status: NewVersionStatus) => {
      const rg = serializeRuleGraph(useRuleBuilderStore.getState().canvases);
      const created = await createVersionFromGraph(
        fid,
        descriptionRef.current,
        rg,
        applicability,
      );
      if (status === "live") {
        try {
          await publishVersion(fid, created.version_number, {
            environment: "live",
          });
        } catch (publishErr) {
          // Don't lose the created draft — surface a partial-failure marker so
          // onError can navigate to the new draft and explain what happened.
          throw new PublishAfterCreateError(created, publishErr);
        }
      }
      return created;
    },
    onSuccess: (created: VersionRead) => {
      setDialogOpen(false);
      clearNodeErrors();
      setError(null);
      setRawResponse(undefined);
      markSaved(serializeRuleGraph(useRuleBuilderStore.getState().canvases));
      refreshVersionCaches();
      router.push(`${featureBase}/${created.version_number}`);
    },
    onError: (err) => {
      setRawResponse(err instanceof ApiError ? err.rawBody : undefined);
      if (err instanceof PublishAfterCreateError) {
        // The draft exists — navigate to it and explain the publish failure.
        setDialogOpen(false);
        markSaved(serializeRuleGraph(useRuleBuilderStore.getState().canvases));
        refreshVersionCaches();
        setError({
          title: "Version created as draft, but publishing to live failed",
          why: "The new version was saved as a draft, but the publish-to-live step didn't complete.",
          howToFix:
            "Use 'Make Live' on the new version to retry publishing it to production.",
          retryable: false,
        });
        router.push(`${featureBase}/${err.created.version_number}`);
        return;
      }
      // A genuine validation failure (e.g. invalid graph) — map it to per-node
      // markers AND a descriptive banner, same as the inline Save path. Close
      // the dialog so the banner + highlighted nodes are visible and the user
      // can fix them, then re-open "Save as New Version" to retry.
      if (err instanceof ApiError && err.isValidation) {
        setDialogOpen(false);
        const canvases = useRuleBuilderStore.getState().canvases;
        const sentGraph = serializeRuleGraph(canvases);
        setNodeErrors(mapValidationErrors(err.details, sentGraph));
        setError(buildValidationUserError(err.details, sentGraph, canvases));
        return;
      }
      setError(toUserError(err, { surface: "create" }));
    },
  });

  return (
    <div className="flex flex-col items-end gap-3">
      {error && (
        <div className="w-full">
          <ErrorBanner error={error} rawResponse={rawResponse} />
        </div>
      )}

      <div className="flex flex-wrap items-center justify-end gap-3">
        {/* Drive off lastSavedAt presence (set by markSaved) + clean state —
            NOT save.isSuccess, which lingers after an edit+undo-to-baseline and
            would re-show "Saved" for a save that didn't happen (LOW-8). */}
        {!dirty && lastSavedAt && (
          <span className="font-mono text-xs text-fg-muted">
            Saved · {formatSavedAt(lastSavedAt)}
          </span>
        )}

        {isEditing && isDraft && (
          <Button
            variant="primary"
            onClick={() => {
              if (!preflight()) return;
              save.mutate();
            }}
            disabled={save.isPending || !dirty}
          >
            {save.isPending ? "Saving…" : "Save"}
          </Button>
        )}

        <Button variant="secondary" onClick={() => setDialogOpen(true)}>
          Save as New Version
        </Button>
      </div>

      <SaveAsNewVersionDialog
        open={dialogOpen}
        onOpenChange={setDialogOpen}
        saving={saveAsNew.isPending}
        onConfirm={(desc, status) => {
          // Pre-flight before creating a new version too: close the dialog so the
          // banner + node markers are visible if the graph is invalid (mirrors
          // the 422 handling), then bail without mutating.
          if (!preflight()) {
            setDialogOpen(false);
            return;
          }
          descriptionRef.current = desc;
          saveAsNew.mutate(status);
        }}
      />
    </div>
  );
}

// Internal marker error: create succeeded but the subsequent publish failed.
// Carries the already-created draft so the UI can navigate to it.
class PublishAfterCreateError extends Error {
  constructor(
    public created: VersionRead,
    public cause: unknown,
  ) {
    super("publish-after-create failed");
    this.name = "PublishAfterCreateError";
  }
}
