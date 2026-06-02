import { screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";

import { renderWithQuery } from "@/test/renderWithQuery";
import { TransformationJourney } from "@/components/canvas/TransformationJourney";
import type { JourneyStep } from "@/lib/api/evalTest";
import type { RFNode } from "@/lib/canvas/types";

const journey: JourneyStep[] = [
  {
    index: 0,
    node_id: "start",
    kind: "start",
    label: "Start",
    branch: null,
    body_after: { api: "dn-article", body: ["a", "b", "c"] },
    time_ms: "0.00",
  },
  {
    index: 1,
    node_id: "d_api",
    kind: "decision",
    label: "JSON Expression",
    branch: true,
    body_after: { api: "dn-article", body: ["a", "b", "c"] },
    time_ms: "0.00",
  },
  {
    index: 2,
    node_id: "t_body",
    kind: "expression",
    label: "Trim JSON",
    branch: null,
    body_after: { api: "dn-article", body: [] },
    time_ms: "0.41",
  },
  {
    index: 3,
    node_id: "end",
    kind: "end",
    label: "END",
    branch: null,
    body_after: {
      api: "dn-article",
      body: [],
      paywall_show: "<html>paywall_showed</html>",
    },
    time_ms: "0.00",
  },
];

// Live canvas nodes matching the journey node ids, so each step can look up its
// config for the inputs + plain-English description.
const canvasNodes: RFNode[] = [
  { id: "start", type: "startNode", position: { x: 0, y: 0 }, data: { label: "Start" } },
  {
    id: "d_api",
    type: "decisionNode",
    position: { x: 0, y: 100 },
    data: { processor: { type: "json_expression", json_path: "$.api", operator: "equals", value: "dn-article" } },
  },
  {
    id: "t_body",
    type: "expressionNode",
    position: { x: 0, y: 200 },
    data: { action: { type: "trim_json", json_path: "$.body", length: 0 } },
  },
  { id: "end", type: "endNode", position: { x: 0, y: 300 }, data: { label: "END" } },
];

function renderJourney(
  props?: Partial<React.ComponentProps<typeof TransformationJourney>>,
) {
  return renderWithQuery(
    <TransformationJourney
      journey={journey}
      featureType="json"
      canvasNodes={canvasNodes}
      onRestoreFullPath={() => {}}
      {...props}
    />,
  );
}

function body() {
  return screen.getByTestId("journey-body").textContent ?? "";
}

// Expand the (default-collapsed) journey so the stepper is visible.
async function expand(user: ReturnType<typeof userEvent.setup>) {
  await user.click(screen.getByRole("button", { name: "Expand" }));
}

describe("TransformationJourney", () => {
  it("is collapsed by default: shows step count + Expand, hides the stepper body", () => {
    renderJourney();

    expect(screen.getByText("4 steps")).toBeInTheDocument();
    expect(
      screen.getByRole("button", { name: "Expand" }),
    ).toBeInTheDocument();
    // Stepper body is hidden while collapsed.
    expect(screen.queryByTestId("journey-body")).not.toBeInTheDocument();
    expect(
      screen.queryByRole("button", { name: "Next node" }),
    ).not.toBeInTheDocument();
  });

  it("restores the full-path highlight on mount (collapsed) and on collapse", async () => {
    const user = userEvent.setup();
    const onRestoreFullPath = vi.fn();
    renderJourney({ onRestoreFullPath });

    // Collapsed on mount → full path restored.
    expect(onRestoreFullPath).toHaveBeenCalled();

    await expand(user);
    onRestoreFullPath.mockClear();

    // Collapse again → full path restored once more.
    await user.click(screen.getByRole("button", { name: "Collapse" }));
    expect(onRestoreFullPath).toHaveBeenCalled();
  });

  it("expanding shows the stepper: first step body + per-node time", async () => {
    const user = userEvent.setup();
    renderJourney();
    await expand(user);

    expect(screen.getByText("Step 1 of 4")).toBeInTheDocument();
    expect(screen.getByText("Start")).toBeInTheDocument();
    expect(screen.getByTestId("journey-time")).toHaveTextContent("0.00 ms");
    // 2-space-indented pretty JSON.
    expect(body()).toContain('"api": "dn-article"');
    expect(body()).toContain('"body": [');
  });

  it("big arrows have accessible labels and a >=40px target", async () => {
    const user = userEvent.setup();
    renderJourney();
    await expand(user);

    const prev = screen.getByRole("button", { name: "Previous node" });
    const next = screen.getByRole("button", { name: "Next node" });
    expect(prev).toHaveClass("h-10", "w-10");
    expect(next).toHaveClass("h-10", "w-10");
  });

  it("Next button advances the index and renders the right body_after + time", async () => {
    const user = userEvent.setup();
    renderJourney();
    await expand(user);

    await user.click(screen.getByRole("button", { name: "Next node" }));
    expect(screen.getByText("Step 2 of 4")).toBeInTheDocument();
    expect(screen.getByText("JSON Expression")).toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: "Next node" }));
    expect(screen.getByText("Step 3 of 4")).toBeInTheDocument();
    expect(screen.getByText("Trim JSON")).toBeInTheDocument();
    // Trim emptied the array.
    expect(body()).toContain('"body": []');
    // Per-node apply time for the expression step.
    expect(screen.getByTestId("journey-time")).toHaveTextContent("0.41 ms");
  });

  it("renders a plain-English description per step (describeStep)", async () => {
    const user = userEvent.setup();
    renderJourney();
    await expand(user);

    // Start.
    expect(screen.getByTestId("journey-description")).toHaveTextContent(
      "Start of the flow.",
    );

    // Decision: subject/op/value + matched (yes).
    await user.click(screen.getByRole("button", { name: "Next node" }));
    expect(screen.getByTestId("journey-description")).toHaveTextContent(
      /Checked .*\$\.api.* equals .*dn-article.* matched \(yes\)/,
    );

    // Expression: trim_json with actual path + length.
    await user.click(screen.getByRole("button", { name: "Next node" }));
    expect(screen.getByTestId("journey-description")).toHaveTextContent(
      "Trimmed the array at `$.body` to at most 0 items.",
    );

    // End.
    await user.click(screen.getByRole("button", { name: "Next node" }));
    expect(screen.getByTestId("journey-description")).toHaveTextContent(
      /End of the flow/,
    );
  });

  it("renders the node inputs as Field label = value", async () => {
    const user = userEvent.setup();
    renderJourney();
    await expand(user);

    // Step to the trim_json expression node.
    await user.click(screen.getByRole("button", { name: "Next node" }));
    await user.click(screen.getByRole("button", { name: "Next node" }));

    // Inputs render once the node-type manifest loads (MSW-backed).
    const inputs = await screen.findByTestId("journey-inputs");
    await waitFor(() => expect(inputs).toHaveTextContent("JSON path"));
    expect(inputs).toHaveTextContent("$.body");
    expect(inputs).toHaveTextContent("Max length");
  });

  it("Prev is disabled at the start and Next is disabled at the end (clamped)", async () => {
    const user = userEvent.setup();
    renderJourney();
    await expand(user);

    expect(screen.getByRole("button", { name: "Previous node" })).toBeDisabled();

    // Walk to the last step.
    const next = screen.getByRole("button", { name: "Next node" });
    await user.click(next);
    await user.click(next);
    await user.click(next);
    expect(screen.getByText("Step 4 of 4")).toBeInTheDocument();
    expect(next).toBeDisabled();
    // Clicking a disabled Next does nothing (still clamped at the end).
    await user.click(next);
    expect(screen.getByText("Step 4 of 4")).toBeInTheDocument();
  });

  it("ArrowRight / ArrowLeft move the step only when the journey is focused, clamped both ends", async () => {
    const user = userEvent.setup();
    renderJourney();
    await expand(user);

    const container = screen.getByRole("region", {
      name: "Transformation Journey",
    });
    container.focus();

    await user.keyboard("{ArrowRight}");
    expect(screen.getByText("Step 2 of 4")).toBeInTheDocument();

    await user.keyboard("{ArrowRight}{ArrowRight}");
    expect(screen.getByText("Step 4 of 4")).toBeInTheDocument();

    // Clamped at the top end.
    await user.keyboard("{ArrowRight}");
    expect(screen.getByText("Step 4 of 4")).toBeInTheDocument();

    // Back down, clamped at zero.
    await user.keyboard("{ArrowLeft}{ArrowLeft}{ArrowLeft}{ArrowLeft}");
    expect(screen.getByText("Step 1 of 4")).toBeInTheDocument();
    expect(body()).toContain('"body": [');
  });

  it("renders raw string body for HTML features", async () => {
    const user = userEvent.setup();
    const htmlJourney: JourneyStep[] = [
      {
        index: 0,
        node_id: "start",
        kind: "start",
        label: "Start",
        branch: null,
        body_after: "<html><body>hi</body></html>",
        time_ms: "0.00",
      },
    ];
    renderJourney({ journey: htmlJourney, featureType: "html" });
    await expand(user);
    expect(body()).toBe("<html><body>hi</body></html>");
  });
});
