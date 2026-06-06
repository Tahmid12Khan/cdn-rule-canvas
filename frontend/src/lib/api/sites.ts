import { z } from "zod";

import { apiGet, apiSend, Page } from "@/lib/api/client";
import { SLUG_RE } from "@/lib/api/features";

// Site API client (spec §6). Mirrors the backend SiteCreate / SiteUpdate /
// SiteRead DTOs (spec §3 + §4). All keys are snake_case on the wire.

export const SiteProtocol = z.enum(["http", "https"]);
export type SiteProtocol = z.infer<typeof SiteProtocol>;

export const SiteRead = z.object({
  slug: z.string(),
  name: z.string(),
  source_protocol: SiteProtocol,
  source_host: z.string(),
  source_port: z.number(),
  dest_protocol: SiteProtocol,
  dest_host: z.string(),
  dest_port: z.number(),
  created_at: z.string(), // ISO8601 (DateTime<Utc>)
  updated_at: z.string(),
});
export type SiteRead = z.infer<typeof SiteRead>;

// Port: integer 1..=65535. Coerced because the form supplies strings.
const sitePort = z.coerce
  .number()
  .int("Port must be a whole number")
  .min(1, "Port must be between 1 and 65535")
  .max(65535, "Port must be between 1 and 65535");

const siteHost = z
  .string()
  .min(1, "Host is required")
  .max(255, "Host must be at most 255 characters");

export const SiteCreate = z.object({
  slug: z
    .string()
    .min(3, "Slug must be at least 3 characters")
    .max(64, "Slug must be at most 64 characters")
    .regex(SLUG_RE, "Use lowercase kebab-case (e.g. my-site)"),
  name: z
    .string()
    .min(1, "Name is required")
    .max(200, "Name must be at most 200 characters"),
  source_protocol: SiteProtocol,
  source_host: siteHost,
  source_port: sitePort,
  dest_protocol: SiteProtocol,
  dest_host: siteHost,
  dest_port: sitePort,
});
export type SiteCreate = z.infer<typeof SiteCreate>;

// SiteUpdate: every field optional except slug (slug is immutable / path-derived).
export const SiteUpdate = z.object({
  name: z.string().min(1).max(200).optional(),
  source_protocol: SiteProtocol.optional(),
  source_host: siteHost.optional(),
  source_port: sitePort.optional(),
  dest_protocol: SiteProtocol.optional(),
  dest_host: siteHost.optional(),
  dest_port: sitePort.optional(),
});
export type SiteUpdate = z.infer<typeof SiteUpdate>;

export interface ListSitesParams {
  page: number;
  page_size: number;
  q?: string;
}

export const listSites = (
  p: ListSitesParams = { page: 1, page_size: 20 },
) => {
  const params = new URLSearchParams({
    page: String(p.page),
    page_size: String(p.page_size),
  });
  if (p.q && p.q.trim() !== "") params.set("q", p.q.trim());
  return apiGet(`/api/v1/sites?${params.toString()}`, Page(SiteRead));
};

export const getSite = (slug: string) =>
  apiGet(`/api/v1/sites/${slug}`, SiteRead);

export const createSite = (b: SiteCreate) =>
  apiSend("POST", `/api/v1/sites`, SiteRead, b);

export const updateSite = (slug: string, b: SiteUpdate) =>
  apiSend("PATCH", `/api/v1/sites/${slug}`, SiteRead, b);

export const deleteSite = (slug: string) =>
  apiSend("DELETE", `/api/v1/sites/${slug}`, z.void());

// Small-page case-insensitive name search for the site_select picker (spec §6).
const SEARCH_PAGE_SIZE = 10;

export const searchSites = (q: string) =>
  listSites({ page: 1, page_size: SEARCH_PAGE_SIZE, q });
