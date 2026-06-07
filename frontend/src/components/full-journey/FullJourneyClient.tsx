"use client";

// Test Full Journey (spec item 5). Runs a REAL URL through the proxy's
// full-journey endpoint, which fetches the live upstream and evaluates EVERY
// configured feature (HTML + JSON rules, in execution order) against it. Shows
// three diff levels:
//   5.1 per-node diffs within each feature (FeatureJourneyResult)
//   5.2 each feature's start → end diff (FeatureJourneyResult)
//   5.3 cross-feature: the FIRST matched feature's input → the LAST matched
//       feature's final output (rendered prominently at the top of the results).
//
// Client component: owns the form state (URL, headers, env, version overrides)
// and the eval mutation. Mirrors UrlTestPanel's URL + header validation so we
// never post a header the proxy would silently drop.
import { useState } from "react";
import { useMutation } from "@tanstack/react-query";

import { DiffView } from "@/components/canvas/DiffView";
import {
  HeaderRowsEditor,
  validateHeaderRows,
  type HeaderRow,
} from "@/components/canvas/HeaderRowsEditor";
import { FeatureJourneyResult } from "@/components/full-journey/FeatureJourneyResult";
import { formatBody } from "@/components/full-journey/formatBody";
import { VersionOverrideList } from "@/components/full-journey/VersionOverrideList";
import { ErrorBanner } from "@/components/ui/ErrorBanner";
import { ApiError } from "@/lib/api/client";
import {
  postEvalFullJourney,
  type FullJourneyFeature,
} from "@/lib/api/fullJourney";
import { toUserError } from "@/lib/errors/userError";

type Env = "live" | "staging";

// Cross-feature diff endpoints (5.3): the first matched feature's input body
// vs. the last matched feature's final output body. Each matched feature is
// formatted with ITS OWN content kind. Returns null when no feature matched.
function crossFeatureDiff(
  features: FullJourneyFeature[],
): { before: string; after: string } | null {
  const matched = features.filter(
    (f) => f.matched && f.journey.length > 0,
  );
  if (matched.length === 0) return null;
  const first = matched[0];
  const last = matched[matched.length - 1];
  return {
    before: formatBody(first.journey[0].body_after, first.type === "json"),
    after: formatBody(
      last.journey[last.journey.length - 1].body_after,
      last.type === "json",
    ),
  };
}

export function FullJourneyClient() {
  const [url, setUrl] = useState("");
  const [headerRows, setHeaderRows] = useState<HeaderRow[]>([
    { key: "", value: "" },
  ]);
  const [env, setEnv] = useState<Env>("live");
  const [overrides, setOverrides] = useState<Record<string, number>>({});

  const mutation = useMutation({
    mutationFn: async () => {
      const { headers } = validateHeaderRows(headerRows);
      return postEvalFullJourney({
        url: url.trim(),
        headers,
        env,
        version_overrides: overrides,
      });
    },
  });

  const { valid: headersValid } = validateHeaderRows(headerRows);

  const runDisabled =
    mutation.isPending || url.trim() === "" || !headersValid;

  function handleRun() {
    mutation.mutate();
  }

  const evalError = mutation.error
    ? toUserError(mutation.error, { surface: "eval" })
    : null;
  const evalRawBody =
    mutation.error instanceof ApiError ? mutation.error.rawBody : undefined;

  const result = mutation.data;
  const cross = result ? crossFeatureDiff(result.features) : null;

  return (
    <div className="flex flex-col gap-6">
      <div>
        <h1 className="text-2xl font-bold text-nav">Test Full Journey</h1>
        <p className="mt-1 text-sm text-status-prevFg">
          Run a real URL through the proxy and watch it pass through every
          feature in execution order. See the transformation diff at three
          levels: node by node, each feature&apos;s start → end, and the full
          journey from the first feature&apos;s input to the last
          feature&apos;s output.
        </p>
      </div>

      {/* FORM */}
      <section
        aria-label="Full journey inputs"
        className="rounded-lg border border-status-prevBg bg-bg-elevated p-4 shadow-sm"
      >
        <div className="space-y-3">
          <label className="flex flex-col gap-1 text-xs font-medium text-nav">
            Full URL
            <input
              type="url"
              value={url}
              onChange={(e) => setUrl(e.target.value)}
              placeholder="https://www.example.com/article/123"
              aria-label="Full URL"
              className="rounded-md border border-status-prevBg px-2 py-1.5 text-sm focus:border-brand-500 focus:outline-none"
            />
          </label>

          <fieldset className="flex items-center gap-4">
            <legend className="text-xs font-medium text-nav">
              Environment
            </legend>
            {(["live", "staging"] as const).map((e) => (
              <label
                key={e}
                className="flex items-center gap-1.5 text-sm text-nav"
              >
                <input
                  type="radio"
                  name="fj-env"
                  value={e}
                  checked={env === e}
                  onChange={() => setEnv(e)}
                  aria-label={e === "live" ? "Live" : "Staging"}
                  className="accent-brand-500"
                />
                {e === "live" ? "Live" : "Staging"}
              </label>
            ))}
          </fieldset>

          <div>
            <div className="text-xs font-medium text-nav">Test headers</div>
            <p className="mt-0.5 text-xs text-status-prevFg">
              Added to the upstream request and read by the rules. The site&apos;s
              configured headers always win on a name clash.
            </p>
            <HeaderRowsEditor rows={headerRows} onChange={setHeaderRows} />
          </div>

          <div>
            <div className="text-xs font-medium text-nav">
              Version overrides
            </div>
            <p className="mt-0.5 text-xs text-status-prevFg">
              Each feature runs its active version by default. Pick a specific
              version to test it instead.
            </p>
            <div className="mt-2">
              <VersionOverrideList
                overrides={overrides}
                onChange={setOverrides}
              />
            </div>
          </div>
        </div>

        <div className="mt-4">
          <button
            type="button"
            onClick={handleRun}
            disabled={runDisabled}
            className="rounded-md bg-brand-500 px-4 py-2 text-sm font-semibold text-white hover:bg-brand-600 disabled:cursor-not-allowed disabled:opacity-50"
          >
            {mutation.isPending ? "Running…" : "Run full journey"}
          </button>
        </div>

        {evalError && (
          <div className="mt-3">
            <ErrorBanner
              error={evalError}
              rawResponse={evalRawBody}
              onRetry={evalError.retryable ? handleRun : undefined}
            />
          </div>
        )}
      </section>

      {/* RESULTS */}
      {result && (
        <div className="flex flex-col gap-6">
          {/* Top summary. */}
          <div className="flex flex-wrap items-center gap-x-6 gap-y-1 rounded-lg border border-status-prevBg bg-bg-elevated p-4 text-sm">
            <span className="text-nav">
              <span className="font-medium">Total time:</span>{" "}
              <span data-testid="total-time" className="tabular-nums">
                {result.total_time_ms} ms
              </span>
            </span>
            <span className="text-nav">
              <span className="font-medium">Content:</span> {result.content_kind}
            </span>
            <span className="text-nav">
              <span className="font-medium">Site:</span>{" "}
              {result.site ?? "— (no site matched)"}
            </span>
            <span className="text-nav">
              <span className="font-medium">Features run:</span>{" "}
              {result.features.length}
            </span>
          </div>

          {/* 5.3 Cross-feature diff (prominent). */}
          <section
            aria-label="Full journey diff"
            className="rounded-lg border-2 border-brand-500 bg-bg-elevated p-4 shadow-sm"
          >
            <h2 className="text-base font-semibold text-brand-700">
              Full journey: first feature input → last feature output
            </h2>
            {cross ? (
              <div className="mt-2" data-testid="cross-feature-diff">
                <DiffView
                  before={cross.before}
                  after={cross.after}
                  title="Full journey: first feature input → last feature output"
                />
              </div>
            ) : (
              <p
                data-testid="cross-feature-empty"
                className="mt-2 text-sm text-status-prevFg"
              >
                No feature matched this request, so there&apos;s nothing to diff
                across features. The response would pass through unchanged.
              </p>
            )}
          </section>

          {/* Per-feature results (5.1 + 5.2), in returned execution order. */}
          {result.features.length === 0 ? (
            <p className="text-sm text-status-prevFg">
              No features are configured.
            </p>
          ) : (
            <div className="flex flex-col gap-4">
              {result.features.map((feature) => (
                <FeatureJourneyResult
                  key={feature.feature_id}
                  feature={feature}
                />
              ))}
            </div>
          )}
        </div>
      )}
    </div>
  );
}
