// "Test Full Journey" eval client (spec item 5). Posts a real URL to the proxy's
// full-journey endpoint, which fetches the live upstream, resolves the host to a
// Site, then runs EVERY configured feature (HTML + JSON rules, in execution
// order) against the response — returning each feature's Transformation Journey
// + timing summary so the UI can render the three diff levels (per-node,
// per-feature start→end, cross-feature first→last).
//
// The endpoint runs the REAL zen JDM evaluator inside the proxy (same engine as
// production), so the diffs match what the proxy would actually serve. The proxy
// is a separate service from the backend API, hence its own base URL.
import { z } from "zod";

import { parseApiError } from "@/lib/api/client";
import {
  ContentKind,
  EvalSummary,
  JourneyStep,
  PROXY_BASE,
} from "@/lib/api/evalTest";

// One feature's full-journey result. `matched` is false when the feature's path
// never reached an outcome (skipped / not applicable to this response). `journey`
// is the SAME shape as a single-canvas eval (lib/api/evalTest JourneyStep) and
// `summary` mirrors that feature's timing summary (null on older proxies / when
// the feature didn't run).
export const FullJourneyFeature = z.object({
  feature_id: z.string(),
  name: z.string(),
  type: ContentKind, // "html" | "json"
  execution_order: z.number().int(),
  version_number: z.number().nullable(),
  matched: z.boolean(),
  // Additive — absent on older proxies → empty array so the feature renders as
  // "no journey" rather than crashing.
  journey: z.array(JourneyStep).default([]),
  summary: EvalSummary.nullable().default(null),
  time_took_ms: z.string().default("0.00"),
});
export type FullJourneyFeature = z.infer<typeof FullJourneyFeature>;

export const FullJourneyResponse = z.object({
  content_kind: ContentKind, // "html" | "json"
  site: z.string().nullable(),
  // EXACT rule-engine wall-clock across ALL features (spec item 7) — not the sum
  // of per-feature times. Formatted "d.dd".
  total_time_ms: z.string(),
  features: z.array(FullJourneyFeature).default([]),
});
export type FullJourneyResponse = z.infer<typeof FullJourneyResponse>;

// Request body. `version_overrides` maps a feature_id → a concrete version_number
// to run instead of the feature's active version for the chosen env (live by
// default). Features absent from the map run their active version.
export interface FullJourneyRequest {
  url: string;
  headers?: Record<string, string>;
  env?: "live" | "staging";
  version_overrides?: Record<string, number>;
}

// POST to the proxy's full-journey endpoint. Same error handling as
// postEvalUrlTest (parseApiError attaches the raw body for the ErrorBanner).
export async function postEvalFullJourney(
  body: FullJourneyRequest,
): Promise<FullJourneyResponse> {
  const res = await fetch(`${PROXY_BASE}/__rre/eval-full-journey`, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify(body),
  });

  if (!res.ok) {
    const text = await res.text().catch(() => "");
    throw parseApiError(res.status, res.statusText, text);
  }

  return FullJourneyResponse.parse(await res.json());
}
