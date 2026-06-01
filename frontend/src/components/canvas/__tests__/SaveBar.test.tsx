import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { http, HttpResponse } from "msw";
import type { ReactNode } from "react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import { SaveBar } from "@/components/canvas/SaveBar";
import { API_BASE } from "@/lib/api/client";
import { server } from "@/test/mocks/server";
import { useRuleBuilderStore } from "@/state/ruleBuilderStore";
import type { RuleGraph } from "@/lib/api/ruleGraph";

const push = vi.fn();
vi.mock("next/navigation", () => ({
  useRouter: () => ({ push }),
}));

const emptyGraph: RuleGraph = {
  anonymous: { nodes: [], edges: [], root_node_id: null },
  registered: { nodes: [], edges: [], root_node_id: null },
  customer: { nodes: [], edges: [], root_node_id: null },
};

function wrapper({ children }: { children: ReactNode }) {
  const client = new QueryClient({
    defaultOptions: { queries: { retry: false }, mutations: { retry: false } },
  });
  return <QueryClientProvider client={client}>{children}</QueryClientProvider>;
}

const CREATE_URL = `${API_BASE}/api/v1/features/dn-article/versions`;
const PUBLISH_URL = `${API_BASE}/api/v1/features/dn-article/versions/5/publish`;

function versionRead(status: string) {
  return {
    id: "55555555-5555-5555-5555-555555555555",
    feature_id: "dn-article",
    version_number: 5,
    description: "new",
    status,
    rule_graph: emptyGraph,
    created_by: "alice",
    last_updated_by: "alice",
    last_updated_at: "2026-05-31T10:00:00Z",
    created_at: "2026-05-31T10:00:00Z",
  };
}

function renderSaveBar() {
  return render(
    <SaveBar fid="dn-article" vnum={3} featureBase="/products/features/html/dn-article" />,
    { wrapper },
  );
}

beforeEach(() => {
  push.mockReset();
  useRuleBuilderStore.getState().seedFromRuleGraph(emptyGraph, "live", () => "X");
});

describe("SaveBar — Save as New Version", () => {
  it("creates a Draft in a single POST (no follow-up PATCH) and navigates to it", async () => {
    const user = userEvent.setup();
    let published = false;
    let createCalls = 0;
    server.use(
      http.post(CREATE_URL, async ({ request }) => {
        createCalls += 1;
        // The on-screen rule_graph is sent on create (single atomic call).
        const body = (await request.json()) as { rule_graph?: unknown };
        expect(body.rule_graph).toBeDefined();
        return HttpResponse.json(versionRead("draft"), { status: 201 });
      }),
      http.post(PUBLISH_URL, () => {
        published = true;
        return HttpResponse.json(versionRead("live"));
      }),
    );

    renderSaveBar();
    await user.click(screen.getByRole("button", { name: /save as new version/i }));
    await user.click(screen.getByRole("button", { name: /^create version$/i }));

    await waitFor(() =>
      expect(push).toHaveBeenCalledWith("/products/features/html/dn-article/5"),
    );
    expect(createCalls).toBe(1);
    expect(published).toBe(false);
  });

  it("creates then publishes when Live is chosen", async () => {
    const user = userEvent.setup();
    let published = false;
    server.use(
      http.post(CREATE_URL, () => HttpResponse.json(versionRead("draft"), { status: 201 })),
      http.post(PUBLISH_URL, async ({ request }) => {
        published = true;
        expect(await request.json()).toEqual({ environment: "live" });
        return HttpResponse.json(versionRead("live"));
      }),
    );

    renderSaveBar();
    await user.click(screen.getByRole("button", { name: /save as new version/i }));
    await user.click(screen.getByRole("radio", { name: "Live" }));
    await user.click(screen.getByRole("button", { name: /create & publish live/i }));

    await waitFor(() => expect(published).toBe(true));
    await waitFor(() =>
      expect(push).toHaveBeenCalledWith("/products/features/html/dn-article/5"),
    );
  });

  it("navigates to the new draft and explains a partial failure when publish fails", async () => {
    const user = userEvent.setup();
    server.use(
      http.post(CREATE_URL, () => HttpResponse.json(versionRead("draft"), { status: 201 })),
      http.post(PUBLISH_URL, () =>
        HttpResponse.json(
          { error: { code: "INTERNAL_ERROR", message: "boom" } },
          { status: 500 },
        ),
      ),
    );

    renderSaveBar();
    await user.click(screen.getByRole("button", { name: /save as new version/i }));
    await user.click(screen.getByRole("radio", { name: "Live" }));
    await user.click(screen.getByRole("button", { name: /create & publish live/i }));

    expect(await screen.findByRole("alert")).toHaveTextContent(
      /publishing to live failed/i,
    );
    // The created draft is not lost — we still navigate to it.
    await waitFor(() =>
      expect(push).toHaveBeenCalledWith("/products/features/html/dn-article/5"),
    );
  });

  it("names the offending node + canvas on a 422 validation failure", async () => {
    const user = userEvent.setup();
    // Seed a single-outcome graph on the Anonymous canvas so the 422 loc
    // (nodes[0]) resolves to a node we can name in the banner.
    const seeded: RuleGraph = {
      anonymous: {
        root_node_id: "n_out",
        nodes: [
          {
            kind: "outcome",
            id: "n_out",
            outcome_id: "33333333-3333-3333-3333-333333333333",
            position: { x: 0, y: 0 },
          },
        ],
        edges: [],
      },
      registered: { nodes: [], edges: [], root_node_id: null },
      customer: { nodes: [], edges: [], root_node_id: null },
    };
    useRuleBuilderStore
      .getState()
      .seedFromRuleGraph(seeded, "live", () => "Show Content");

    server.use(
      http.post(CREATE_URL, () =>
        HttpResponse.json(
          {
            error: {
              code: "VALIDATION_ERROR",
              message: "invalid graph",
              details: [
                {
                  loc: "rule_graph.anonymous.nodes[0]",
                  msg: "outcome ref missing",
                  rule_id: "outcome_ref_exists",
                },
              ],
            },
          },
          { status: 422 },
        ),
      ),
    );

    renderSaveBar();
    await user.click(screen.getByRole("button", { name: /save as new version/i }));
    await user.click(screen.getByRole("button", { name: /^create version$/i }));

    const alert = await screen.findByRole("alert");
    expect(alert).toHaveTextContent(/1 rule node needs attention/i);
    expect(alert).toHaveTextContent(/Anonymous canvas/i);
    expect(alert).toHaveTextContent(/Show Content/);
    // The offending node is highlighted via the store's nodeErrors.
    expect(useRuleBuilderStore.getState().nodeErrors).toEqual({
      n_out: "outcome ref missing",
    });
  });
});
