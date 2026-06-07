import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { render, screen, waitFor, within } from "@testing-library/react";
import { http, HttpResponse } from "msw";
import type { ReactNode } from "react";
import { describe, expect, it } from "vitest";

import { FeaturesListClient } from "@/components/features/FeaturesListClient";
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

function featurePage(items: unknown[], total = items.length) {
  return { items, page: 1, page_size: 100, total };
}

const sampleFeature = {
  id: "demo-article",
  name: "Article Paywall",
  type: "html",
  execution_order: 1,
  staging_version_id: null,
  live_version_id: "11111111-1111-1111-1111-111111111111",
  created_at: "2026-05-31T00:00:00Z",
  updated_at: "2026-05-31T00:00:00Z",
};

describe("FeaturesListClient", () => {
  it("renders the empty state when there are no features", async () => {
    server.use(
      http.get(FEATURES_URL, () => HttpResponse.json(featurePage([]))),
    );
    render(<FeaturesListClient />, { wrapper });

    await waitFor(() =>
      expect(screen.getByText(/no features yet/i)).toBeInTheDocument(),
    );
  });

  it("renders feature cards when the list is populated", async () => {
    server.use(
      http.get(FEATURES_URL, () =>
        HttpResponse.json(featurePage([sampleFeature])),
      ),
    );
    render(<FeaturesListClient />, { wrapper });

    await waitFor(() =>
      expect(
        screen.getByRole("heading", { name: /article paywall/i }),
      ).toBeInTheDocument(),
    );
    // links to the version list page for this feature.
    const link = screen.getByRole("link", { name: /article paywall/i });
    expect(link).toHaveAttribute(
      "href",
      "/products/features/html/demo-article",
    );
    // live badge present since live_version_id is set.
    expect(screen.getByText(/^live$/i)).toBeInTheDocument();
  });

  it("splits features into HTML and JSON sections, each ordered by execution_order", async () => {
    const features = [
      { ...sampleFeature, id: "html-b", name: "HTML B", execution_order: 2 },
      { ...sampleFeature, id: "html-a", name: "HTML A", execution_order: 1 },
      {
        ...sampleFeature,
        id: "json-b",
        name: "JSON B",
        type: "json",
        execution_order: 2,
      },
      {
        ...sampleFeature,
        id: "json-a",
        name: "JSON A",
        type: "json",
        execution_order: 1,
      },
    ];
    server.use(
      http.get(FEATURES_URL, () => HttpResponse.json(featurePage(features))),
    );
    render(<FeaturesListClient />, { wrapper });

    await screen.findByRole("heading", { name: /html rules/i });

    // HTML section comes before JSON in the document.
    const sections = screen.getAllByRole("heading", { level: 2 });
    expect(sections.map((h) => h.textContent)).toEqual([
      "HTML Rules",
      "JSON Rules",
    ]);

    // Within each section the cards are sorted execution_order ASC.
    const headings = screen.getAllByRole("heading", { level: 3 });
    expect(headings.map((h) => h.textContent)).toEqual([
      "HTML A",
      "HTML B",
      "JSON A",
      "JSON B",
    ]);

    // Execution number shown per card (editable order input).
    const htmlA = screen.getByRole("link", { name: /html a/i });
    expect(
      within(htmlA).getByRole("spinbutton", { name: /execution order/i }),
    ).toHaveValue(1);
  });

  it("shows a per-section empty hint when a section has no features", async () => {
    server.use(
      http.get(FEATURES_URL, () =>
        HttpResponse.json(featurePage([sampleFeature])),
      ),
    );
    render(<FeaturesListClient />, { wrapper });

    await screen.findByRole("heading", { name: /html rules/i });
    expect(screen.getByText(/no json rules yet/i)).toBeInTheDocument();
  });

  it("renders an error banner with retry when the request fails", async () => {
    server.use(
      http.get(FEATURES_URL, () =>
        HttpResponse.json(
          { error: { code: "INTERNAL_ERROR", message: "boom" } },
          { status: 500 },
        ),
      ),
    );
    render(<FeaturesListClient />, { wrapper });

    await waitFor(() =>
      expect(screen.getByRole("alert")).toBeInTheDocument(),
    );
    expect(
      screen.getByRole("button", { name: /retry/i }),
    ).toBeInTheDocument();
  });
});
