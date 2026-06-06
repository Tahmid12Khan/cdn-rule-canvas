"use client";

// Rule Builder / Version detail client shell (Tasks 11–14). Seeds the
// ruleBuilderStore from the version's rule_graph once per (fid, vnum), then
// composes the header, description, metadata, palette, canvas, config drawer,
// save bar and outcome list. React Flow is lazy-loaded (ssr:false).
import { useEffect, useMemo, useRef, useState } from "react";
import dynamic from "next/dynamic";
import * as Tooltip from "@radix-ui/react-tooltip";
import { useQuery } from "@tanstack/react-query";

import { CanvasSlider } from "@/components/canvas/CanvasSlider";
import { CompareDialog } from "@/components/canvas/compare/CompareDialog";
import { NodeConfigDrawer } from "@/components/canvas/config/NodeConfigDrawer";
import { NodePalette } from "@/components/canvas/palette/NodePalette";
import { ReadOnlyBanner } from "@/components/canvas/ReadOnlyBanner";
import { SaveBar } from "@/components/canvas/SaveBar";
import { TestingPanel } from "@/components/canvas/TestingPanel";
import { UnsavedChangesGuard } from "@/components/canvas/UnsavedChangesGuard";
import { ApplicabilityForm } from "@/components/version/ApplicabilityForm";
import { DescriptionEditable } from "@/components/version/DescriptionEditable";
import { LastUpdatedCard } from "@/components/version/LastUpdatedCard";
import { OutcomeListSection } from "@/components/version/OutcomeListSection";
import { VersionHeader } from "@/components/version/VersionHeader";
import { MakeLiveDialog } from "@/components/versions/MakeLiveDialog";
import {
  listCanvasOutcomes,
  type CanvasOutcome,
} from "@/lib/api/canvasOutcomes";
import { getVersion, type VersionRead } from "@/lib/api/canvasVersions";
import type { OutcomeOption } from "@/lib/canvas/nodeTemplates";
import { useOnboardingStore } from "@/state/onboardingStore";
import { useRuleBuilderStore } from "@/state/ruleBuilderStore";

// React Flow is browser-only — load with ssr:false (FRONTEND CONTRACT §8).
const RuleBuilderCanvas = dynamic(
  () => import("@/components/canvas/RuleBuilderCanvas"),
  {
    ssr: false,
    loading: () => (
      <div className="flex h-[520px] w-full items-center justify-center rounded-lg border border-status-prevBg bg-bg-elevated text-sm text-status-prev">
        Loading canvas…
      </div>
    ),
  },
);

interface RuleBuilderClientProps {
  fid: string;
  vnum: number;
  type: string;
  initialVersion: VersionRead;
}

export function RuleBuilderClient({
  fid,
  vnum,
  type,
  initialVersion,
}: RuleBuilderClientProps) {
  const { data: version } = useQuery({
    queryKey: ["version", fid, vnum],
    queryFn: () => getVersion(fid, vnum),
    initialData: initialVersion,
  });

  const versionId = version.id;

  const { data: outcomes = [], isFetched: outcomesFetched } = useQuery<
    CanvasOutcome[]
  >({
    queryKey: ["outcomes", versionId],
    queryFn: () => listCanvasOutcomes(versionId),
  });

  const seedFromRuleGraph = useRuleBuilderStore((s) => s.seedFromRuleGraph);
  const selected = useRuleBuilderStore((s) => s.selected);
  const setSelected = useRuleBuilderStore((s) => s.setSelected);
  const isEditing = useRuleBuilderStore((s) => s.isEditing);
  const toggleEdit = useRuleBuilderStore((s) => s.toggleEdit);
  const selectedCanvasEmpty = useRuleBuilderStore(
    (s) => s.canvases[s.selected].nodes.length === 0,
  );

  // Onboarding: tick the "edit" step the first time the user enters edit mode.
  const completeOnboarding = useOnboardingStore((s) => s.complete);
  useEffect(() => {
    if (isEditing) completeOnboarding("edit");
  }, [isEditing, completeOnboarding]);

  const isDraft = version.status === "draft";
  const canMakeLive = version.status !== "live";
  const [makeLiveOpen, setMakeLiveOpen] = useState(false);
  const [compareOpen, setCompareOpen] = useState(false);

  // Narrow the decorative route param to the feature content kind used by the
  // palette filter, TestPanel inputs, applicability gate and component forms.
  const featureType: "html" | "json" = type === "json" ? "json" : "html";

  // Title resolver for outcome nodes (denormalized display cache).
  const outcomeTitleById = useMemo(() => {
    const map = new Map(outcomes.map((o) => [o.id, o.title]));
    return (id: string) => map.get(id) ?? "Outcome";
    // Recompute when outcomes change so deserialize gets fresh titles.
  }, [outcomes]);

  // Seed the store once per (fid, vnum), but only after the outcomes query has
  // settled so outcome-node titles resolve from the cache (not "Outcome").
  // The ref guards against re-seeding on re-render (which would clobber edits).
  const seededKey = useRef<string | null>(null);
  useEffect(() => {
    if (!outcomesFetched) return;
    const key = `${fid}:${vnum}`;
    if (seededKey.current === key) return;
    seededKey.current = key;
    seedFromRuleGraph(version.rule_graph, version.status, outcomeTitleById);
    // Intentionally gate on identity key + outcomes-settled; seed inputs are
    // stable for a given (fid, vnum).
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [fid, vnum, outcomesFetched]);

  const paletteOutcomes: OutcomeOption[] = outcomes.map((o) => ({
    id: o.id,
    title: o.title,
  }));

  const featureBase = `/products/features/${type}/${fid}`;
  const routeBase = `${featureBase}/${vnum}`;

  return (
    <div className="space-y-6 px-6 py-6">
      <UnsavedChangesGuard />

      <MakeLiveDialog
        featureId={fid}
        versionNumber={vnum}
        open={makeLiveOpen}
        onOpenChange={setMakeLiveOpen}
      />

      <CompareDialog
        fid={fid}
        currentVersion={version}
        open={compareOpen}
        onOpenChange={setCompareOpen}
      />

      <header className="space-y-3">
        <VersionHeader
          versionNumber={version.version_number}
          status={version.status}
          isLive={version.status === "live"}
          isStaging={version.status === "staging"}
        />
        <DescriptionEditable
          fid={fid}
          vnum={vnum}
          description={version.description}
          editable={isDraft}
        />
        <LastUpdatedCard
          by={version.last_updated_by}
          at={version.last_updated_at}
        />
      </header>

      <section className="space-y-4">
        <div className="flex flex-wrap items-center justify-between gap-3">
          <h2 className="text-lg font-semibold text-nav">Rules Builder</h2>
          <div className="flex items-center gap-2">
            <button
              type="button"
              onClick={toggleEdit}
              className={[
                "rounded-md px-3 py-1.5 text-sm font-semibold",
                isEditing
                  ? "bg-brand-500 text-white"
                  : "border border-action-600 text-action-600 hover:bg-action-600 hover:text-white",
              ].join(" ")}
            >
              {isEditing ? "Editing" : "Edit"}
            </button>
            <button
              type="button"
              onClick={() => setCompareOpen(true)}
              className="rounded-md border border-border px-3 py-1.5 text-sm font-medium text-nav hover:bg-brand-50 hover:text-brand-700"
            >
              Compare
            </button>
            {canMakeLive && (
              <button
                type="button"
                onClick={() => setMakeLiveOpen(true)}
                className="rounded-md bg-status-live px-3 py-1.5 text-sm font-semibold text-white hover:opacity-90"
              >
                Make Live
              </button>
            )}
            <Tooltip.Provider delayDuration={200}>
              <Tooltip.Root>
                <Tooltip.Trigger asChild>
                  <button
                    type="button"
                    disabled
                    aria-disabled="true"
                    className="rounded-md border border-status-prevBg px-3 py-1.5 text-sm font-medium text-status-prev opacity-60"
                  >
                    Analytics
                  </button>
                </Tooltip.Trigger>
                <Tooltip.Portal>
                  <Tooltip.Content
                    side="bottom"
                    className="rounded bg-nav px-2 py-1 text-xs text-nav-fg shadow-md"
                  >
                    Coming soon
                    <Tooltip.Arrow className="fill-nav" />
                  </Tooltip.Content>
                </Tooltip.Portal>
              </Tooltip.Root>
            </Tooltip.Provider>
          </div>
        </div>

        {!isDraft &&
          (isEditing ? (
            <ReadOnlyBanner variant="local" />
          ) : (
            <ReadOnlyBanner variant="readonly" />
          ))}

        <ApplicabilityForm
          fid={fid}
          vnum={vnum}
          applicability={version.applicability}
          featureType={featureType}
          editable={isDraft}
        />

        <CanvasSlider selected={selected} onSelect={setSelected} />

        {isEditing && selectedCanvasEmpty && (
          <p className="rounded-lg border border-dashed border-brand-400 bg-brand-50 px-4 py-2 text-sm text-accent-onMuted">
            Step 4 of 5:{" "}
            {featureType === "json"
              ? "Drag a JSON Expression chip (and an Outcome)"
              : "Drag a Meta Tags or Device Type chip (and an Outcome)"}{" "}
            from the palette onto the canvas to build a rule.
          </p>
        )}

        {isEditing && (
          <NodePalette
            outcomes={paletteOutcomes}
            draggable={isEditing}
            featureType={featureType}
          />
        )}

        <RuleBuilderCanvas canvasKey={selected} editable={isEditing} />

        <NodeConfigDrawer canvasKey={selected} outcomes={paletteOutcomes} />

        <TestingPanel
          outcomeTitleById={outcomeTitleById}
          featureType={featureType}
        />

        <SaveBar
          fid={fid}
          vnum={vnum}
          featureBase={featureBase}
          applicability={version.applicability}
        />
      </section>

      <OutcomeListSection
        versionId={versionId}
        routeBase={routeBase}
        editable={isDraft}
      />
    </div>
  );
}
