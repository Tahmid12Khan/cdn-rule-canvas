import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { http, HttpResponse } from "msw";
import type { ReactNode } from "react";
import { describe, expect, it } from "vitest";

import { ProductCatalogueClient } from "@/components/product-catalogue/ProductCatalogueClient";
import { API_BASE } from "@/lib/api/client";
import type { ProductRead } from "@/lib/api/products";
import { server } from "@/test/mocks/server";

function wrapper({ children }: { children: ReactNode }) {
  const client = new QueryClient({
    defaultOptions: { queries: { retry: false }, mutations: { retry: false } },
  });
  return (
    <QueryClientProvider client={client}>{children}</QueryClientProvider>
  );
}

const PRODUCTS_URL = `${API_BASE}/api/v1/products`;

const sampleProduct: ProductRead = {
  label: "premium_tier",
  name: "Premium Tier",
  description: "Full access",
  created_at: "2026-06-06T00:00:00Z",
  updated_at: "2026-06-06T00:00:00Z",
};

function productsPage(items: ProductRead[], total = items.length) {
  return { items, page: 1, page_size: 20, total };
}

describe("ProductCatalogueClient", () => {
  it("renders the empty state when there are no products", async () => {
    server.use(
      http.get(PRODUCTS_URL, () => HttpResponse.json(productsPage([]))),
    );
    render(<ProductCatalogueClient />, { wrapper });

    await waitFor(() =>
      expect(screen.getByText(/no products yet/i)).toBeInTheDocument(),
    );
  });

  it("renders a row with label, name, and description", async () => {
    server.use(
      http.get(PRODUCTS_URL, () =>
        HttpResponse.json(productsPage([sampleProduct])),
      ),
    );
    render(<ProductCatalogueClient />, { wrapper });

    await waitFor(() =>
      expect(screen.getByText("Premium Tier")).toBeInTheDocument(),
    );
    expect(screen.getByText("premium_tier")).toBeInTheDocument();
    expect(screen.getByText("Full access")).toBeInTheDocument();
  });

  it("sends the search query to the products endpoint", async () => {
    const user = userEvent.setup();
    let lastQ: string | null = null;
    server.use(
      http.get(PRODUCTS_URL, ({ request }) => {
        lastQ = new URL(request.url).searchParams.get("q");
        return HttpResponse.json(productsPage([sampleProduct]));
      }),
    );
    render(<ProductCatalogueClient />, { wrapper });

    await waitFor(() =>
      expect(screen.getByText("Premium Tier")).toBeInTheDocument(),
    );

    await user.type(screen.getByLabelText(/search products/i), "prem");

    await waitFor(() => expect(lastQ).toBe("prem"));
  });

  it("creates a product via the modal and shows it in the list", async () => {
    const user = userEvent.setup();
    let items: ProductRead[] = [];
    server.use(
      http.get(PRODUCTS_URL, () => HttpResponse.json(productsPage(items))),
      http.post(PRODUCTS_URL, async ({ request }) => {
        const body = (await request.json()) as {
          label: string;
          name: string;
          description?: string;
        };
        const created: ProductRead = {
          label: body.label,
          name: body.name,
          description: body.description ?? null,
          created_at: "2026-06-06T00:00:00Z",
          updated_at: "2026-06-06T00:00:00Z",
        };
        items = [...items, created];
        return HttpResponse.json(created, { status: 201 });
      }),
    );
    render(<ProductCatalogueClient />, { wrapper });

    await waitFor(() =>
      expect(screen.getByText(/no products yet/i)).toBeInTheDocument(),
    );

    await user.click(screen.getByRole("button", { name: /new product/i }));
    await user.type(screen.getByLabelText(/label/i), "basic_tier");
    await user.type(screen.getByLabelText(/^name$/i), "Basic Tier");
    await user.click(screen.getByRole("button", { name: /create product/i }));

    await waitFor(() =>
      expect(screen.getByText("Basic Tier")).toBeInTheDocument(),
    );
    expect(screen.getByText("basic_tier")).toBeInTheDocument();
  });
});
