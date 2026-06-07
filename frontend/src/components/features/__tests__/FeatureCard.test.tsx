import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { http, HttpResponse } from "msw";
import type { ReactNode } from "react";
import { describe, expect, it, vi } from "vitest";

import { FeatureCard } from "@/components/features/FeatureCard";
import { API_BASE } from "@/lib/api/client";
import type { FeatureRead } from "@/lib/api/features";
import { server } from "@/test/mocks/server";

function wrapper({ children }: { children: ReactNode }) {
  const client = new QueryClient({
    defaultOptions: { queries: { retry: false }, mutations: { retry: false } },
  });
  return (
    <QueryClientProvider client={client}>{children}</QueryClientProvider>
  );
}

const feature: FeatureRead = {
  id: "demo-article",
  name: "Article Paywall",
  type: "html",
  execution_order: 3,
  staging_version_id: null,
  live_version_id: null,
  created_at: "2026-05-31T00:00:00Z",
  updated_at: "2026-05-31T00:00:00Z",
};

const PATCH_URL = `${API_BASE}/api/v1/features/${feature.id}`;

describe("FeatureCard", () => {
  it("shows the execution number and links to the version list", () => {
    render(<FeatureCard feature={feature} />, { wrapper });
    expect(screen.getByRole("spinbutton", { name: /execution order/i })).toHaveValue(
      3,
    );
    expect(
      screen.getByRole("link", { name: /article paywall/i }),
    ).toHaveAttribute("href", "/products/features/html/demo-article");
  });

  it("PATCHes the typed order on Enter, committing only the final value", async () => {
    const user = userEvent.setup();
    let receivedBody: unknown = null;
    server.use(
      http.patch(PATCH_URL, async ({ request }) => {
        receivedBody = await request.json();
        return HttpResponse.json({ ...feature, execution_order: 7 });
      }),
    );

    render(<FeatureCard feature={feature} />, { wrapper });
    const input = screen.getByRole("spinbutton", { name: /execution order/i });
    await user.clear(input);
    await user.type(input, "7");
    // Not committed yet on keystroke.
    expect(receivedBody).toBeNull();
    await user.keyboard("{Enter}");

    await waitFor(() =>
      expect(receivedBody).toEqual({ execution_order: 7 }),
    );
  });

  it("PATCHes the typed order on blur", async () => {
    const user = userEvent.setup();
    let receivedBody: unknown = null;
    server.use(
      http.patch(PATCH_URL, async ({ request }) => {
        receivedBody = await request.json();
        return HttpResponse.json({ ...feature, execution_order: 5 });
      }),
    );

    render(<FeatureCard feature={feature} />, { wrapper });
    const input = screen.getByRole("spinbutton", { name: /execution order/i });
    await user.clear(input);
    await user.type(input, "5");
    await user.tab(); // blur

    await waitFor(() =>
      expect(receivedBody).toEqual({ execution_order: 5 }),
    );
  });

  it("does not PATCH when the value is unchanged", async () => {
    const user = userEvent.setup();
    const onPatch = vi.fn();
    server.use(
      http.patch(PATCH_URL, () => {
        onPatch();
        return HttpResponse.json(feature);
      }),
    );

    render(<FeatureCard feature={feature} />, { wrapper });
    const input = screen.getByRole("spinbutton", { name: /execution order/i });
    await user.click(input);
    await user.keyboard("{Enter}");

    expect(onPatch).not.toHaveBeenCalled();
  });

  it("surfaces the 409 EXECUTION_ORDER_CONFLICT as an inline message", async () => {
    const user = userEvent.setup();
    server.use(
      http.patch(PATCH_URL, () =>
        HttpResponse.json(
          {
            error: {
              code: "EXECUTION_ORDER_CONFLICT",
              message: "duplicate order",
            },
          },
          { status: 409 },
        ),
      ),
    );

    render(<FeatureCard feature={feature} />, { wrapper });
    const input = screen.getByRole("spinbutton", { name: /execution order/i });
    await user.clear(input);
    await user.type(input, "1");
    await user.keyboard("{Enter}");

    await waitFor(() =>
      expect(
        screen.getByText(/order 1 is already used by another html rule/i),
      ).toBeInTheDocument(),
    );
  });

  it("does not navigate when editing the order (stops propagation)", async () => {
    const user = userEvent.setup();
    const parentClick = vi.fn();
    server.use(
      http.patch(PATCH_URL, () =>
        HttpResponse.json({ ...feature, execution_order: 4 }),
      ),
    );

    // The order input sits inside the card link, under a parent click
    // listener; its handlers stopPropagation so the listener never fires and
    // the link never navigates when the input is used.
    render(
      <div onClick={parentClick}>
        <FeatureCard feature={feature} />
      </div>,
      { wrapper },
    );

    const input = screen.getByRole("spinbutton", { name: /execution order/i });
    await user.click(input);
    await user.clear(input);
    await user.type(input, "4");
    await user.keyboard("{Enter}");

    expect(parentClick).not.toHaveBeenCalled();
  });
});
