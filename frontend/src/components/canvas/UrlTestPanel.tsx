"use client";

// UrlTestPanel. A second test panel that runs the currently-selected canvas
// against a REAL upstream response: the user enters a full URL, the proxy
// resolves it to a configured Site, fetches the live page (applying the Site's
// configured headers), and evaluates the LIVE (possibly unsaved) editor canvas
// against it — highlighting the path + showing the Transformation Journey
// exactly like TestPanel.
//
// Extra "test headers" are forwarded to the upstream and fed to the evaluator,
// but a Site's configured header always WINS on a name collision (a test header
// may not override a site default — the proxy enforces this).
import { useState } from "react";
import { useMutation } from "@tanstack/react-query";

import {
  useEvalHighlight,
  useMatchedOutcome,
} from "@/components/canvas/evalHighlight";
import {
  HeaderRowsEditor,
  validateHeaderRows,
  type HeaderRow,
} from "@/components/canvas/HeaderRowsEditor";
import { TestPresetBar } from "@/components/canvas/TestPresetBar";
import { TransformationJourney } from "@/components/canvas/TransformationJourney";
import { ErrorBanner } from "@/components/ui/ErrorBanner";
import { ApiError } from "@/lib/api/client";
import { postEvalUrlTest } from "@/lib/api/evalTest";
import { serializeCanvas } from "@/lib/canvas/serialize";
import { toUserError } from "@/lib/errors/userError";
import { useRuleBuilderStore } from "@/state/ruleBuilderStore";

interface UrlTestPanelProps {
  // Title resolver so the matched-outcome banner can show a friendly name.
  outcomeTitleById: (id: string) => string;
  // Feature content kind — formats the Transformation Journey body diffs.
  featureType: "html" | "json";
}

// Turn a saved header map back into editable rows (always keep one blank row so
// the editor never renders empty).
function rowsFromHeaders(headers: Record<string, string>): HeaderRow[] {
  const rows = Object.entries(headers).map(([key, value]) => ({ key, value }));
  return rows.length > 0 ? rows : [{ key: "", value: "" }];
}

export function UrlTestPanel({
  outcomeTitleById,
  featureType,
}: UrlTestPanelProps) {
  const highlight = useRuleBuilderStore((s) => s.testHighlight);
  // Live canvas nodes — passed to the journey so each step can look up its
  // node's config (inputs + plain-English description).
  const canvasNodes = useRuleBuilderStore((s) => s.canvas.nodes);

  // Per-run highlight lifecycle, shared with TestPanel so both highlight
  // identically.
  const { applyResult, restoreFullPath, clear } = useEvalHighlight();

  const [url, setUrl] = useState("");
  const [headerRows, setHeaderRows] = useState<HeaderRow[]>([
    { key: "", value: "" },
  ]);

  const mutation = useMutation({
    mutationFn: async () => {
      const c = useRuleBuilderStore.getState().canvas;
      const canvas = serializeCanvas(c.nodes, c.edges, c.rootNodeId);
      // Collapse the rows into a header map (drop blank names; last write wins).
      const { headers } = validateHeaderRows(headerRows);
      return postEvalUrlTest({ canvas, url: url.trim(), headers });
    },
    onSuccess: (res) => {
      const { canvas } = useRuleBuilderStore.getState();
      applyResult(res, canvas.edges);
    },
  });

  // Result-banner facts (reached-END + applied-outcome name), shared with
  // TestPanel.
  const { matchedTitle, reachedEnd } = useMatchedOutcome(
    mutation.data,
    outcomeTitleById,
  );

  function handleClear() {
    clear();
    mutation.reset();
  }

  function handleRun() {
    mutation.mutate();
  }

  const evalError = mutation.error
    ? toUserError(mutation.error, { surface: "eval" })
    : null;
  const evalRawBody =
    mutation.error instanceof ApiError ? mutation.error.rawBody : undefined;

  // Per-row header validation (same rules as Site headers / the backend). A blank
  // name row is ignored; any non-blank row that fails the HTTP-token name rule or
  // the visible-ASCII value rule blocks Run so we never post a header the proxy
  // would silently drop.
  const { headers: headerMap, valid: headersValid } =
    validateHeaderRows(headerRows);

  const runDisabled =
    mutation.isPending || url.trim() === "" || !headersValid;

  // Test-preset payload (url kind): the URL + the collapsed header map. Loading
  // applies the saved url + headers back into the panel state.
  const presetPayload = { url, headers: headerMap };
  const presetValid = url.trim() !== "" && headersValid;
  function handleLoadPreset(payload: unknown) {
    const p = (payload ?? {}) as { url?: unknown; headers?: unknown };
    if (typeof p.url === "string") setUrl(p.url);
    if (p.headers && typeof p.headers === "object") {
      setHeaderRows(rowsFromHeaders(p.headers as Record<string, string>));
    }
  }

  return (
    <section
      aria-label="Test with a live URL"
      className="rounded-lg border border-status-prevBg bg-bg-elevated p-4 shadow-sm"
    >
      <div className="flex items-center justify-between">
        <h3 className="text-sm font-semibold text-nav">Test with a live URL</h3>
        {(highlight || mutation.data) && (
          <button
            type="button"
            onClick={handleClear}
            className="text-xs font-medium text-status-prevFg hover:text-brand-600"
          >
            Clear highlight
          </button>
        )}
      </div>
      <p className="mt-1 text-xs text-status-prevFg">
        Enter a full URL to fetch the real page through the proxy and run the
        canvas against the live response.
      </p>

      <div className="mt-3">
        <TestPresetBar
          kind="url"
          currentPayload={presetPayload}
          payloadValid={presetValid}
          onLoad={handleLoadPreset}
        />
      </div>

      <div className="mt-3 space-y-3">
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

        <div>
          <div className="text-xs font-medium text-nav">Test headers</div>
          <p className="mt-0.5 text-xs text-status-prevFg">
            Added to the upstream request and read by the rule. The site&apos;s
            configured headers always win on a name clash.
          </p>
          <HeaderRowsEditor rows={headerRows} onChange={setHeaderRows} />
        </div>
      </div>

      <div className="mt-4 flex items-center gap-3">
        <button
          type="button"
          onClick={handleRun}
          disabled={runDisabled}
          className="rounded-md bg-brand-500 px-4 py-2 text-sm font-semibold text-white hover:bg-brand-600 disabled:cursor-not-allowed disabled:opacity-50"
        >
          {mutation.isPending ? "Running…" : "Run test"}
        </button>

        {mutation.data &&
          (matchedTitle ? (
            <p className="text-sm text-status-prevFg">
              Reached END · applied outcome:{" "}
              <span className="font-semibold text-brand-700">
                {matchedTitle}
              </span>
            </p>
          ) : reachedEnd ? (
            <p className="text-sm text-status-prevFg">
              Reached <span className="font-semibold text-brand-700">END</span>{" "}
              — the final body is the last step of the journey below.
            </p>
          ) : (
            <p className="text-sm font-medium text-status-stagingFg">
              Path didn&apos;t reach an END node (fail-open). Only the partial
              path is shown.
            </p>
          ))}
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

      {mutation.data && mutation.data.journey.length > 0 && (
        <TransformationJourney
          // Remount on each run so the stepper resets to the first node.
          key={mutation.submittedAt}
          journey={mutation.data.journey}
          featureType={featureType}
          canvasNodes={canvasNodes}
          onRestoreFullPath={restoreFullPath}
        />
      )}
    </section>
  );
}
