import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { http, HttpResponse } from "msw";
import type { ReactNode } from "react";
import { beforeEach, describe, expect, it } from "vitest";

import { TestingPanel } from "@/components/canvas/TestingPanel";
import { API_BASE } from "@/lib/api/client";
import { server } from "@/test/mocks/server";
import { useRuleBuilderStore } from "@/state/ruleBuilderStore";
import type { RuleGraph } from "@/lib/api/ruleGraph";

const graph: RuleGraph = {
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

beforeEach(() => {
  useRuleBuilderStore.getState().seedFromRuleGraph(graph, "draft", () => "X");
  // The "Test a rule" tab mounts TestPresetBar, which lists presets.
  server.use(
    http.get(`${API_BASE}/api/v1/test-presets`, () =>
      HttpResponse.json({ items: [], page: 1, page_size: 100, total: 0 }),
    ),
  );
});

describe("TestingPanel", () => {
  it("is collapsed by default (no tabs visible)", () => {
    render(
      <TestingPanel outcomeTitleById={() => "X"} featureType="html" />,
      { wrapper },
    );

    expect(
      screen.getByRole("button", { name: /^testing$/i }),
    ).toHaveAttribute("aria-expanded", "false");
    expect(screen.queryByRole("tablist")).not.toBeInTheDocument();
  });

  it("expands to show two tabs with the live-URL tab active by default", async () => {
    const user = userEvent.setup();
    render(
      <TestingPanel outcomeTitleById={() => "X"} featureType="html" />,
      { wrapper },
    );

    await user.click(screen.getByRole("button", { name: /^testing$/i }));

    expect(screen.getByRole("tablist")).toBeInTheDocument();
    const urlTab = screen.getByRole("tab", { name: /test with live url/i });
    const ruleTab = screen.getByRole("tab", { name: /test a rule/i });
    expect(urlTab).toHaveAttribute("aria-selected", "true");
    expect(ruleTab).toHaveAttribute("aria-selected", "false");

    // Default body is the live-URL panel.
    expect(
      screen.getByRole("region", { name: /test with a live url/i }),
    ).toBeInTheDocument();
    expect(
      screen.queryByRole("region", { name: /^test a rule$/i }),
    ).not.toBeInTheDocument();
  });

  it("switches the body when a different tab is selected", async () => {
    const user = userEvent.setup();
    render(
      <TestingPanel outcomeTitleById={() => "X"} featureType="html" />,
      { wrapper },
    );

    await user.click(screen.getByRole("button", { name: /^testing$/i }));
    await user.click(screen.getByRole("tab", { name: /test a rule/i }));

    expect(
      screen.getByRole("tab", { name: /test a rule/i }),
    ).toHaveAttribute("aria-selected", "true");
    expect(
      screen.getByRole("region", { name: /^test a rule$/i }),
    ).toBeInTheDocument();
    expect(
      screen.queryByRole("region", { name: /test with a live url/i }),
    ).not.toBeInTheDocument();
  });
});
