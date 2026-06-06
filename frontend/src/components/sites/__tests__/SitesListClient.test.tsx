import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { render, screen, waitFor } from "@testing-library/react";
import { http, HttpResponse } from "msw";
import type { ReactNode } from "react";
import { describe, expect, it } from "vitest";

import { SitesListClient } from "@/components/sites/SitesListClient";
import { API_BASE } from "@/lib/api/client";
import type { SiteRead } from "@/lib/api/sites";
import { server } from "@/test/mocks/server";

function wrapper({ children }: { children: ReactNode }) {
  const client = new QueryClient({
    defaultOptions: { queries: { retry: false }, mutations: { retry: false } },
  });
  return (
    <QueryClientProvider client={client}>{children}</QueryClientProvider>
  );
}

const SITES_URL = `${API_BASE}/api/v1/sites`;

const sampleSite: SiteRead = {
  slug: "demo-localhost",
  name: "Demo (localhost:9000)",
  source_protocol: "http",
  source_host: "localhost",
  source_port: 9000,
  dest_protocol: "http",
  dest_host: "demo-upstream",
  dest_port: 8081,
  headers: {},
  created_at: "2026-06-06T00:00:00Z",
  updated_at: "2026-06-06T00:00:00Z",
};

function sitesPage(items: SiteRead[], total = items.length) {
  return { items, page: 1, page_size: 20, total };
}

describe("SitesListClient", () => {
  it("renders the empty state when there are no sites", async () => {
    server.use(http.get(SITES_URL, () => HttpResponse.json(sitesPage([]))));
    render(<SitesListClient />, { wrapper });

    await waitFor(() =>
      expect(screen.getByText(/no sites yet/i)).toBeInTheDocument(),
    );
  });

  it("renders a card with the source → destination routing", async () => {
    server.use(
      http.get(SITES_URL, () => HttpResponse.json(sitesPage([sampleSite]))),
    );
    render(<SitesListClient />, { wrapper });

    await waitFor(() =>
      expect(
        screen.getByRole("heading", { name: /demo \(localhost:9000\)/i }),
      ).toBeInTheDocument(),
    );
    expect(screen.getByText("http://localhost:9000")).toBeInTheDocument();
    expect(screen.getByText("http://demo-upstream:8081")).toBeInTheDocument();
    expect(screen.getByText("demo-localhost")).toBeInTheDocument();
    // No custom-headers indicator when the map is empty.
    expect(screen.queryByText(/header/i)).not.toBeInTheDocument();
  });

  it("shows a custom-headers count when the site has headers", async () => {
    const withHeaders: SiteRead = {
      ...sampleSite,
      headers: { "X-A": "1", "X-B": "2" },
    };
    server.use(
      http.get(SITES_URL, () => HttpResponse.json(sitesPage([withHeaders]))),
    );
    render(<SitesListClient />, { wrapper });

    await waitFor(() =>
      expect(screen.getByText("2 headers")).toBeInTheDocument(),
    );
  });

  it("renders an error banner with retry when the request fails", async () => {
    server.use(
      http.get(SITES_URL, () =>
        HttpResponse.json(
          { error: { code: "INTERNAL_ERROR", message: "boom" } },
          { status: 500 },
        ),
      ),
    );
    render(<SitesListClient />, { wrapper });

    await waitFor(() =>
      expect(screen.getByRole("alert")).toBeInTheDocument(),
    );
    expect(
      screen.getByRole("button", { name: /retry/i }),
    ).toBeInTheDocument();
  });
});
