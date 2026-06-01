// Minimal outcomes fetch for the canvas subtree (Tasks 11/12). Dedicated module
// to avoid a write collision with Task 15's `lib/api/outcomes.ts`. We only need
// id + title to (a) populate the Outcomes palette category and (b) resolve the
// denormalized title cache on outcomeNode deserialize. Mirrors BACKEND CONTRACT
// §5 OutcomeRead (subset).
import { z } from "zod";

import { apiGet } from "@/lib/api/client";

export const CanvasOutcome = z.object({
  id: z.string().uuid(),
  version_id: z.string().uuid(),
  title: z.string(),
  is_builtin: z.boolean(),
  order_index: z.number().int(),
});
export type CanvasOutcome = z.infer<typeof CanvasOutcome>;

// GET /versions/{vid}/outcomes — passthrough extra fields are ignored by zod.
export const listCanvasOutcomes = (vid: string): Promise<CanvasOutcome[]> =>
  apiGet(`/api/v1/versions/${vid}/outcomes`, z.array(CanvasOutcome));
