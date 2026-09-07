import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { http, HttpResponse } from "msw";
import type { ReactNode } from "react";
import { describe, expect, it, vi } from "vitest";

import { SavedOutcomeEditorPage } from "@/components/outcomes-library/SavedOutcomeEditorPage";
import { API_BASE } from "@/lib/api/client";
import type { SavedOutcomeRead } from "@/lib/api/savedOutcomes";
import { server } from "@/test/mocks/server";

vi.mock("next/navigation", () => ({
  useRouter: () => ({ push: vi.fn() }),
}));

function wrapper({ children }: { children: ReactNode }) {
  const client = new QueryClient({
    defaultOptions: { queries: { retry: false }, mutations: { retry: false } },
  });
  return <QueryClientProvider client={client}>{children}</QueryClientProvider>;
}

const OUTCOMES_URL = `${API_BASE}/api/v1/saved-outcomes`;
const COMPONENTS_URL = `${API_BASE}/api/v1/component-templates`;

const OUTCOME_ID = "11111111-1111-1111-1111-111111111111";
const COMPONENT_ID = "22222222-2222-2222-2222-222222222222";
const SLUG = "promo-banner-default";

const outcome: SavedOutcomeRead = {
  id: OUTCOME_ID,
  slug: SLUG,
  name: "Promo Banner (Default)",
  component_id: COMPONENT_ID,
  component_name: "Promo Banner",
  version_number: null,
  variables: { headline: "Sale" },
  created_at: "2026-06-06T00:00:00Z",
  updated_at: "2026-06-06T00:00:00Z",
};

function mockEndpoints() {
  server.use(
    http.get(OUTCOMES_URL, () =>
      HttpResponse.json({ items: [outcome], page: 1, page_size: 200, total: 1 }),
    ),
    http.get(COMPONENTS_URL, () =>
      HttpResponse.json({
        items: [
          {
            id: COMPONENT_ID,
            slug: "promo-banner",
            name: "Promo Banner",
            description: null,
            default_version_number: 2,
            latest_version_number: 2,
            updated_at: "2026-06-06T00:00:00Z",
          },
        ],
        page: 1,
        page_size: 100,
        total: 1,
      }),
    ),
    http.get(`${COMPONENTS_URL}/${COMPONENT_ID}`, () =>
      HttpResponse.json({
        id: COMPONENT_ID,
        slug: "promo-banner",
        name: "Promo Banner",
        description: null,
        default_mode: "latest",
        default_version_number: 2,
        latest_version_number: 2,
        versions: [
          {
            id: "v1",
            version_number: 1,
            description: null,
            is_default: false,
            created_at: "2026-06-06T00:00:00Z",
            updated_at: "2026-06-06T00:00:00Z",
          },
          {
            id: "v2",
            version_number: 2,
            description: null,
            is_default: true,
            created_at: "2026-06-06T00:00:00Z",
            updated_at: "2026-06-06T00:00:00Z",
          },
        ],
        created_at: "2026-06-06T00:00:00Z",
        updated_at: "2026-06-06T00:00:00Z",
      }),
    ),
    http.get(`${COMPONENTS_URL}/${COMPONENT_ID}/resolve`, () =>
      HttpResponse.json({
        version_number: 2,
        html_body: "<h1>{{headline}}</h1>",
        variables: [{ name: "headline", title: "Headline" }],
      }),
    ),
  );
}

describe("SavedOutcomeEditorPage", () => {
  it("resolves the slug, renders variables prefilled, and updates the preview", async () => {
    mockEndpoints();
    const user = userEvent.setup();
    render(<SavedOutcomeEditorPage slug={SLUG} />, { wrapper });

    await waitFor(() =>
      expect(screen.getByText("Promo Banner (Default)")).toBeInTheDocument(),
    );

    const headline = (await screen.findByLabelText(
      "Headline",
    )) as HTMLInputElement;
    expect(headline.value).toBe("Sale");

    await user.clear(headline);
    await user.type(headline, "Big Sale");

    // The preview renders into a srcdoc iframe (jsdom doesn't parse srcdoc).
    await waitFor(() =>
      expect(
        screen.getByTitle("Component preview").getAttribute("srcdoc"),
      ).toContain("<h1>Big Sale</h1>"),
    );
  });

  it("saves the version + variables via PATCH", async () => {
    mockEndpoints();
    let patchBody: unknown = null;
    server.use(
      http.patch(`${OUTCOMES_URL}/${OUTCOME_ID}`, async ({ request }) => {
        patchBody = await request.json();
        return HttpResponse.json({
          ...outcome,
          name: "Renamed Banner",
          variables: { headline: "Big Sale" },
        });
      }),
    );
    const user = userEvent.setup();
    render(<SavedOutcomeEditorPage slug={SLUG} />, { wrapper });

    const headline = (await screen.findByLabelText(
      "Headline",
    )) as HTMLInputElement;
    await user.clear(headline);
    await user.type(headline, "Big Sale");

    const nameInput = screen.getByLabelText(/name/i) as HTMLInputElement;
    await user.clear(nameInput);
    await user.type(nameInput, "Renamed Banner");

    await user.click(screen.getByRole("button", { name: /^save$/i }));

    await waitFor(() =>
      expect(patchBody).toMatchObject({
        name: "Renamed Banner",
        version_number: null,
        variables: { headline: "Big Sale" },
      }),
    );
  });
});
