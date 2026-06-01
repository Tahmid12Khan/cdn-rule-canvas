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
  meta_tags: z.record(z.string()).optional(),
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

export const EvalResponse = z.object({
  matched_outcome_id: z.string().nullable(),
  matched_node_id: z.string().nullable(),
  traversed_node_ids: z.array(z.string()),
  traversed_edge_ids: z.array(z.string()),
  steps: z.array(EvalStep),
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
