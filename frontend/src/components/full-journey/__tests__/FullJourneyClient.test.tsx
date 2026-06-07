import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { http, HttpResponse } from "msw";
import type { ReactNode } from "react";
import { describe, expect, it } from "vitest";

import { FullJourneyClient } from "@/components/full-journey/FullJourneyClient";
import { API_BASE } from "@/lib/api/client";
import { PROXY_BASE } from "@/lib/api/evalTest";
import { server } from "@/test/mocks/server";

function wrapper({ children }: { children: ReactNode }) {
  const client = new QueryClient({
    defaultOptions: { queries: { retry: false }, mutations: { retry: false } },
  });
  return <QueryClientProvider client={client}>{children}</QueryClientProvider>;
}

// A skipped step shape helper.
function step(
  index: number,
  node_id: string,
  kind: string,
  label: string,
  body_after: unknown,
  time_ms = "0.00",
) {
  return { index, node_id, kind, label, branch: null, body_after, time_ms };
}

// Two CHAINED features: an HTML feature that injects a paywall, then a JSON
// feature that flips a flag. The HTML feature's start body is what the journey
// begins with; the JSON feature's last body is where it ends.
const fullJourney = {
  content_kind: "html",
  site: "demo-site",
  total_time_ms: "12.34",
  features: [
    {
      feature_id: "feat-html",
      name: "Paywall HTML",
      type: "html",
      execution_order: 1,
      version_number: null,
      matched: true,
      time_took_ms: "5.00",
      journey: [
        step(0, "start", "start", "Start", "<html><body>free</body></html>"),
        step(
          1,
          "a1",
          "expression",
          "Inject paywall",
          "<html><body>paywall</body></html>",
          "0.42",
        ),
        step(
          2,
          "end",
          "end",
          "END",
          "<html><body>paywall</body></html>",
        ),
      ],
      summary: {
        expressions: [
          {
            expression_id: "a1",
            expression_label: "add_attribute",
            custom_expression_label: "paywall_showed",
            expression_time_ms: "0.42",
          },
        ],
        time_took_ms: "5.00",
        expensive_nodes: [],
      },
    },
    {
      feature_id: "feat-json",
      name: "Entitlement JSON",
      type: "json",
      execution_order: 1,
      version_number: 3,
      matched: true,
      time_took_ms: "7.00",
      journey: [
        step(0, "start", "start", "Start", { entitled: false }),
        step(1, "j1", "expression", "Flip flag", { entitled: true }, "0.61"),
        step(2, "end", "end", "END", { entitled: true }),
      ],
      summary: null,
    },
    {
      feature_id: "feat-skip",
      name: "Regwall HTML",
      type: "html",
      execution_order: 2,
      version_number: null,
      matched: false,
      time_took_ms: "0.10",
      journey: [],
      summary: null,
    },
  ],
};

// Feature list for the version-override UI.
const featuresPage = {
  items: [
    {
      id: "feat-html",
      name: "Paywall HTML",
      type: "html",
      execution_order: 1,
      staging_version_id: null,
      live_version_id: null,
      created_at: "2026-01-01T00:00:00Z",
      updated_at: "2026-01-01T00:00:00Z",
    },
  ],
  page: 1,
  page_size: 100,
  total: 1,
};

const versionsPage = {
  items: [
    {
      id: "11111111-1111-1111-1111-111111111111",
      feature_id: "feat-html",
      version_number: 2,
      description: null,
      status: "live",
      last_updated_by: "me",
      last_updated_at: "2026-01-01T00:00:00Z",
      created_at: "2026-01-01T00:00:00Z",
    },
    {
      id: "22222222-2222-2222-2222-222222222222",
      feature_id: "feat-html",
      version_number: 5,
      description: null,
      status: "draft",
      last_updated_by: "me",
      last_updated_at: "2026-01-01T00:00:00Z",
      created_at: "2026-01-01T00:00:00Z",
    },
  ],
  page: 1,
  page_size: 100,
  total: 2,
};

function seedFeatureHandlers() {
  server.use(
    http.get(`${API_BASE}/api/v1/features`, () =>
      HttpResponse.json(featuresPage),
    ),
    http.get(`${API_BASE}/api/v1/features/feat-html/versions`, () =>
      HttpResponse.json(versionsPage),
    ),
  );
}

describe("FullJourneyClient", () => {
  it("renders all three diff levels, total time, and marks skipped features", async () => {
    const user = userEvent.setup();
    seedFeatureHandlers();
    server.use(
      http.post(`${PROXY_BASE}/__rre/eval-full-journey`, () =>
        HttpResponse.json(fullJourney),
      ),
    );

    render(<FullJourneyClient />, { wrapper });

    await user.type(
      screen.getByLabelText("Full URL"),
      "https://www.example.com/article/1",
    );
    await user.click(
      screen.getByRole("button", { name: /run full journey/i }),
    );

    // total_time_ms surfaces.
    await waitFor(() =>
      expect(screen.getByTestId("total-time")).toHaveTextContent("12.34 ms"),
    );

    // 5.3 cross-feature diff renders.
    expect(screen.getByTestId("cross-feature-diff")).toBeInTheDocument();

    // 5.2 per-feature start→end diffs: one per matched feature (2).
    expect(screen.getAllByTestId("feature-combined-diff")).toHaveLength(2);

    // 5.1 per-node diffs: each matched feature has one consecutive pair per
    // step after the first (2 features × 2 = 4 node diffs).
    expect(screen.getAllByTestId("node-diff")).toHaveLength(4);

    // The skipped feature is clearly marked.
    expect(screen.getByTestId("feature-skipped")).toHaveTextContent(
      /not applicable \/ skipped/i,
    );
  });

  it("a non-default version selection populates version_overrides in the POST body", async () => {
    const user = userEvent.setup();
    seedFeatureHandlers();
    let captured: {
      url?: string;
      env?: string;
      version_overrides?: Record<string, number>;
    } = {};
    server.use(
      http.post(`${PROXY_BASE}/__rre/eval-full-journey`, async ({ request }) => {
        captured = (await request.json()) as typeof captured;
        return HttpResponse.json(fullJourney);
      }),
    );

    render(<FullJourneyClient />, { wrapper });

    await user.type(
      screen.getByLabelText("Full URL"),
      "https://www.example.com/article/1",
    );

    // The override select populates once versions load.
    const select = await screen.findByLabelText("Version for Paywall HTML");
    // Default is "Active (live)" → no override.
    await user.selectOptions(select, "5");

    await user.click(
      screen.getByRole("button", { name: /run full journey/i }),
    );

    await waitFor(() =>
      expect(screen.getByTestId("total-time")).toBeInTheDocument(),
    );

    expect(captured.url).toBe("https://www.example.com/article/1");
    expect(captured.env).toBe("live");
    expect(captured.version_overrides).toEqual({ "feat-html": 5 });
  });

  it("reverting a version select back to Active removes the override", async () => {
    const user = userEvent.setup();
    seedFeatureHandlers();
    let captured: { version_overrides?: Record<string, number> } = {};
    server.use(
      http.post(`${PROXY_BASE}/__rre/eval-full-journey`, async ({ request }) => {
        captured = (await request.json()) as typeof captured;
        return HttpResponse.json(fullJourney);
      }),
    );

    render(<FullJourneyClient />, { wrapper });

    await user.type(screen.getByLabelText("Full URL"), "https://x.test/a");
    const select = await screen.findByLabelText("Version for Paywall HTML");
    await user.selectOptions(select, "5");
    await user.selectOptions(select, "active");

    await user.click(
      screen.getByRole("button", { name: /run full journey/i }),
    );

    await waitFor(() =>
      expect(screen.getByTestId("total-time")).toBeInTheDocument(),
    );
    expect(captured.version_overrides).toEqual({});
  });

  it("shows the cross-feature empty state when no feature matched", async () => {
    const user = userEvent.setup();
    seedFeatureHandlers();
    server.use(
      http.post(`${PROXY_BASE}/__rre/eval-full-journey`, () =>
        HttpResponse.json({
          content_kind: "html",
          site: null,
          total_time_ms: "1.00",
          features: [
            {
              ...fullJourney.features[2],
            },
          ],
        }),
      ),
    );

    render(<FullJourneyClient />, { wrapper });
    await user.type(screen.getByLabelText("Full URL"), "https://x.test/a");
    await user.click(
      screen.getByRole("button", { name: /run full journey/i }),
    );

    await waitFor(() =>
      expect(screen.getByTestId("cross-feature-empty")).toBeInTheDocument(),
    );
    expect(screen.queryByTestId("cross-feature-diff")).toBeNull();
  });

  it("disables Run until a URL is entered", async () => {
    const user = userEvent.setup();
    seedFeatureHandlers();
    render(<FullJourneyClient />, { wrapper });

    const run = screen.getByRole("button", { name: /run full journey/i });
    expect(run).toBeDisabled();
    await user.type(screen.getByLabelText("Full URL"), "https://x.test/a");
    expect(run).toBeEnabled();
  });

  it("the HTML feature's per-node diff shows the injected paywall change", async () => {
    const user = userEvent.setup();
    seedFeatureHandlers();
    server.use(
      http.post(`${PROXY_BASE}/__rre/eval-full-journey`, () =>
        HttpResponse.json(fullJourney),
      ),
    );

    render(<FullJourneyClient />, { wrapper });
    await user.type(screen.getByLabelText("Full URL"), "https://x.test/a");
    await user.click(
      screen.getByRole("button", { name: /run full journey/i }),
    );

    const htmlResult = await waitFor(() => {
      const results = screen.getAllByTestId("feature-result");
      const match = results.find(
        (r) => r.getAttribute("data-feature-id") === "feat-html",
      );
      if (!match) throw new Error("html feature not rendered yet");
      return match;
    });

    // The diffs added the "paywall" body line (rendered in the DiffView). The
    // line is one node ("<html><body>paywall</body></html>"); match it by
    // substring so the assertion isn't confused by the "Inject paywall" labels.
    const paywallLines = within(htmlResult)
      .getAllByText((_content, el) =>
        Boolean(el?.textContent?.includes("<body>paywall</body>")),
      )
      .filter((el) => el.tagName === "DIV");
    expect(paywallLines.length).toBeGreaterThan(0);
  });
});
