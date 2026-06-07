"use client";

// One feature's full-journey result (spec items 5.1 + 5.2). Renders, for a
// matched feature:
//   5.1 PER-NODE diffs — one DiffView per consecutive journey step pair
//       (prev.body_after → step.body_after), with the node label + per-node time.
//   5.2 COMBINED start→end — one DiffView of journey[0] → journey[last].
// A skipped feature (matched === false, or no journey) renders a clear
// "Not applicable / skipped" note instead. Reuses the shared git-like DiffView
// so the diffs match the single-canvas Transformation Journey exactly.
import { DiffView } from "@/components/canvas/DiffView";
import { formatBody } from "@/components/full-journey/formatBody";
import type { FullJourneyFeature } from "@/lib/api/fullJourney";

interface FeatureJourneyResultProps {
  feature: FullJourneyFeature;
}

export function FeatureJourneyResult({ feature }: FeatureJourneyResultProps) {
  const isJson = feature.type === "json";
  const { journey } = feature;
  const skipped = !feature.matched || journey.length === 0;

  return (
    <section
      data-testid="feature-result"
      data-feature-id={feature.feature_id}
      aria-label={`Journey for ${feature.name}`}
      className="rounded-lg border border-status-prevBg bg-bg-elevated p-4 shadow-sm"
    >
      <div className="flex flex-wrap items-center justify-between gap-2">
        <div className="min-w-0">
          <h3
            className="truncate text-sm font-semibold text-nav"
            title={feature.name}
          >
            {feature.name}
          </h3>
          <p className="truncate font-mono text-[11px] text-status-prevFg">
            <span className="uppercase tracking-wide">{feature.type}</span>
            {" · "}
            {feature.feature_id}
            {" · order "}
            {feature.execution_order}
            {" · "}
            {feature.version_number !== null
              ? `v${feature.version_number}`
              : "active"}
          </p>
        </div>
        <div className="flex items-center gap-2">
          {skipped ? (
            <span
              data-testid="feature-skipped"
              className="rounded-full bg-status-prevBg px-2.5 py-0.5 text-[11px] font-medium text-status-prevFg"
            >
              Not applicable / skipped
            </span>
          ) : (
            <span className="rounded-full bg-status-liveBg px-2.5 py-0.5 text-[11px] font-medium text-status-liveFg">
              Matched
            </span>
          )}
          <span className="text-[11px] tabular-nums text-status-prevFg">
            {feature.time_took_ms} ms
          </span>
        </div>
      </div>

      {skipped ? (
        <p className="mt-2 text-xs text-status-prevFg">
          This feature didn&apos;t transform the response — its rule path never
          reached an outcome for this request, so there are no node diffs.
        </p>
      ) : (
        <>
          {/* 5.2 Combined start → end for THIS feature. */}
          <div className="mt-3" data-testid="feature-combined-diff">
            <p className="mb-1 text-xs font-medium text-nav">
              Feature start → end
            </p>
            <DiffView
              before={formatBody(journey[0].body_after, isJson)}
              after={formatBody(
                journey[journey.length - 1].body_after,
                isJson,
              )}
              title="Feature start → end"
            />
          </div>

          {/* 5.1 Per-node diffs: each consecutive step pair. */}
          <div className="mt-3">
            <p className="mb-1 text-xs font-medium text-nav">
              Node-by-node changes
            </p>
            <div className="flex flex-col gap-2">
              {journey.slice(1).map((step, i) => {
                const prev = journey[i]; // slice(1) → prev is journey[i]
                return (
                  <div key={step.node_id + ":" + step.index}>
                    <p className="mb-0.5 flex items-center justify-between gap-2 text-[11px] text-status-prevFg">
                      <span className="truncate font-medium text-nav">
                        {prev.label} → {step.label}
                      </span>
                      <span
                        data-testid="node-time"
                        className="shrink-0 tabular-nums"
                      >
                        {step.time_ms} ms
                      </span>
                    </p>
                    <div data-testid="node-diff">
                      <DiffView
                        before={formatBody(prev.body_after, isJson)}
                        after={formatBody(step.body_after, isJson)}
                        title={`${prev.label} → ${step.label}`}
                      />
                    </div>
                  </div>
                );
              })}
            </div>
          </div>

          {/* Per-feature timing summary (expensive expression nodes). */}
          {feature.summary && feature.summary.expressions.length > 0 && (
            <div className="mt-3">
              <p className="mb-1 text-xs font-medium text-nav">
                Expression timings
              </p>
              <dl
                data-testid="feature-summary"
                className="space-y-0.5 rounded-md border border-status-prevBg bg-bg p-2"
              >
                {feature.summary.expressions.map((expr) => (
                  <div
                    key={expr.expression_id}
                    className="flex justify-between gap-2 text-[11px]"
                  >
                    <dt className="truncate text-status-prevFg">
                      {expr.custom_expression_label || expr.expression_label}
                    </dt>
                    <dd className="shrink-0 font-mono tabular-nums text-nav">
                      {expr.expression_time_ms} ms
                    </dd>
                  </div>
                ))}
              </dl>
            </div>
          )}
        </>
      )}
    </section>
  );
}
