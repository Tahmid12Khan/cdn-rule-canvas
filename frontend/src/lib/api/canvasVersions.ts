// Canvas-specific version API calls (Tasks 11/14). These live in a dedicated
// module to avoid a write collision with Task 08's `lib/api/versions.ts` (which
// owns the version-list/CRUD calls). The shapes mirror BACKEND CONTRACT §5
// (VersionRead) and §6 (RuleGraph). If `lib/api/versions.ts` later exports the
// same helpers, prefer importing from there — this module is the canvas
// subtree's self-contained entry point.
import { z } from "zod";

import { apiGet, apiSend } from "@/lib/api/client";
import { VersionStatus } from "@/lib/api/enums";
import { RuleGraph } from "@/lib/api/ruleGraph";

export const VersionRead = z.object({
  id: z.string().uuid(),
  feature_id: z.string(),
  version_number: z.number().int(),
  description: z.string().nullable(),
  status: VersionStatus,
  rule_graph: RuleGraph,
  created_by: z.string(),
  last_updated_by: z.string(),
  last_updated_at: z.string(),
  created_at: z.string(),
});
export type VersionRead = z.infer<typeof VersionRead>;

export const VersionUpdate = z.object({
  description: z.string().max(2000).optional(),
  rule_graph: RuleGraph.optional(),
});
export type VersionUpdate = z.infer<typeof VersionUpdate>;

export const VersionCreate = z.object({
  description: z.string().max(2000).optional(),
  // The backend accepts the on-screen graph on create and atomically remaps the
  // carried-forward outcome refs to the new version's outcome ids (so the
  // returned rule_graph already references the new ids — no follow-up PATCH).
  rule_graph: RuleGraph.optional(),
});
export type VersionCreate = z.infer<typeof VersionCreate>;

export const getVersion = (fid: string, vnum: number): Promise<VersionRead> =>
  apiGet(`/api/v1/features/${fid}/versions/${vnum}`, VersionRead);

export const updateVersion = (
  fid: string,
  vnum: number,
  body: VersionUpdate,
): Promise<VersionRead> =>
  apiSend("PATCH", `/api/v1/features/${fid}/versions/${vnum}`, VersionRead, body);

// PATCH the current DRAFT version's rule_graph (BACKEND §7 PATCH version).
export const patchRuleGraph = (
  fid: string,
  vnum: number,
  rule_graph: z.infer<typeof RuleGraph>,
): Promise<VersionRead> => updateVersion(fid, vnum, { rule_graph });

export const createVersion = (
  fid: string,
  body: VersionCreate,
): Promise<VersionRead> =>
  apiSend("POST", `/api/v1/features/${fid}/versions`, VersionRead, body);

// "Save as New Version" (current canvas state). A SINGLE atomic POST: the
// backend creates the next-numbered draft, carries outcomes forward with new
// ids, and remaps the sent rule_graph's outcome refs server-side. The returned
// VersionRead's rule_graph already references the new ids — no follow-up PATCH
// (the old two-step create+PATCH raced on the stale outcome ids and 422'd).
export const createVersionFromGraph = (
  fid: string,
  description: string,
  rule_graph: z.infer<typeof RuleGraph>,
): Promise<VersionRead> => createVersion(fid, { description, rule_graph });
