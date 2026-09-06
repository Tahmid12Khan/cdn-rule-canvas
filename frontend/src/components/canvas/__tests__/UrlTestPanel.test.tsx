import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { http, HttpResponse } from "msw";
import type { ReactNode } from "react";
import { beforeEach, describe, expect, it } from "vitest";

import { UrlTestPanel } from "@/components/canvas/UrlTestPanel";
import { PROXY_BASE } from "@/lib/api/evalTest";
import { server } from "@/test/mocks/server";
import { useRuleBuilderStore } from "@/state/ruleBuilderStore";
import type { RuleGraph } from "@/lib/api/ruleGraph";

const OUTCOME_UUID = "11111111-1111-1111-1111-111111111111";

const graph: RuleGraph = {
  canvas: {
    nodes: [
      { kind: "start", id: "start", position: { x: 0, y: 0 } },
      {
        kind: "decision",
        id: "d1",
        processor: { type: "device_type", operator: "equals", value: "mobile" },
        position: { x: 0, y: 100 },
      },
      {
        kind: "expression",
        id: "o1",
        action: { type: "apply_outcome", outcome_id: OUTCOME_UUID },
        position: { x: 100, y: 200 },
      },
      { kind: "end", id: "end", position: { x: 100, y: 300 } },
    ],
    edges: [
      { id: "e0", source_node_id: "start", target_node_id: "d1", branch: "yes" },
      { id: "e1", source_node_id: "d1", target_node_id: "o1", branch: "yes" },
      { id: "e2", source_node_id: "o1", target_node_id: "end", branch: "yes" },
    ],
    root_node_id: "start",
  },
};

function wrapper({ children }: { children: ReactNode }) {
  const client = new QueryClient({
    defaultOptions: { queries: { retry: false }, mutations: { retry: false } },
  });
  return <QueryClientProvider client={client}>{children}</QueryClientProvider>;
}

// A complete start→END journey so the highlight + Transformation Journey render.
const fullJourneyResponse = {
  matched_node_id: "o1",
  traversed_node_ids: ["d1", "o1"],
  traversed_edge_ids: ["e1"],
  steps: [{ node_id: "d1", kind: "decision", branch: "yes", result: true }],
  journey: [
    { index: 0, node_id: "start", kind: "start", label: "Start", branch: null, body_after: "<html></html>", time_ms: "0.00" },
    { index: 1, node_id: "d1", kind: "decision", label: "Device Type", branch: true, body_after: "<html></html>", time_ms: "0.00" },
    { index: 2, node_id: "o1", kind: "expression", label: "Apply Outcome", branch: null, body_after: "<html>pw</html>", time_ms: "0.42" },
    { index: 3, node_id: "end", kind: "end", label: "END", branch: null, body_after: "<html>pw</html>", time_ms: "0.00" },
  ],
};

beforeEach(() => {
  useRuleBuilderStore
    .getState()
    .seedFromRuleGraph(graph, "draft", () => "Paywall");
});

describe("UrlTestPanel", () => {
  it("sends the URL + test headers, highlights the path, and names the outcome", async () => {
    const user = userEvent.setup();
    let captured: { url?: string; headers?: Record<string, string> } = {};
    server.use(
      http.post(`${PROXY_BASE}/__rre/eval-url`, async ({ request }) => {
        captured = (await request.json()) as typeof captured;
        return HttpResponse.json(fullJourneyResponse);
      }),
    );

    render(
      <UrlTestPanel outcomeTitleById={() => "Paywall"} featureType="html" />,
      { wrapper },
    );

    await user.type(
      screen.getByLabelText("Full URL"),
      "https://www.example.com/article/1",
    );
    await user.type(screen.getByLabelText("Test header name 1"), "X-Test");
    await user.type(screen.getByLabelText("Test header value 1"), "yes");

    await user.click(screen.getByRole("button", { name: /run test/i }));

    await waitFor(() =>
      expect(screen.getByText(/applied outcome/i)).toBeInTheDocument(),
    );
    expect(screen.getByText("Paywall")).toBeInTheDocument();

    // The request carried the URL + test headers verbatim.
    expect(captured.url).toBe("https://www.example.com/article/1");
    expect(captured.headers).toEqual({ "X-Test": "yes" });

    // The FULL start→END path is highlighted (incl. start + end), same as the
    // synthetic test panel.
    const hl = useRuleBuilderStore.getState().testHighlight;
    expect(hl?.nodeIds.has("start")).toBe(true);
    expect(hl?.nodeIds.has("end")).toBe(true);
    expect(hl?.edgeIds.has("e0")).toBe(true);
    expect(hl?.outcomeNodeId).toBe("o1");

    // Clear removes the highlight.
    await user.click(screen.getByRole("button", { name: /clear highlight/i }));
    expect(useRuleBuilderStore.getState().testHighlight).toBeNull();
  });

  it("disables Run until a URL is entered", async () => {
    const user = userEvent.setup();
    render(
      <UrlTestPanel outcomeTitleById={() => "Paywall"} featureType="html" />,
      { wrapper },
    );

    const run = screen.getByRole("button", { name: /run test/i });
    expect(run).toBeDisabled();
    await user.type(screen.getByLabelText("Full URL"), "https://x.test/a");
    expect(run).toBeEnabled();
  });

  it("surfaces the proxy's message when the host matches no site", async () => {
    const user = userEvent.setup();
    server.use(
      http.post(`${PROXY_BASE}/__rre/eval-url`, () =>
        HttpResponse.json(
          {
            error: {
              code: "NO_SITE",
              message:
                'No Site is configured for host "unknown.test". Add a Site whose source host matches this URL, then run the test again.',
            },
          },
          { status: 400 },
        ),
      ),
    );

    render(
      <UrlTestPanel outcomeTitleById={() => "Paywall"} featureType="html" />,
      { wrapper },
    );

    await user.type(
      screen.getByLabelText("Full URL"),
      "http://unknown.test/article/1",
    );
    await user.click(screen.getByRole("button", { name: /run test/i }));

    await waitFor(() =>
      expect(
        screen.getAllByText(/no site is configured/i).length,
      ).toBeGreaterThan(0),
    );
  });

  it("rejects an invalid test-header name inline and blocks Run until fixed", async () => {
    const user = userEvent.setup();
    let posted = false;
    server.use(
      http.post(`${PROXY_BASE}/__rre/eval-url`, () => {
        posted = true;
        return HttpResponse.json(fullJourneyResponse);
      }),
    );

    render(
      <UrlTestPanel outcomeTitleById={() => "Paywall"} featureType="html" />,
      { wrapper },
    );

    await user.type(
      screen.getByLabelText("Full URL"),
      "https://www.example.com/article/1",
    );
    // An invalid header NAME — the embedded space makes it a non-HTTP-token.
    await user.type(screen.getByLabelText("Test header name 1"), "X-Bad Header");
    await user.type(screen.getByLabelText("Test header value 1"), "leak");

    // Inline validation surfaces and Run is disabled — nothing is ever posted to
    // the proxy (the frontend now validates to the same rules as Site headers).
    expect(screen.getByRole("alert")).toHaveTextContent(/valid HTTP token/i);
    const run = screen.getByRole("button", { name: /run test/i });
    expect(run).toBeDisabled();

    // Fixing the name (remove the space) clears the error and re-enables Run.
    await user.clear(screen.getByLabelText("Test header name 1"));
    await user.type(screen.getByLabelText("Test header name 1"), "X-Good");
    expect(screen.queryByRole("alert")).toBeNull();
    expect(run).toBeEnabled();
    expect(posted).toBe(false);
  });
});
