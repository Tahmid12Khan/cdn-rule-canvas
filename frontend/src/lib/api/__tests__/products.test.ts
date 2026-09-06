import { http, HttpResponse } from "msw";
import { describe, expect, it } from "vitest";

import { API_BASE } from "@/lib/api/client";
import {
  createProduct,
  deleteProduct,
  listProducts,
  ProductCreate,
  type ProductRead,
  searchProducts,
  updateProduct,
} from "@/lib/api/products";
import { server } from "@/test/mocks/server";

const PRODUCTS_URL = `${API_BASE}/api/v1/products`;

const sample: ProductRead = {
  label: "premium_tier",
  name: "Premium Tier",
  description: "Full access",
  created_at: "2026-06-06T00:00:00Z",
  updated_at: "2026-06-06T00:00:00Z",
};

function page(items: ProductRead[], total = items.length) {
  return { items, page: 1, page_size: 20, total };
}

describe("products api", () => {
  it("listProducts builds the page/page_size query and parses the envelope", async () => {
    let requested: URL | null = null;
    server.use(
      http.get(PRODUCTS_URL, ({ request }) => {
        requested = new URL(request.url);
        return HttpResponse.json(page([sample]));
      }),
    );

    const res = await listProducts({ page: 2, page_size: 5 });
    expect(requested!.searchParams.get("page")).toBe("2");
    expect(requested!.searchParams.get("page_size")).toBe("5");
    expect(requested!.searchParams.has("q")).toBe(false);
    expect(res.items[0].label).toBe("premium_tier");
    expect(res.items[0].name).toBe("Premium Tier");
  });

  it("listProducts sends a trimmed q only when non-empty", async () => {
    let requested: URL | null = null;
    server.use(
      http.get(PRODUCTS_URL, ({ request }) => {
        requested = new URL(request.url);
        return HttpResponse.json(page([]));
      }),
    );

    await listProducts({ page: 1, page_size: 20, q: "  premium  " });
    expect(requested!.searchParams.get("q")).toBe("premium");
  });

  it("searchProducts wraps list with a small page_size and the query", async () => {
    let requested: URL | null = null;
    server.use(
      http.get(PRODUCTS_URL, ({ request }) => {
        requested = new URL(request.url);
        return HttpResponse.json(page([sample]));
      }),
    );

    await searchProducts("prem");
    expect(requested!.searchParams.get("q")).toBe("prem");
    expect(requested!.searchParams.get("page_size")).toBe("10");
  });

  it("createProduct POSTs the body and parses ProductRead", async () => {
    let body: unknown = null;
    server.use(
      http.post(PRODUCTS_URL, async ({ request }) => {
        body = await request.json();
        return HttpResponse.json(sample, { status: 201 });
      }),
    );

    const created = await createProduct({
      label: "premium_tier",
      name: "Premium Tier",
      description: "Full access",
    });
    expect((body as { label: string }).label).toBe("premium_tier");
    expect(created.name).toBe("Premium Tier");
  });

  it("updateProduct PATCHes the label path", async () => {
    let method: string | null = null;
    server.use(
      http.patch(`${PRODUCTS_URL}/premium_tier`, ({ request }) => {
        method = request.method;
        return HttpResponse.json({ ...sample, name: "Renamed" });
      }),
    );

    const updated = await updateProduct("premium_tier", { name: "Renamed" });
    expect(method).toBe("PATCH");
    expect(updated.name).toBe("Renamed");
  });

  it("deleteProduct resolves on 204", async () => {
    server.use(
      http.delete(`${PRODUCTS_URL}/premium_tier`, () =>
        HttpResponse.text("", { status: 204 }),
      ),
    );
    await expect(deleteProduct("premium_tier")).resolves.toBeUndefined();
  });
});

describe("ProductCreate schema", () => {
  const valid = { label: "premium_tier", name: "Premium Tier" };

  it("accepts a valid snake_case label", () => {
    expect(ProductCreate.safeParse(valid).success).toBe(true);
  });

  it("rejects a non-snake_case label", () => {
    expect(
      ProductCreate.safeParse({ ...valid, label: "Bad-Label" }).success,
    ).toBe(false);
  });

  it("rejects an empty name", () => {
    expect(ProductCreate.safeParse({ ...valid, name: "" }).success).toBe(
      false,
    );
  });
});
