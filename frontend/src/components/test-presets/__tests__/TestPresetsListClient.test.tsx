import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { http, HttpResponse } from "msw";
import type { ReactNode } from "react";
import { describe, expect, it } from "vitest";

import { TEST_PRESET_EXAMPLES } from "@/components/test-presets/TestPresetExamples";
import { TestPresetsListClient } from "@/components/test-presets/TestPresetsListClient";
import { API_BASE } from "@/lib/api/client";
import type { TestPresetRead } from "@/lib/api/test-presets";
import { server } from "@/test/mocks/server";

function wrapper({ children }: { children: ReactNode }) {
  const client = new QueryClient({
    defaultOptions: { queries: { retry: false }, mutations: { retry: false } },
  });
  return <QueryClientProvider client={client}>{children}</QueryClientProvider>;
}

const PRESETS_URL = `${API_BASE}/api/v1/test-presets`;

const sample: TestPresetRead = {
  slug: "mobile-paywall",
  name: "Mobile paywall",
  kind: "rule",
  payload: { feature_type: "html", device_type: "mobile", path: "/article" },
  created_at: "2026-06-06T00:00:00Z",
  updated_at: "2026-06-06T00:00:00Z",
};

function presetsPage(items: TestPresetRead[], total = items.length) {
  return { items, page: 1, page_size: 20, total };
}

describe("TestPresetsListClient", () => {
  it("renders the empty state when there are no presets", async () => {
    server.use(http.get(PRESETS_URL, () => HttpResponse.json(presetsPage([]))));
    render(<TestPresetsListClient />, { wrapper });

    await waitFor(() =>
      expect(screen.getByText(/no test presets yet/i)).toBeInTheDocument(),
    );
  });

  it("renders a card with the name, slug, kind badge, and summary", async () => {
    server.use(
      http.get(PRESETS_URL, () => HttpResponse.json(presetsPage([sample]))),
    );
    render(<TestPresetsListClient />, { wrapper });

    // Exact name so the "Mobile paywall test" starter-example card (also a
    // heading) doesn't collide with the preset card's "Mobile paywall".
    await waitFor(() =>
      expect(
        screen.getByRole("heading", { name: "Mobile paywall" }),
      ).toBeInTheDocument(),
    );
    expect(screen.getByText("mobile-paywall")).toBeInTheDocument();
    expect(screen.getByText("rule")).toBeInTheDocument();
    // Summary surfaces salient payload fields.
    expect(screen.getByText(/mobile · \/article/)).toBeInTheDocument();
  });

  it("opens a pre-filled create modal when a starter template is used", async () => {
    const user = userEvent.setup();
    server.use(http.get(PRESETS_URL, () => HttpResponse.json(presetsPage([]))));
    render(<TestPresetsListClient />, { wrapper });

    await waitFor(() =>
      expect(screen.getByText(/no test presets yet/i)).toBeInTheDocument(),
    );

    const example = TEST_PRESET_EXAMPLES[0];
    const buttons = screen.getAllByRole("button", {
      name: /use this template/i,
    });
    await user.click(buttons[0]);

    const dialog = await screen.findByRole("dialog");
    expect(within(dialog).getByLabelText(/^slug$/i)).toHaveValue(example.slug);
    expect(within(dialog).getByLabelText(/^name$/i)).toHaveValue(example.name);
    expect(within(dialog).getByLabelText(/payload/i)).toHaveValue(
      JSON.stringify(example.payload, null, 2),
    );
  });

  it("renders an error banner with retry when the request fails", async () => {
    server.use(
      http.get(PRESETS_URL, () =>
        HttpResponse.json(
          { error: { code: "INTERNAL_ERROR", message: "boom" } },
          { status: 500 },
        ),
      ),
    );
    render(<TestPresetsListClient />, { wrapper });

    await waitFor(() => expect(screen.getByRole("alert")).toBeInTheDocument());
    expect(screen.getByRole("button", { name: /retry/i })).toBeInTheDocument();
  });
});
