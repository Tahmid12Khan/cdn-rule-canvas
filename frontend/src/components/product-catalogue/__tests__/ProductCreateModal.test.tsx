import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { http, HttpResponse } from "msw";
import type { ReactNode } from "react";
import { describe, expect, it, vi } from "vitest";

import { ProductCreateModal } from "@/components/product-catalogue/ProductCreateModal";
import { API_BASE } from "@/lib/api/client";
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

async function openModal(user: ReturnType<typeof userEvent.setup>) {
  await user.click(screen.getByRole("button", { name: /new product/i }));
  await screen.findByRole("dialog");
}

describe("ProductCreateModal", () => {
  it("shows validation errors and does not submit on invalid input", async () => {
    const user = userEvent.setup();
    const onPost = vi.fn();
    server.use(
      http.post(PRODUCTS_URL, () => {
        onPost();
        return HttpResponse.json({}, { status: 201 });
      }),
    );

    render(<ProductCreateModal />, { wrapper });
    await openModal(user);

    // invalid label (uppercase/kebab) + empty name.
    await user.type(screen.getByLabelText(/label/i), "Bad-Label");
    await user.click(
      screen.getByRole("button", { name: /create product/i }),
    );

    await waitFor(() =>
      expect(screen.getByText(/lowercase snake_case/i)).toBeInTheDocument(),
    );
    expect(screen.getByText(/name is required/i)).toBeInTheDocument();
    expect(onPost).not.toHaveBeenCalled();
  });

  it("submits valid input and closes the modal", async () => {
    const user = userEvent.setup();
    let receivedBody: unknown = null;
    server.use(
      http.post(PRODUCTS_URL, async ({ request }) => {
        receivedBody = await request.json();
        return HttpResponse.json(
          {
            label: "premium_tier",
            name: "Premium Tier",
            description: null,
            created_at: "2026-06-06T00:00:00Z",
            updated_at: "2026-06-06T00:00:00Z",
          },
          { status: 201 },
        );
      }),
    );

    render(<ProductCreateModal />, { wrapper });
    await openModal(user);

    await user.type(screen.getByLabelText(/label/i), "premium_tier");
    await user.type(screen.getByLabelText(/^name$/i), "Premium Tier");
    await user.click(
      screen.getByRole("button", { name: /create product/i }),
    );

    await waitFor(() =>
      expect(screen.queryByRole("dialog")).not.toBeInTheDocument(),
    );
    expect(receivedBody).toEqual({
      label: "premium_tier",
      name: "Premium Tier",
    });
  });

  it("surfaces a slug-conflict error from the backend on the label field", async () => {
    const user = userEvent.setup();
    server.use(
      http.post(PRODUCTS_URL, () =>
        HttpResponse.json(
          {
            error: {
              code: "SLUG_CONFLICT",
              message: "A product with this label or name already exists",
            },
          },
          { status: 409 },
        ),
      ),
    );

    render(<ProductCreateModal />, { wrapper });
    await openModal(user);

    await user.type(screen.getByLabelText(/label/i), "premium_tier");
    await user.type(screen.getByLabelText(/^name$/i), "Premium Tier");
    await user.click(
      screen.getByRole("button", { name: /create product/i }),
    );

    await waitFor(() =>
      expect(
        screen.getByText(/this label or name already exists/i),
      ).toBeInTheDocument(),
    );
    // dialog stays open so the user can fix the label.
    expect(screen.getByRole("dialog")).toBeInTheDocument();
  });
});
