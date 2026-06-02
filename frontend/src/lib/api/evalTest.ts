// "Test a rule" eval client (WS4). Posts the currently-selected canvas plus a
// synthetic request context to the proxy's internal eval-trace endpoint and
// returns the traversed node/edge path so the canvas can highlight it.
//
// The endpoint runs the REAL zen JDM evaluator inside the proxy (not a TS
// reimplementation), so the highlighted path matches production evaluation.
// The proxy is a separate service from the backend API, hence its own base URL.
import { z } from "zod";

import { parseApiError } from "@/lib/api/client";
import { CanvasGraph } from "@/lib/api/ruleGraph";

export const PROXY_BASE =
  process.env.NEXT_PUBLIC_PROXY_BASE ?? "http://localhost:9000";

export const DeviceType = z.enum(["mobile", "desktop", "tablet"]);
export type DeviceType = z.infer<typeof DeviceType>;

// Content kind of the synthetic response under test (proxy eval delta). "html"
// keeps the device/meta/path inputs; "json" carries a parsed response body so
// json_expression nodes evaluate.
export const ContentKind = z.enum(["html", "json"]);
export type ContentKind = z.infer<typeof ContentKind>;

// Test input context. Mirrors the proxy's EvaluationContext test fields. All
// optional — the user fills only what the selected canvas's processors read.
// JSON features send `response_json` (PREFERRED — a parsed object, e.g.
// {"type":"premium"}); `response_body`/`content_kind` are the raw-string
// fallbacks the proxy also accepts.
export const EvalContext = z.object({
  device_type: DeviceType.optional(),
  user_agent: z.string().optional(),
  meta_tags: z.record(z.string(), z.string()).optional(),
  path: z.string().optional(),
  url: z.string().optional(),
  response_json: z.unknown().optional(),
  response_body: z.string().optional(),
  content_kind: ContentKind.optional(),
});
export type EvalContext = z.infer<typeof EvalContext>;

export const EvalRequest = z.object({
  canvas: CanvasGraph,
  context: EvalContext,
});
export type EvalRequest = z.infer<typeof EvalRequest>;

export const EvalStep = z.object({
  node_id: z.string(),
  kind: z.string(),
  branch: z.enum(["yes", "no"]).nullable(),
  result: z.boolean().nullable(),
});
export type EvalStep = z.infer<typeof EvalStep>;

// One node in the Transformation Journey (expression-nodes-spec §5). `body_after`
// is the body AFTER this node's action — a JSON value for JSON features, a raw
// string for HTML. Decisions/start/end don't mutate, so theirs equals the
// running body. `branch` is the decision's taken branch (true/false) or null for
// non-decision nodes.
export const JourneyStep = z.object({
  index: z.number(),
  node_id: z.string(),
  kind: z.string(),
  label: z.string(),
  branch: z.boolean().nullable(),
  body_after: z.unknown(),
  // Per-node apply time, ms, formatted "d.dd" (features-matched-spec §3/§5).
  // Expression steps carry the node's apply time; start/decision/end are
  // "0.00". Additive — absent on older proxies → defaults to "0.00".
  time_ms: z.string().default("0.00"),
});
export type JourneyStep = z.infer<typeof JourneyStep>;

// One expression node in the summary (features-matched-spec §v2.2). Same object
// shape for both `expressions` and `expensive_nodes`. `custom_expression_label`
// is the node's custom_label (v2.3), "" when unset. Times are "d.dd" strings.
export const SummaryExpression = z.object({
  expression_id: z.string(),
  expression_label: z.string(),
  custom_expression_label: z.string(),
  expression_time_ms: z.string(),
});
export type SummaryExpression = z.infer<typeof SummaryExpression>;

// Per-feature timing summary for the canvas under test (features-matched-spec
// §5 / §v2.2). Mirrors a single feature's `feature_expressions` entry. All
// times are strings formatted "d.dd" so trailing zeros survive (0.10, not 0.1).
export const EvalSummary = z.object({
  expressions: z.array(SummaryExpression),
  time_took_ms: z.string(),
  expensive_nodes: z.array(SummaryExpression),
});
export type EvalSummary = z.infer<typeof EvalSummary>;

export const EvalResponse = z.object({
  matched_node_id: z.string().nullable(),
  traversed_node_ids: z.array(z.string()),
  traversed_edge_ids: z.array(z.string()),
  steps: z.array(EvalStep),
  // Additive (expression-nodes-spec §5). Absent on older proxies → defaults to
  // an empty array so the journey view simply doesn't render.
  journey: z.array(JourneyStep).default([]),
  // Additive (features-matched-spec §5). Absent on older proxies → null, so
  // the timing summary simply doesn't render.
  summary: EvalSummary.nullish().default(null),
});
export type EvalResponse = z.infer<typeof EvalResponse>;

// POST to the proxy. Mirrors lib/api/client.request but targets PROXY_BASE.
export async function postEvalTest(body: EvalRequest): Promise<EvalResponse> {
  const res = await fetch(`${PROXY_BASE}/__rre/eval`, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify(body),
  });

  if (!res.ok) {
    const text = await res.text().catch(() => "");
    throw parseApiError(res.status, res.statusText, text);
  }

  return EvalResponse.parse(await res.json());
}
