import { z } from "zod";

import { apiGet, apiSend, Page } from "@/lib/api/client";
import { PublishEnvironment, VersionStatus } from "@/lib/api/enums";
import { RuleGraph } from "@/lib/api/ruleGraph";

// Mirrors BACKEND CONTRACT §5 Version DTOs verbatim. Wire timestamps are
// ISO8601 strings (DateTime<Utc>); version_number is an i32.

// List rows — lighter, no rule_graph payload (BACKEND VersionSummary).
export const VersionSummary = z.object({
  id: z.string().uuid(),
  feature_id: z.string(),
  version_number: z.number().int(),
  description: z.string().nullable(),
  status: VersionStatus,
  last_updated_by: z.string(),
  last_updated_at: z.string(),
  created_at: z.string(),
});
export type VersionSummary = z.infer<typeof VersionSummary>;

// Full read — adds rule_graph + created_by (BACKEND VersionRead).
export const VersionRead = VersionSummary.extend({
  rule_graph: RuleGraph,
  created_by: z.string(),
});
export type VersionRead = z.infer<typeof VersionRead>;

export const VersionCreate = z.object({
  description: z.string().max(2000).optional(),
});
export type VersionCreate = z.infer<typeof VersionCreate>;

export const VersionUpdate = z.object({
  description: z.string().max(2000).optional(),
  rule_graph: RuleGraph.optional(),
});
export type VersionUpdate = z.infer<typeof VersionUpdate>;

export const PublishRequest = z.object({ environment: PublishEnvironment });
export type PublishRequest = z.infer<typeof PublishRequest>;

export interface VersionListQuery {
  status?: string;
  search?: string;
  page?: number;
  page_size?: number;
}

// Drops undefined / empty values so they never reach the query string.
function cleanQuery(q: VersionListQuery): Record<string, string> {
  const out: Record<string, string> = {};
  if (q.status) out.status = q.status;
  if (q.search) out.search = q.search;
  if (q.page !== undefined) out.page = String(q.page);
  if (q.page_size !== undefined) out.page_size = String(q.page_size);
  return out;
}

export const listVersions = (fid: string, q: VersionListQuery = {}) =>
  apiGet(
    `/api/v1/features/${fid}/versions?${new URLSearchParams(cleanQuery(q)).toString()}`,
    Page(VersionSummary),
  );

export const getVersion = (fid: string, vnum: number) =>
  apiGet(`/api/v1/features/${fid}/versions/${vnum}`, VersionRead);

export const createVersion = (fid: string, b: VersionCreate) =>
  apiSend("POST", `/api/v1/features/${fid}/versions`, VersionRead, b);

export const updateVersion = (fid: string, vnum: number, b: VersionUpdate) =>
  apiSend("PATCH", `/api/v1/features/${fid}/versions/${vnum}`, VersionRead, b);

export const patchRuleGraph = (
  fid: string,
  vnum: number,
  rule_graph: z.infer<typeof RuleGraph>,
) => updateVersion(fid, vnum, { rule_graph });

export const publishVersion = (fid: string, vnum: number, b: PublishRequest) =>
  apiSend(
    "POST",
    `/api/v1/features/${fid}/versions/${vnum}/publish`,
    VersionRead,
    b,
  );

export const unpublishVersion = (
  fid: string,
  vnum: number,
  b: PublishRequest,
) =>
  apiSend(
    "POST",
    `/api/v1/features/${fid}/versions/${vnum}/unpublish`,
    VersionRead,
    b,
  );

export const deleteVersion = (fid: string, vnum: number) =>
  apiSend("DELETE", `/api/v1/features/${fid}/versions/${vnum}`, z.void());
