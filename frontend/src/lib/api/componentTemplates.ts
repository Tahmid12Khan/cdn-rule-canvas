import {
  useMutation,
  useQuery,
  useQueryClient,
} from "@tanstack/react-query";
import { z } from "zod";

import { apiGet, apiSend, Page } from "@/lib/api/client";
import {
  ComponentTemplateCreate,
  ComponentTemplateRead,
  ComponentTemplateSummary,
  ComponentTemplateUpdate,
  ComponentTemplateVersionRead,
  ResolvedComponentRead,
  VersionCreate,
  VersionUpdate,
} from "@/lib/schemas/componentTemplates";

// Component-template API client + TanStack Query hooks (component-editor design
// §5.2). NOT `components.ts` — that file is the outcome sub-component client.
// All wire shapes mirror the locked backend DTOs (design §3.2); every response
// is zod-validated through the shared apiGet/apiSend helpers.

const BASE = "/api/v1/component-templates";

// ── Plain fetchers ───────────────────────────────────────────────────────────

export interface ListComponentTemplatesParams {
  page: number;
  page_size: number;
  q?: string;
}

export const listComponentTemplates = (
  p: ListComponentTemplatesParams = { page: 1, page_size: 20 },
) => {
  const params = new URLSearchParams({
    page: String(p.page),
    page_size: String(p.page_size),
  });
  if (p.q && p.q.trim() !== "") params.set("q", p.q.trim());
  return apiGet(`${BASE}?${params.toString()}`, Page(ComponentTemplateSummary));
};

export const getComponentTemplate = (cid: string) =>
  apiGet(`${BASE}/${cid}`, ComponentTemplateRead);

// Resolve a component by its route slug. Returns the same ComponentTemplateRead
// shape as getComponentTemplate; 404s when the slug is unknown.
export const getComponentTemplateBySlug = (slug: string) =>
  apiGet(`${BASE}/by-slug/${encodeURIComponent(slug)}`, ComponentTemplateRead);

export const createComponentTemplate = (b: ComponentTemplateCreate) =>
  apiSend("POST", BASE, ComponentTemplateRead, b);

export const updateComponentTemplate = (
  cid: string,
  b: ComponentTemplateUpdate,
) => apiSend("PATCH", `${BASE}/${cid}`, ComponentTemplateRead, b);

export const deleteComponentTemplate = (cid: string) =>
  apiSend("DELETE", `${BASE}/${cid}`, z.void());

export const listVersions = (cid: string) =>
  apiGet(`${BASE}/${cid}/versions`, z.array(ComponentTemplateVersionRead));

export const getVersion = (cid: string, vnum: number) =>
  apiGet(`${BASE}/${cid}/versions/${vnum}`, ComponentTemplateVersionRead);

export const createVersion = (cid: string, b: VersionCreate) =>
  apiSend("POST", `${BASE}/${cid}/versions`, ComponentTemplateVersionRead, b);

export const updateVersion = (cid: string, vnum: number, b: VersionUpdate) =>
  apiSend(
    "PATCH",
    `${BASE}/${cid}/versions/${vnum}`,
    ComponentTemplateVersionRead,
    b,
  );

export const deleteVersion = (cid: string, vnum: number) =>
  apiSend("DELETE", `${BASE}/${cid}/versions/${vnum}`, z.void());

export const makeDefaultVersion = (cid: string, vnum: number) =>
  apiSend(
    "POST",
    `${BASE}/${cid}/versions/${vnum}/make-default`,
    ComponentTemplateRead,
  );

// `version` is the string "default" or a positive version number. Proxy-facing,
// but reused by the editor preview for the resolved variable list.
export const resolveComponent = (cid: string, version: "default" | number) =>
  apiGet(
    `${BASE}/${cid}/resolve?version=${version}`,
    ResolvedComponentRead,
  );

// ── Query keys ───────────────────────────────────────────────────────────────
export const componentKeys = {
  all: ["component-templates"] as const,
  list: (params: ListComponentTemplatesParams) =>
    ["component-templates", "list", params] as const,
  detail: (cid: string) => ["component-templates", "detail", cid] as const,
  versions: (cid: string) =>
    ["component-templates", "versions", cid] as const,
  version: (cid: string, vnum: number) =>
    ["component-templates", "version", cid, vnum] as const,
};

// ── Hooks ────────────────────────────────────────────────────────────────────

export function useComponentTemplates(params: ListComponentTemplatesParams) {
  return useQuery({
    queryKey: componentKeys.list(params),
    queryFn: () => listComponentTemplates(params),
  });
}

export function useComponentTemplate(cid: string, enabled = true) {
  return useQuery({
    queryKey: componentKeys.detail(cid),
    queryFn: () => getComponentTemplate(cid),
    enabled,
  });
}

export function useComponentVersion(
  cid: string,
  vnum: number | null,
) {
  return useQuery({
    queryKey: vnum === null ? ["component-templates", "version", cid, "none"] : componentKeys.version(cid, vnum),
    queryFn: () => getVersion(cid, vnum as number),
    enabled: vnum !== null,
  });
}

export function useCreateComponentTemplate() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (b: ComponentTemplateCreate) => createComponentTemplate(b),
    onSuccess: () => {
      void qc.invalidateQueries({ queryKey: componentKeys.all });
    },
  });
}

export function useUpdateComponentTemplate(cid: string) {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (b: ComponentTemplateUpdate) => updateComponentTemplate(cid, b),
    onSuccess: () => {
      void qc.invalidateQueries({ queryKey: componentKeys.all });
    },
  });
}

export function useDeleteComponentTemplate() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (cid: string) => deleteComponentTemplate(cid),
    onSuccess: () => {
      void qc.invalidateQueries({ queryKey: componentKeys.all });
    },
  });
}

export function useCreateVersion(cid: string) {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (b: VersionCreate) => createVersion(cid, b),
    onSuccess: () => {
      void qc.invalidateQueries({ queryKey: componentKeys.detail(cid) });
      void qc.invalidateQueries({ queryKey: componentKeys.versions(cid) });
    },
  });
}

export function useUpdateVersion(cid: string) {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: ({ vnum, body }: { vnum: number; body: VersionUpdate }) =>
      updateVersion(cid, vnum, body),
    onSuccess: (_data, { vnum }) => {
      void qc.invalidateQueries({ queryKey: componentKeys.detail(cid) });
      void qc.invalidateQueries({ queryKey: componentKeys.versions(cid) });
      void qc.invalidateQueries({ queryKey: componentKeys.version(cid, vnum) });
    },
  });
}

export function useDeleteVersion(cid: string) {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (vnum: number) => deleteVersion(cid, vnum),
    onSuccess: () => {
      void qc.invalidateQueries({ queryKey: componentKeys.detail(cid) });
      void qc.invalidateQueries({ queryKey: componentKeys.versions(cid) });
    },
  });
}

export function useMakeDefaultVersion(cid: string) {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (vnum: number) => makeDefaultVersion(cid, vnum),
    onSuccess: () => {
      void qc.invalidateQueries({ queryKey: componentKeys.detail(cid) });
      void qc.invalidateQueries({ queryKey: componentKeys.versions(cid) });
    },
  });
}
