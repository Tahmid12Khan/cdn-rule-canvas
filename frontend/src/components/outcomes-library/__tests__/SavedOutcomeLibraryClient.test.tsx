import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { http, HttpResponse } from "msw";
import type { ReactNode } from "react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import { SavedOutcomeLibraryClient } from "@/components/outcomes-library/SavedOutcomeLibraryClient";
import { API_BASE } from "@/lib/api/client";
import type { SavedOutcomeRead } from "@/lib/api/savedOutcomes";
import { server } from "@/test/mocks/server";

const push = vi.fn();
vi.mock("next/navigation", () => ({
  useRouter: () => ({ push }),
}));

function wrapper({ children }: { children: ReactNode }) {
  const client = new QueryClient({
    defaultOptions: { queries: { retry: false }, mutations: { retry: false } },
  });
  return <QueryClientProvider client={client}>{children}</QueryClientProvider>;
}

const OUTCOMES_URL = `${API_BASE}/api/v1/saved-outcomes`;
const COMPONENTS_URL = `${API_BASE}/api/v1/component-templates`;

const COMPONENT_ID = "22222222-2222-2222-2222-222222222222";

const sampleOutcome: SavedOutcomeRead = {
  id: "11111111-1111-1111-1111-111111111111",
  slug: "promo-banner-default",
  name: "Promo Banner (Default)",
  component_id: COMPONENT_ID,
  component_name: "Promo Banner",
  version_number: null,
  variables: { headline: "Sale" },
  created_at: "2026-06-06T00:00:00Z",
  updated_at: "2026-06-06T00:00:00Z",
};

function outcomesPage(items: SavedOutcomeRead[], total = items.length) {
  return { items, page: 1, page_size: 20, total };
}

function mockComponentsList() {
  server.use(
    http.get(COMPONENTS_URL, () =>
      HttpResponse.json({
        items: [
          {
            id: COMPONENT_ID,
            slug: "promo-banner",
            name: "Promo Banner",
            description: null,
            default_version_number: 1,
            latest_version_number: 1,
            updated_at: "2026-06-06T00:00:00Z",
          },
        ],
        page: 1,
        page_size: 100,
        total: 1,
      }),
    ),
  );
}

beforeEach(() => {
  push.mockClear();
});

describe("SavedOutcomeLibraryClient", () => {
  it("renders the empty state when there are no outcomes", async () => {
    server.use(
      http.get(OUTCOMES_URL, () => HttpResponse.json(outcomesPage([]))),
    );
    render(<SavedOutcomeLibraryClient />, { wrapper });

    await waitFor(() =>
      expect(screen.getByText(/no outcomes yet/i)).toBeInTheDocument(),
    );
  });

  it("renders a card with name, component, and version badge", async () => {
    server.use(
      http.get(OUTCOMES_URL, () =>
        HttpResponse.json(outcomesPage([sampleOutcome])),
      ),
    );
    render(<SavedOutcomeLibraryClient />, { wrapper });

    await waitFor(() =>
      expect(screen.getByText("Promo Banner (Default)")).toBeInTheDocument(),
    );
    expect(screen.getByText("Promo Banner")).toBeInTheDocument();
    expect(screen.getByText("Latest")).toBeInTheDocument();
  });

  it("sends the search query to the saved-outcomes endpoint", async () => {
    const user = userEvent.setup();
    let lastQ: string | null = null;
    server.use(
      http.get(OUTCOMES_URL, ({ request }) => {
        lastQ = new URL(request.url).searchParams.get("q");
        return HttpResponse.json(outcomesPage([sampleOutcome]));
      }),
    );
    render(<SavedOutcomeLibraryClient />, { wrapper });

    await waitFor(() =>
      expect(screen.getByText("Promo Banner (Default)")).toBeInTheDocument(),
    );

    await user.type(screen.getByLabelText(/search outcomes/i), "promo");

    await waitFor(() => expect(lastQ).toBe("promo"));
  });

  it("creates an outcome via the modal and navigates to its editor", async () => {
    const user = userEvent.setup();
    mockComponentsList();
    server.use(
      http.get(OUTCOMES_URL, () => HttpResponse.json(outcomesPage([]))),
      http.post(OUTCOMES_URL, async ({ request }) => {
        const body = (await request.json()) as {
          slug: string;
          name: string;
          component_id: string;
        };
        const created: SavedOutcomeRead = {
          id: "33333333-3333-3333-3333-333333333333",
          slug: body.slug,
          name: body.name,
          component_id: body.component_id,
          component_name: "Promo Banner",
          version_number: null,
          variables: {},
          created_at: "2026-06-06T00:00:00Z",
          updated_at: "2026-06-06T00:00:00Z",
        };
        return HttpResponse.json(created, { status: 201 });
      }),
    );
    render(<SavedOutcomeLibraryClient />, { wrapper });

    await waitFor(() =>
      expect(screen.getByText(/no outcomes yet/i)).toBeInTheDocument(),
    );

    await user.click(screen.getAllByRole("button", { name: /new outcome/i })[0]);
    await waitFor(() =>
      expect(screen.getByRole("option", { name: "Promo Banner" })).toBeInTheDocument(),
    );
    await user.type(screen.getByLabelText(/slug/i), "new-promo");
    await user.type(screen.getByLabelText(/^name$/i), "New Promo");
    await user.selectOptions(
      screen.getByLabelText(/component/i),
      COMPONENT_ID,
    );
    await user.click(screen.getByRole("button", { name: /create outcome/i }));

    await waitFor(() =>
      expect(push).toHaveBeenCalledWith("/products/outcomes/new-promo"),
    );
  });
});
