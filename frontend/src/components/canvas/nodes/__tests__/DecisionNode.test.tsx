import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import type { ReactNode } from "react";
import { ReactFlowProvider } from "reactflow";
import { describe, expect, it } from "vitest";

import { DecisionNode } from "@/components/canvas/nodes/DecisionNode";
import type { DecisionNodeData } from "@/lib/canvas/types";

// Minimal NodeProps subset DecisionNode reads.
function nodeProps(data: DecisionNodeData) {
  return {
    id: "d1",
    data,
    selected: false,
    type: "decisionNode",
    isConnectable: true,
    xPos: 0,
    yPos: 0,
    dragging: false,
    zIndex: 0,
  } as unknown as Parameters<typeof DecisionNode>[0];
}

function wrapper({ children }: { children: ReactNode }) {
  const client = new QueryClient({
    defaultOptions: { queries: { retry: false } },
  });
  return (
    <QueryClientProvider client={client}>
      <ReactFlowProvider>{children}</ReactFlowProvider>
    </QueryClientProvider>
  );
}

const articleProcessor: DecisionNodeData = {
  processor: { type: "article_url", operator: "contains", value: "/news/" },
};

describe("DecisionNode (manifest-driven)", () => {
  it("shows the manifest label as the title (article_url -> 'URL')", async () => {
    render(<DecisionNode {...nodeProps(articleProcessor)} />, { wrapper });
    const titles = await screen.findAllByText("URL");
    expect(titles.length).toBeGreaterThan(0);
  });

  it("truncates the title and keeps the full text available on hover (title attr)", async () => {
    render(<DecisionNode {...nodeProps(articleProcessor)} />, { wrapper });
    // Wait for the manifest-driven label (not the pre-load kind fallback).
    await waitFor(() =>
      expect(screen.getByTestId("decision-title")).toHaveAttribute(
        "title",
        "URL",
      ),
    );
    const title = screen.getByTestId("decision-title");
    expect(title).toHaveClass("truncate");
    expect(title).toHaveClass("max-w-[120px]");
  });

  it("renders the title pill IN FRONT of the diamond (relative z-20)", async () => {
    render(<DecisionNode {...nodeProps(articleProcessor)} />, { wrapper });
    const title = await screen.findByTestId("decision-title");
    // The load-bearing layering: the title must be positioned + above the
    // diamond body so a bigger diamond can't occlude the name (req 4).
    expect(title).toHaveClass("relative");
    expect(title).toHaveClass("z-20");
  });

  it("shows each input field's value INSIDE the diamond, one per line", async () => {
    render(<DecisionNode {...nodeProps(articleProcessor)} />, { wrapper });
    // Wait for the manifest to load (the inside values come from spec.fields);
    // the container exists immediately but is empty until the spec resolves.
    await waitFor(() =>
      expect(screen.getByTestId("decision-values").querySelectorAll("span"))
        .toHaveLength(2),
    );
    const values = screen.getByTestId("decision-values");
    // article_url has operator "contains" + value "/news/" — both rendered as
    // their own centered, truncated <span> lines inside the diamond body.
    const lines = values.querySelectorAll("span");
    expect(lines.length).toBe(2);
    expect(values).toHaveTextContent("contains");
    expect(values).toHaveTextContent("/news/");
    // Centered column.
    expect(values).toHaveClass("items-center");
    expect(values).toHaveClass("text-center");
    lines.forEach((line) => expect(line).toHaveClass("truncate"));
  });

  it("renders a tooltip on hover with fields, summary, and branches", async () => {
    const user = userEvent.setup();
    render(<DecisionNode {...nodeProps(articleProcessor)} />, { wrapper });
    await screen.findByTestId("decision-title");
    await user.hover(screen.getByTestId("decision-node"));

    const tooltip = await screen.findByTestId("decision-tooltip");
    // field label -> current value
    expect(tooltip).toHaveTextContent("Operator:");
    expect(tooltip).toHaveTextContent("contains");
    expect(tooltip).toHaveTextContent("Value:");
    expect(tooltip).toHaveTextContent("/news/");
    // input summary
    expect(tooltip).toHaveTextContent("Matches against the request URL");
    // output branches
    expect(tooltip).toHaveTextContent("Yes · No");
  });

  it("keeps the tooltip non-interactive so it never blocks drag/handles", async () => {
    const user = userEvent.setup();
    render(<DecisionNode {...nodeProps(articleProcessor)} />, { wrapper });
    await screen.findByTestId("decision-title");
    await user.hover(screen.getByTestId("decision-node"));
    const tooltip = await screen.findByTestId("decision-tooltip");
    expect(tooltip).toHaveClass("pointer-events-none");
  });

  it("hides the tooltip when the manifest has no spec for the kind", async () => {
    render(
      <DecisionNode
        {...nodeProps({ processor: { type: "unknown_future" } })}
      />,
      { wrapper },
    );
    // Title falls back to the raw kind.
    const title = await screen.findByTestId("decision-title");
    expect(title).toHaveAttribute("title", "unknown_future");
    await waitFor(() =>
      expect(screen.queryByTestId("decision-tooltip")).toBeNull(),
    );
  });
});
