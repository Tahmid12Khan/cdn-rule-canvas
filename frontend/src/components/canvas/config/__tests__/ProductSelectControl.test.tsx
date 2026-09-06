import { screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { http, HttpResponse } from "msw";
import { describe, expect, it, vi } from "vitest";

import { ProductSelectControl } from "@/components/canvas/config/ProductSelectControl";
import { API_BASE } from "@/lib/api/client";
import type { ProductRead } from "@/lib/api/products";
import { renderWithQuery } from "@/test/renderWithQuery";
import { server } from "@/test/mocks/server";

const PRODUCTS_URL = `${API_BASE}/api/v1/products`;

const product: ProductRead = {
  label: "premium_tier",
  name: "Premium Tier",
  description: "Full access",
  created_at: "2026-06-06T00:00:00Z",
  updated_at: "2026-06-06T00:00:00Z",
};

describe("ProductSelectControl", () => {
  it("opens on focus, searches, and stores the selected label", async () => {
    const user = userEvent.setup();
    const onChange = vi.fn();
    server.use(
      http.get(PRODUCTS_URL, () =>
        HttpResponse.json({ items: [product], page: 1, page_size: 10, total: 1 }),
      ),
    );

    renderWithQuery(
      <ProductSelectControl id="field-product" value="" onChange={onChange} />,
    );

    await user.click(screen.getByRole("combobox"));
    await waitFor(() =>
      expect(screen.getByText("Premium Tier")).toBeInTheDocument(),
    );

    // Each option is a <li role="option"> wrapping a <button>; the click
    // handler lives on the button.
    await user.click(screen.getByRole("button", { name: /premium/i }));
    expect(onChange).toHaveBeenCalledWith("premium_tier");
  });

  it("resolves the selected label to 'name (label)' and clears it", async () => {
    const user = userEvent.setup();
    const onChange = vi.fn();
    server.use(
      http.get(`${PRODUCTS_URL}/:label`, () => HttpResponse.json(product)),
    );

    renderWithQuery(
      <ProductSelectControl
        id="field-product"
        value="premium_tier"
        onChange={onChange}
      />,
    );

    await waitFor(() =>
      expect(
        screen.getByText("Premium Tier (premium_tier)"),
      ).toBeInTheDocument(),
    );
    await user.click(screen.getByRole("button", { name: /clear/i }));
    expect(onChange).toHaveBeenCalledWith("");
  });

  it("falls back to the bare label while the product is unresolved", () => {
    const onChange = vi.fn();
    server.use(
      http.get(`${PRODUCTS_URL}/:label`, () =>
        HttpResponse.json(
          { error: { code: "NOT_FOUND", message: "gone" } },
          { status: 404 },
        ),
      ),
    );

    renderWithQuery(
      <ProductSelectControl
        id="field-product"
        value="ghost_product"
        onChange={onChange}
      />,
    );

    expect(screen.getByText("ghost_product")).toBeInTheDocument();
  });
});
