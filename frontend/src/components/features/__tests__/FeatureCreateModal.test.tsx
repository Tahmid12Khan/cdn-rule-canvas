import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { http, HttpResponse } from "msw";
import type { ReactNode } from "react";
import { describe, expect, it, vi } from "vitest";

import { FeatureCreateModal } from "@/components/features/FeatureCreateModal";
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

const FEATURES_URL = `${API_BASE}/api/v1/features`;

async function openModal(user: ReturnType<typeof userEvent.setup>) {
  await user.click(
    screen.getByRole("button", { name: /add a new feature/i }),
  );
  await screen.findByRole("dialog");
}

describe("FeatureCreateModal", () => {
  it("shows validation errors and does not submit on invalid input", async () => {
    const user = userEvent.setup();
    const onPost = vi.fn();
    server.use(
      http.post(FEATURES_URL, () => {
        onPost();
        return HttpResponse.json({}, { status: 201 });
      }),
    );

    render(<FeatureCreateModal />, { wrapper });
    await openModal(user);

    // invalid slug (uppercase) + empty name.
    await user.type(screen.getByLabelText(/slug/i), "Bad_Slug");
    await user.click(
      screen.getByRole("button", { name: /create feature/i }),
    );

    await waitFor(() =>
      expect(screen.getByText(/lowercase kebab-case/i)).toBeInTheDocument(),
    );
    expect(screen.getByText(/name is required/i)).toBeInTheDocument();
    expect(onPost).not.toHaveBeenCalled();
  });

  it("submits valid input and closes the modal", async () => {
    const user = userEvent.setup();
    let receivedBody: unknown = null;
    server.use(
      http.post(FEATURES_URL, async ({ request }) => {
        receivedBody = await request.json();
        return HttpResponse.json(
          {
            id: "demo-article",
            name: "Article Paywall",
            type: "html",
            execution_order: 1,
            staging_version_id: null,
            live_version_id: null,
            created_at: "2026-05-31T00:00:00Z",
            updated_at: "2026-05-31T00:00:00Z",
          },
          { status: 201 },
        );
      }),
    );

    render(<FeatureCreateModal />, { wrapper });
    await openModal(user);

    await user.type(screen.getByLabelText(/slug/i), "demo-article");
    await user.type(screen.getByLabelText(/name/i), "Article Paywall");
    await user.click(
      screen.getByRole("button", { name: /create feature/i }),
    );

    await waitFor(() =>
      expect(screen.queryByRole("dialog")).not.toBeInTheDocument(),
    );
    expect(receivedBody).toEqual({
      id: "demo-article",
      name: "Article Paywall",
      type: "html",
    });
  });

  it("surfaces a slug-conflict error from the backend", async () => {
    const user = userEvent.setup();
    server.use(
      http.post(FEATURES_URL, () =>
        HttpResponse.json(
          {
            error: { code: "SLUG_CONFLICT", message: "duplicate slug" },
          },
          { status: 409 },
        ),
      ),
    );

    render(<FeatureCreateModal />, { wrapper });
    await openModal(user);

    await user.type(screen.getByLabelText(/slug/i), "demo-article");
    await user.type(screen.getByLabelText(/name/i), "Article Paywall");
    await user.click(
      screen.getByRole("button", { name: /create feature/i }),
    );

    await waitFor(() =>
      expect(
        screen.getByText(/that slug is already taken/i),
      ).toBeInTheDocument(),
    );
    // dialog stays open so the user can fix the slug.
    expect(screen.getByRole("dialog")).toBeInTheDocument();
  });
});
