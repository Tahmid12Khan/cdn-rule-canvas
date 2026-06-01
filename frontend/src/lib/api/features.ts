import { z } from "zod";

import { apiGet, apiSend, Page } from "@/lib/api/client";
import { FeatureType } from "@/lib/api/enums";

// Feature API client (FRONTEND CONTRACT §5.4, Task 06). Mirrors the backend
// FeatureCreate / FeatureUpdate / FeatureRead DTOs (BACKEND CONTRACT §5).

// BACKEND SLUG_RE — kebab-case, lowercase.
export const SLUG_RE = /^[a-z0-9]+(?:-[a-z0-9]+)*$/;

export const FeatureRead = z.object({
  id: z.string(),
  name: z.string(),
  type: FeatureType,
  staging_version_id: z.string().uuid().nullable(),
  live_version_id: z.string().uuid().nullable(),
  created_at: z.string(), // ISO8601 (DateTime<Utc>)
  updated_at: z.string(),
});
export type FeatureRead = z.infer<typeof FeatureRead>;

export const FeatureCreate = z.object({
  id: z
    .string()
    .min(3, "Slug must be at least 3 characters")
    .max(64, "Slug must be at most 64 characters")
    .regex(SLUG_RE, "Use lowercase kebab-case (e.g. my-feature)"),
  name: z
    .string()
    .min(1, "Name is required")
    .max(200, "Name must be at most 200 characters"),
  type: FeatureType,
});
export type FeatureCreate = z.infer<typeof FeatureCreate>;

export const FeatureUpdate = z.object({
  name: z.string().min(1).max(200).optional(),
});
export type FeatureUpdate = z.infer<typeof FeatureUpdate>;

export interface ListFeaturesParams {
  page: number;
  page_size: number;
}

export const listFeatures = (
  p: ListFeaturesParams = { page: 1, page_size: 20 },
) =>
  apiGet(
    `/api/v1/features?page=${p.page}&page_size=${p.page_size}`,
    Page(FeatureRead),
  );

export const getFeature = (fid: string) =>
  apiGet(`/api/v1/features/${fid}`, FeatureRead);

export const createFeature = (b: FeatureCreate) =>
  apiSend("POST", `/api/v1/features`, FeatureRead, b);

export const updateFeature = (fid: string, b: FeatureUpdate) =>
  apiSend("PATCH", `/api/v1/features/${fid}`, FeatureRead, b);

export const deleteFeature = (fid: string) =>
  apiSend("DELETE", `/api/v1/features/${fid}`, z.void());
