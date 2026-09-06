"use client";

// TestPanel (WS4). A small side panel under the canvas that lets the user enter
// a synthetic request context (device + meta tags + optional path, plus an
// Advanced section for request headers, User-Agent, a matched Site, and a raw
// response body) for the currently-selected canvas, run it through the proxy's
// real JDM evaluator, and highlight the traversed node/edge path on the canvas.
//
// The graph it tests is the LIVE (possibly unsaved) canvas from the store, so
// edits can be tested before saving. Highlight state is stored on the
// ruleBuilderStore so the custom nodes/edges can subscribe and glow.
import { useState } from "react";
import { useMutation } from "@tanstack/react-query";

import { SiteSelectControl } from "@/components/canvas/config/SiteSelectControl";
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
import {
  postEvalTest,
  type DeviceType,
  type EvalContext,
} from "@/lib/api/evalTest";
import { serializeCanvas } from "@/lib/canvas/serialize";
import { toUserError } from "@/lib/errors/userError";
import { useRuleBuilderStore } from "@/state/ruleBuilderStore";

interface TestPanelProps {
  // Title resolver so the matched-outcome banner can show a friendly name.
  outcomeTitleById: (id: string) => string;
  // Feature content kind (route param). JSON features test against a parsed
  // response body (context.response_json); HTML features keep device/meta/path.
  featureType: "html" | "json";
}

interface MetaRow {
  key: string;
  value: string;
}

const DEVICES: DeviceType[] = ["mobile", "desktop", "tablet"];

// Turn a saved header map back into editable rows (always keep one blank row).
function rowsFromHeaders(headers: Record<string, string>): HeaderRow[] {
  const rows = Object.entries(headers).map(([key, value]) => ({ key, value }));
  return rows.length > 0 ? rows : [{ key: "", value: "" }];
}

function metaRowsFromMap(meta: Record<string, string>): MetaRow[] {
  const rows = Object.entries(meta).map(([key, value]) => ({ key, value }));
  return rows.length > 0 ? rows : [{ key: "", value: "" }];
}

export function TestPanel({ outcomeTitleById, featureType }: TestPanelProps) {
  const isJson = featureType === "json";
  const highlight = useRuleBuilderStore((s) => s.testHighlight);
  // Live canvas nodes — passed to the journey so each step can look up its
  // node's config (inputs + plain-English description).
  const canvasNodes = useRuleBuilderStore((s) => s.canvas.nodes);

  // Per-run highlight lifecycle (apply on success / restore on journey collapse /
  // clear), shared with UrlTestPanel so both highlight identically.
  const { applyResult, restoreFullPath, clear } = useEvalHighlight();

  const [device, setDevice] = useState<DeviceType | "">("");
  const [path, setPath] = useState("");
  const [metaRows, setMetaRows] = useState<MetaRow[]>([{ key: "", value: "" }]);
  // JSON features: a textarea holding the synthetic response body. Parsed to a
  // value on Run; a parse error is shown inline and blocks the request.
  const [jsonBody, setJsonBody] = useState("");
  const [jsonParseError, setJsonParseError] = useState<string | null>(null);

  // Advanced section: collapsed by default so the panel isn't overwhelming.
  const [advancedOpen, setAdvancedOpen] = useState(false);
  const [userAgent, setUserAgent] = useState("");
  const [site, setSite] = useState("");
  const [headerRows, setHeaderRows] = useState<HeaderRow[]>([
    { key: "", value: "" },
  ]);
  // Raw response body (content_kind aware). For json this is the JSON body;
  // for html it's a raw HTML string passed straight to the evaluator.
  const [rawBody, setRawBody] = useState("");

  // Introspect the canvas's decision processors so we can hint which inputs
  // actually matter (meta_tags vs device_type).
  const usesDevice = useRuleBuilderStore((s) =>
    s.canvas.nodes.some(
      (n) =>
        n.type === "decisionNode" && n.data.processor.type === "device_type",
    ),
  );
  const usesMetaTags = useRuleBuilderStore((s) =>
    s.canvas.nodes.some(
      (n) => n.type === "decisionNode" && n.data.processor.type === "meta_tags",
    ),
  );

  const { headers: headerMap, valid: headersValid } =
    validateHeaderRows(headerRows);

  const mutation = useMutation({
    mutationFn: async () => {
      const c = useRuleBuilderStore.getState().canvas;
      const canvas = serializeCanvas(c.nodes, c.edges, c.rootNodeId);

      let context: EvalContext;
      if (isJson) {
        // PREFERRED for JSON features: send the parsed object as response_json
        // so json_expression nodes evaluate. content_kind tags the request.
        // Guard the parse (mirrors handleRun's pre-flight) so a bad body lands
        // in a clean error state instead of surfacing a raw SyntaxError.
        let parsed: unknown = {};
        if (jsonBody.trim()) {
          try {
            parsed = JSON.parse(jsonBody);
          } catch {
            setJsonParseError(
              "That isn't valid JSON. Fix the response body and try again.",
            );
            throw new Error("Invalid JSON response body");
          }
        }
        context = {
          content_kind: "json",
          response_json: parsed,
          ...(path.trim() ? { path: path.trim() } : {}),
        };
      } else {
        const meta_tags = metaRows.reduce<Record<string, string>>(
          (acc, row) => {
            const k = row.key.trim();
            if (k) acc[k] = row.value;
            return acc;
          },
          {},
        );
        context = {
          ...(device ? { device_type: device } : {}),
          ...(Object.keys(meta_tags).length > 0 ? { meta_tags } : {}),
          ...(path.trim() ? { path: path.trim() } : {}),
        };
      }

      // Advanced inputs (apply to both content kinds). Only attach defined ones.
      if (userAgent.trim()) context.user_agent = userAgent.trim();
      if (site) context.site = site;
      if (Object.keys(headerMap).length > 0) context.headers = headerMap;
      if (rawBody.trim()) {
        context.response_body = rawBody;
        context.content_kind = isJson ? "json" : "html";
      }

      return postEvalTest({ canvas, context });
    },
    onSuccess: (res) => {
      // Highlight the FULL start→END path for this run (the hook remembers it so
      // stepping the journey can restore it). A path is a dead-end only when it
      // never reaches an END node — the proxy appends the END step when it does.
      const { canvas } = useRuleBuilderStore.getState();
      applyResult(res, canvas.edges);
    },
  });

  // Result-banner facts (reached-END + applied-outcome name), shared with
  // UrlTestPanel so both panels report the result identically.
  const { matchedTitle, reachedEnd } = useMatchedOutcome(
    mutation.data,
    outcomeTitleById,
  );

  function updateRow(index: number, patch: Partial<MetaRow>) {
    setMetaRows((rows) =>
      rows.map((r, i) => (i === index ? { ...r, ...patch } : r)),
    );
  }
  function addRow() {
    setMetaRows((rows) => [...rows, { key: "", value: "" }]);
  }
  function removeRow(index: number) {
    setMetaRows((rows) =>
      rows.length === 1 ? rows : rows.filter((_, i) => i !== index),
    );
  }

  function handleClear() {
    clear();
    mutation.reset();
  }

  function handleRun() {
    // For JSON features, validate the textarea is parseable BEFORE the request
    // (show the parse error inline). Empty = an empty object (always-no for most
    // json_expression operators), which is a valid test input.
    if (isJson && jsonBody.trim()) {
      try {
        JSON.parse(jsonBody);
      } catch {
        setJsonParseError(
          "That isn't valid JSON. Fix the response body and try again.",
        );
        return;
      }
    }
    setJsonParseError(null);
    mutation.mutate();
  }

  // Test-preset payload (rule kind): only defined fields are included so a
  // loaded preset round-trips cleanly. Blocked from saving when headers are
  // invalid.
  const presetPayload: Record<string, unknown> = { feature_type: featureType };
  if (device) presetPayload.device_type = device;
  if (userAgent.trim()) presetPayload.user_agent = userAgent.trim();
  if (path.trim()) presetPayload.path = path.trim();
  if (!isJson) {
    const meta = metaRows.reduce<Record<string, string>>((acc, row) => {
      const k = row.key.trim();
      if (k) acc[k] = row.value;
      return acc;
    }, {});
    if (Object.keys(meta).length > 0) presetPayload.meta_tags = meta;
  }
  if (Object.keys(headerMap).length > 0) presetPayload.headers = headerMap;
  if (isJson && jsonBody.trim()) {
    presetPayload.response_body = jsonBody;
    presetPayload.content_kind = "json";
  } else if (rawBody.trim()) {
    presetPayload.response_body = rawBody;
    presetPayload.content_kind = isJson ? "json" : "html";
  }
  if (site) presetPayload.site = site;

  function handleLoadPreset(payload: unknown) {
    const p = (payload ?? {}) as Record<string, unknown>;
    setDevice(
      typeof p.device_type === "string"
        ? (p.device_type as DeviceType)
        : "",
    );
    setUserAgent(typeof p.user_agent === "string" ? p.user_agent : "");
    setPath(typeof p.path === "string" ? p.path : "");
    setSite(typeof p.site === "string" ? p.site : "");
    setMetaRows(
      p.meta_tags && typeof p.meta_tags === "object"
        ? metaRowsFromMap(p.meta_tags as Record<string, string>)
        : [{ key: "", value: "" }],
    );
    setHeaderRows(
      p.headers && typeof p.headers === "object"
        ? rowsFromHeaders(p.headers as Record<string, string>)
        : [{ key: "", value: "" }],
    );
    const body = typeof p.response_body === "string" ? p.response_body : "";
    if (isJson) {
      setJsonBody(body);
      setRawBody("");
    } else {
      setRawBody(body);
      setJsonBody("");
    }
    // Open Advanced if the preset uses any advanced field so the user sees it.
    if (p.user_agent || p.site || p.headers || (p.response_body && !isJson)) {
      setAdvancedOpen(true);
    }
  }

  const evalError = mutation.error
    ? toUserError(mutation.error, { surface: "eval" })
    : null;
  const evalRawBody =
    mutation.error instanceof ApiError ? mutation.error.rawBody : undefined;

  const inputClass =
    "rounded-md border border-status-prevBg px-2 py-1.5 text-sm focus:border-brand-500 focus:outline-none";

  return (
    <section
      aria-label="Test a rule"
      className="rounded-lg border border-status-prevBg bg-bg-elevated p-4 shadow-sm"
    >
      <div className="flex items-center justify-between">
        <h3 className="text-sm font-semibold text-nav">Test a rule</h3>
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
        Enter a {isJson ? "response body" : "request context"} and run it
        through the evaluator to highlight the path.
      </p>

      <div className="mt-3">
        <TestPresetBar
          kind="rule"
          currentPayload={presetPayload}
          payloadValid={headersValid}
          onLoad={handleLoadPreset}
        />
      </div>

      {isJson ? (
        <div className="mt-3 space-y-3">
          <label className="flex flex-col gap-1 text-xs font-medium text-nav">
            Response JSON
            <textarea
              value={jsonBody}
              onChange={(e) => {
                setJsonBody(e.target.value);
                if (jsonParseError) setJsonParseError(null);
              }}
              placeholder={'{\n  "type": "premium"\n}'}
              rows={6}
              aria-label="Response JSON body"
              aria-invalid={Boolean(jsonParseError) || undefined}
              className="rounded-md border border-status-prevBg px-2 py-1.5 font-mono text-xs focus:border-brand-500 focus:outline-none"
            />
          </label>
          {jsonParseError && (
            <p role="alert" className="text-xs font-medium text-danger">
              {jsonParseError}
            </p>
          )}
          <label className="flex flex-col gap-1 text-xs font-medium text-nav">
            Path (optional)
            <input
              type="text"
              value={path}
              onChange={(e) => setPath(e.target.value)}
              placeholder="/api/article/123"
              className="rounded-md border border-status-prevBg px-2 py-1.5 text-sm focus:border-brand-500 focus:outline-none"
            />
          </label>
        </div>
      ) : (
        <>
          <div className="mt-3 grid gap-3 sm:grid-cols-2">
            <label className="flex flex-col gap-1 text-xs font-medium text-nav">
              Device type
              {usesDevice && (
                <span className="font-normal text-status-prev">
                  (used here)
                </span>
              )}
              <select
                value={device}
                onChange={(e) => setDevice(e.target.value as DeviceType | "")}
                className="rounded-md border border-status-prevBg px-2 py-1.5 text-sm focus:border-brand-500 focus:outline-none"
              >
                <option value="">— none —</option>
                {DEVICES.map((d) => (
                  <option key={d} value={d}>
                    {d}
                  </option>
                ))}
              </select>
            </label>

            <label className="flex flex-col gap-1 text-xs font-medium text-nav">
              Path (optional)
              <input
                type="text"
                value={path}
                onChange={(e) => setPath(e.target.value)}
                placeholder="/article/123"
                className="rounded-md border border-status-prevBg px-2 py-1.5 text-sm focus:border-brand-500 focus:outline-none"
              />
            </label>
          </div>

          <div className="mt-3">
            <div className="flex items-center gap-2 text-xs font-medium text-nav">
              Meta tags
              {usesMetaTags && (
                <span className="font-normal text-status-prev">
                  (used here)
                </span>
              )}
            </div>
            <div className="mt-1 flex flex-col gap-2">
              {metaRows.map((row, i) => (
                <div key={i} className="flex items-center gap-2">
                  <input
                    type="text"
                    value={row.key}
                    onChange={(e) => updateRow(i, { key: e.target.value })}
                    placeholder="name"
                    aria-label={`Meta tag name ${i + 1}`}
                    className="w-1/3 rounded-md border border-status-prevBg px-2 py-1.5 text-sm focus:border-brand-500 focus:outline-none"
                  />
                  <input
                    type="text"
                    value={row.value}
                    onChange={(e) => updateRow(i, { value: e.target.value })}
                    placeholder="content"
                    aria-label={`Meta tag value ${i + 1}`}
                    className="flex-1 rounded-md border border-status-prevBg px-2 py-1.5 text-sm focus:border-brand-500 focus:outline-none"
                  />
                  <button
                    type="button"
                    onClick={() => removeRow(i)}
                    aria-label={`Remove meta tag ${i + 1}`}
                    className="rounded p-1 text-status-prev hover:bg-status-prevBg disabled:opacity-40"
                    disabled={metaRows.length === 1}
                  >
                    ×
                  </button>
                </div>
              ))}
              <button
                type="button"
                onClick={addRow}
                className="w-fit text-xs font-medium text-action-600 hover:text-action-700"
              >
                + Add meta tag
              </button>
            </div>
          </div>
        </>
      )}

      {/* Advanced — request headers, User-Agent, matched Site, raw body.
          Collapsed by default so the panel stays simple. */}
      <div className="mt-3 rounded-md border border-status-prevBg">
        <button
          type="button"
          onClick={() => setAdvancedOpen((o) => !o)}
          aria-expanded={advancedOpen}
          className="flex w-full items-center justify-between px-3 py-2 text-xs font-semibold text-nav"
        >
          Advanced
          <span aria-hidden className="text-status-prevFg">
            {advancedOpen ? "▲" : "▼"}
          </span>
        </button>
        {advancedOpen && (
          <div className="space-y-3 border-t border-status-prevBg px-3 py-3">
            <label className="flex flex-col gap-1 text-xs font-medium text-nav">
              User-Agent
              <input
                type="text"
                value={userAgent}
                onChange={(e) => setUserAgent(e.target.value)}
                placeholder="Mozilla/5.0 (iPhone; …)"
                aria-label="User-Agent"
                className={inputClass}
              />
            </label>

            <div className="flex flex-col gap-1 text-xs font-medium text-nav">
              Site (for Site Match)
              <SiteSelectControl
                id="test-panel-site"
                value={site}
                onChange={setSite}
              />
            </div>

            <div>
              <div className="text-xs font-medium text-nav">
                Request headers
              </div>
              <HeaderRowsEditor rows={headerRows} onChange={setHeaderRows} />
            </div>

            {/* Raw response body only applies to HTML features — for JSON the
                "Response JSON" textarea above IS the body (sent as response_json,
                which the proxy gives precedence over response_body anyway). */}
            {!isJson && (
              <label className="flex flex-col gap-1 text-xs font-medium text-nav">
                Raw HTML body
                <textarea
                  value={rawBody}
                  onChange={(e) => setRawBody(e.target.value)}
                  placeholder="<html>…</html>"
                  rows={5}
                  aria-label="Raw response body"
                  className="rounded-md border border-status-prevBg px-2 py-1.5 font-mono text-xs focus:border-brand-500 focus:outline-none"
                />
              </label>
            )}
          </div>
        )}
      </div>

      <div className="mt-4 flex items-center gap-3">
        <button
          type="button"
          onClick={handleRun}
          disabled={mutation.isPending || !headersValid}
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
          // Remount on each run so the stepper resets to the first node (and
          // re-collapses) rather than carrying a stale index across runs.
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
