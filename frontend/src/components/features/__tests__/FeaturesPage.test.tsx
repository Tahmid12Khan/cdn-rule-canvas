import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { render, screen, waitFor } from "@testing-library/react";
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
  return { items, page: 1, page_size: 20, total };
}

const sampleFeature = {
  id: "dn-article",
  name: "Article Paywall",
  type: "html",
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
      "/products/features/html/dn-article",
    );
    // live badge present since live_version_id is set.
    expect(screen.getByText(/^live$/i)).toBeInTheDocument();
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
