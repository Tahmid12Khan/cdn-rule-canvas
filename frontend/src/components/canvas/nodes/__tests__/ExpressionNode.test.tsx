import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { render, screen, waitFor } from "@testing-library/react";
import type { ReactNode } from "react";
import { ReactFlowProvider } from "@xyflow/react";
import { describe, expect, it } from "vitest";

import { ExpressionNode } from "@/components/canvas/nodes/ExpressionNode";
import type { ExpressionNodeData } from "@/lib/canvas/types";

function nodeProps(data: ExpressionNodeData) {
  return {
    id: "a1",
    data,
    selected: false,
    type: "expressionNode",
    isConnectable: true,
    xPos: 0,
    yPos: 0,
    dragging: false,
    zIndex: 0,
  } as unknown as Parameters<typeof ExpressionNode>[0];
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

describe("ExpressionNode (manifest-driven)", () => {
  it("shows the manifest label as the title (trim_json -> 'Trim JSON')", async () => {
    render(
      <ExpressionNode
        {...nodeProps({ action: { type: "trim_json", json_path: "$.body", length: 3 } })}
      />,
      { wrapper },
    );
    await waitFor(() =>
      expect(screen.getByTestId("expression-title")).toHaveTextContent(
        "Trim JSON",
      ),
    );
  });

  it("renders a one-line action summary for a json action", async () => {
    render(
      <ExpressionNode
        {...nodeProps({ action: { type: "add_attribute", json_path: "$.x", value: "v" } })}
      />,
      { wrapper },
    );
    await waitFor(() =>
      expect(screen.getByTestId("expression-summary")).toHaveTextContent("$.x"),
    );
  });

  it("shows the cached outcome title for an apply_outcome action", async () => {
    render(
      <ExpressionNode
        {...nodeProps({
          action: { type: "apply_outcome", outcome_id: "x" },
          outcomeTitle: "Show Paywall",
        })}
      />,
      { wrapper },
    );
    await waitFor(() =>
      expect(screen.getByTestId("expression-summary")).toHaveTextContent(
        "Show Paywall",
      ),
    );
  });
});
