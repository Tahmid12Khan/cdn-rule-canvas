import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { http, HttpResponse } from "msw";
import type { ReactNode } from "react";
import { beforeEach, describe, expect, it } from "vitest";

import { TestPanel } from "@/components/canvas/TestPanel";
import { PROXY_BASE } from "@/lib/api/evalTest";
import { server } from "@/test/mocks/server";
import { useRuleBuilderStore } from "@/state/ruleBuilderStore";
import type { RuleGraph } from "@/lib/api/ruleGraph";

const OUTCOME_UUID = "11111111-1111-1111-1111-111111111111";

const graph: RuleGraph = {
  anonymous: {
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
  registered: { nodes: [], edges: [], root_node_id: null },
  customer: { nodes: [], edges: [], root_node_id: null },
};

function wrapper({ children }: { children: ReactNode }) {
  const client = new QueryClient({
    defaultOptions: { queries: { retry: false }, mutations: { retry: false } },
  });
  return <QueryClientProvider client={client}>{children}</QueryClientProvider>;
}

beforeEach(() => {
  useRuleBuilderStore
    .getState()
    .seedFromRuleGraph(graph, "draft", () => "Paywall");
});

describe("TestPanel", () => {
  it("runs a test, highlights the path, and names the applied outcome", async () => {
    const user = userEvent.setup();
    server.use(
      http.post(`${PROXY_BASE}/__rre/eval`, () =>
        HttpResponse.json({
          matched_node_id: "o1",
          traversed_node_ids: ["d1", "o1"],
          traversed_edge_ids: ["e1"],
          steps: [{ node_id: "d1", kind: "decision", branch: "yes", result: true }],
        }),
      ),
    );

    render(<TestPanel outcomeTitleById={() => "Paywall"} featureType="html" />, {
      wrapper,
    });

    await user.click(screen.getByRole("button", { name: /run test/i }));

    await waitFor(() =>
      expect(screen.getByText(/applied outcome/i)).toBeInTheDocument(),
    );
    expect(screen.getByText("Paywall")).toBeInTheDocument();
    const hl = useRuleBuilderStore.getState().testHighlight;
    expect(hl?.nodeIds.has("d1")).toBe(true);
    expect(hl?.edgeIds.has("e1")).toBe(true);
    expect(hl?.outcomeNodeId).toBe("o1");

    // Clear removes the highlight.
    await user.click(screen.getByRole("button", { name: /clear highlight/i }));
    expect(useRuleBuilderStore.getState().testHighlight).toBeNull();
  });

  it("treats reaching END with no apply_outcome as success (not a dead end)", async () => {
    const user = userEvent.setup();
    server.use(
      http.post(`${PROXY_BASE}/__rre/eval`, () =>
        HttpResponse.json({
          matched_node_id: null,
          traversed_node_ids: ["d1"],
          traversed_edge_ids: [],
          steps: [{ node_id: "d1", kind: "decision", branch: "no", result: false }],
          // NO branch goes straight from the decision to END — a complete path,
          // even though no apply_outcome ran. The proxy appends the END step.
          journey: [
            { index: 0, node_id: "start", kind: "start", label: "Start", branch: null, body_after: "<html></html>" },
            { index: 1, node_id: "d1", kind: "decision", label: "device_type", branch: false, body_after: "<html></html>" },
            { index: 2, node_id: "end", kind: "end", label: "End", branch: null, body_after: "<html></html>" },
          ],
        }),
      ),
    );

    render(<TestPanel outcomeTitleById={() => "Paywall"} featureType="html" />, {
      wrapper,
    });

    await user.click(screen.getByRole("button", { name: /run test/i }));

    // Reaching END is success — NOT the old misleading "no outcome / dead end".
    await waitFor(() =>
      expect(screen.getByText(/reached/i)).toBeInTheDocument(),
    );
    expect(
      screen.queryByText(/didn.t reach an end node/i),
    ).not.toBeInTheDocument();
  });

  it("shows a friendly message when the proxy is unreachable", async () => {
    const user = userEvent.setup();
    server.use(
      http.post(`${PROXY_BASE}/__rre/eval`, () => HttpResponse.error()),
    );

    render(<TestPanel outcomeTitleById={() => "Paywall"} featureType="html" />, {
      wrapper,
    });
    await user.click(screen.getByRole("button", { name: /run test/i }));

    await waitFor(() =>
      expect(
        screen.getByText(/could not reach the evaluator/i),
      ).toBeInTheDocument(),
    );
  });
});
