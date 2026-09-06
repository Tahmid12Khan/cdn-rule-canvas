import { z } from "zod";

import { apiGet, apiSend, Page } from "@/lib/api/client";

// Product Catalogue API client. Mirrors the backend ProductCreate / ProductUpdate
// / ProductRead DTOs. All keys are snake_case on the wire.

export const PRODUCT_LABEL_RE = /^[a-z0-9]+(_[a-z0-9]+)*$/;

export const ProductRead = z.object({
  label: z.string(),
  name: z.string(),
  description: z.string().nullable(),
  created_at: z.string(),
  updated_at: z.string(),
});
export type ProductRead = z.infer<typeof ProductRead>;

export const ProductCreate = z.object({
  label: z
    .string()
    .min(1, "Label is required")
    .max(64, "Label must be at most 64 characters")
    .regex(PRODUCT_LABEL_RE, "Use lowercase snake_case (e.g. premium_tier)"),
  name: z.string().min(1, "Name is required").max(200),
  description: z.string().max(500).optional(),
});
export type ProductCreate = z.infer<typeof ProductCreate>;

export const ProductUpdate = z.object({
  name: z.string().min(1).max(200).optional(),
  description: z.string().max(500).optional(),
});
export type ProductUpdate = z.infer<typeof ProductUpdate>;

export interface ListProductsParams {
  page: number;
  page_size: number;
  q?: string;
}

export const listProducts = (
  p: ListProductsParams = { page: 1, page_size: 20 },
) => {
  const params = new URLSearchParams({
    page: String(p.page),
    page_size: String(p.page_size),
  });
  if (p.q && p.q.trim() !== "") params.set("q", p.q.trim());
  return apiGet(`/api/v1/products?${params.toString()}`, Page(ProductRead));
};

export const getProduct = (label: string) =>
  apiGet(`/api/v1/products/${label}`, ProductRead);

export const createProduct = (b: ProductCreate) =>
  apiSend("POST", `/api/v1/products`, ProductRead, b);

export const updateProduct = (label: string, b: ProductUpdate) =>
  apiSend("PATCH", `/api/v1/products/${label}`, ProductRead, b);

export const deleteProduct = (label: string) =>
  apiSend("DELETE", `/api/v1/products/${label}`, z.void());

// Small-page case-insensitive name search for the product_select picker.
const SEARCH_PAGE_SIZE = 10;

export const searchProducts = (q: string) =>
  listProducts({ page: 1, page_size: SEARCH_PAGE_SIZE, q });
