"use client";

// TestPanel (WS4). A small side panel under the canvas that lets the user enter
// a synthetic request context (device + meta tags + optional path) for the
// currently-selected canvas, run it through the proxy's real JDM evaluator, and
// highlight the traversed node/edge path on the canvas.
//
// The graph it tests is the LIVE (possibly unsaved) canvas from the store, so
// edits can be tested before saving. Highlight state is stored on the
// ruleBuilderStore so the custom nodes/edges can subscribe and glow.
import { useMemo, useState } from "react";
import { useMutation } from "@tanstack/react-query";

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

export function TestPanel({ outcomeTitleById, featureType }: TestPanelProps) {
  const isJson = featureType === "json";
  const selected = useRuleBuilderStore((s) => s.selected);
  const setTestHighlight = useRuleBuilderStore((s) => s.setTestHighlight);
  const clearTestHighlight = useRuleBuilderStore((s) => s.clearTestHighlight);
  const highlight = useRuleBuilderStore((s) => s.testHighlight);

  const [device, setDevice] = useState<DeviceType | "">("");
  const [path, setPath] = useState("");
  const [metaRows, setMetaRows] = useState<MetaRow[]>([{ key: "", value: "" }]);
  // JSON features: a textarea holding the synthetic response body. Parsed to a
  // value on Run; a parse error is shown inline and blocks the request.
  const [jsonBody, setJsonBody] = useState("");
  const [jsonParseError, setJsonParseError] = useState<string | null>(null);

  // Introspect the selected canvas's decision processors so we can hint which
  // inputs actually matter (meta_tags vs device_type).
  const usesDevice = useRuleBuilderStore((s) =>
    s.canvases[s.selected].nodes.some(
      (n) =>
        n.type === "decisionNode" && n.data.processor.type === "device_type",
    ),
  );
  const usesMetaTags = useRuleBuilderStore((s) =>
    s.canvases[s.selected].nodes.some(
      (n) => n.type === "decisionNode" && n.data.processor.type === "meta_tags",
    ),
  );

  const mutation = useMutation({
    mutationFn: async () => {
      const { canvases } = useRuleBuilderStore.getState();
      const c = canvases[selected];
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

      return postEvalTest({ canvas, context });
    },
    onSuccess: (res) => {
      // A path is a dead-end only when it never reaches an END node — NOT merely
      // when no apply_outcome ran. The NO branch straight to END is a complete
      // path (its terminal body IS the output). The journey's last step is the
      // END node when the path completed (the proxy appends it).
      const reachedEnd =
        res.journey.length > 0 &&
        res.journey[res.journey.length - 1].kind === "end";
      setTestHighlight({
        nodeIds: new Set(res.traversed_node_ids),
        edgeIds: new Set(res.traversed_edge_ids),
        outcomeNodeId: res.matched_node_id,
        deadEnd: !reachedEnd,
      });
    },
  });

  // Whether the matched path reached an END node (the path completed). Drives the
  // result banner: reaching END is success — that terminal body IS the output
  // (expression-nodes-spec §0/§5), even when no apply_outcome ran on the path.
  const reachedEnd = useMemo(() => {
    const j = mutation.data?.journey;
    return Boolean(j && j.length > 0 && j[j.length - 1].kind === "end");
  }, [mutation.data]);

  // The matched node is the terminal expression node on the path. The proxy no
  // longer returns a single outcome id (the canvas now applies an ordered action
  // pipeline, expression-nodes-spec §0/§5); recover the applied outcome's id from
  // that node's apply_outcome action in the live canvas so the banner can name it.
  const matchedTitle = useMemo(() => {
    const matchedNodeId = mutation.data?.matched_node_id;
    if (!matchedNodeId) return null;
    const { canvases } = useRuleBuilderStore.getState();
    const node = canvases[selected].nodes.find((n) => n.id === matchedNodeId);
    if (node?.type !== "expressionNode") return null;
    const { action, outcomeTitle } = node.data;
    if (action.type !== "apply_outcome") return null;
    if (outcomeTitle) return outcomeTitle;
    const outcomeId = action.outcome_id;
    if (typeof outcomeId !== "string" || !outcomeId) return null;
    return outcomeTitleById(outcomeId);
  }, [mutation.data, selected, outcomeTitleById]);

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
    clearTestHighlight();
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

  const evalError = mutation.error
    ? toUserError(mutation.error, { surface: "eval" })
    : null;
  const evalRawBody =
    mutation.error instanceof ApiError ? mutation.error.rawBody : undefined;

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
        Enter a {isJson ? "response body" : "request context"} for the{" "}
        <span className="font-semibold">{selected}</span> canvas and run it
        through the evaluator to highlight the path.
      </p>

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

      <div className="mt-4 flex items-center gap-3">
        <button
          type="button"
          onClick={handleRun}
          disabled={mutation.isPending}
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
          // re-glows it) rather than carrying a stale index across runs.
          key={mutation.submittedAt}
          journey={mutation.data.journey}
          featureType={featureType}
        />
      )}
    </section>
  );
}
