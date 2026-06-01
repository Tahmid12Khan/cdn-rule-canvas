import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it } from "vitest";

import { TransformationJourney } from "@/components/canvas/TransformationJourney";
import type { JourneyStep } from "@/lib/api/evalTest";

const journey: JourneyStep[] = [
  {
    index: 0,
    node_id: "start",
    kind: "start",
    label: "Start",
    branch: null,
    body_after: { api: "dn-article", body: ["a", "b", "c"] },
  },
  {
    index: 1,
    node_id: "d_api",
    kind: "decision",
    label: "JSON Expression",
    branch: true,
    body_after: { api: "dn-article", body: ["a", "b", "c"] },
  },
  {
    index: 2,
    node_id: "t_body",
    kind: "expression",
    label: "Trim JSON",
    branch: null,
    body_after: { api: "dn-article", body: [] },
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
  },
];

function body() {
  return screen.getByTestId("journey-body").textContent ?? "";
}

describe("TransformationJourney", () => {
  it("renders the first step's body_after and pretty-prints JSON", () => {
    render(<TransformationJourney journey={journey} featureType="json" />);

    expect(screen.getByText("Step 1 of 4")).toBeInTheDocument();
    expect(screen.getByText("Start")).toBeInTheDocument();
    // 2-space-indented pretty JSON.
    expect(body()).toContain('"api": "dn-article"');
    expect(body()).toContain('"body": [');
  });

  it("big arrows have accessible labels and a >=40px target", () => {
    render(<TransformationJourney journey={journey} featureType="json" />);
    const prev = screen.getByRole("button", { name: "Previous node" });
    const next = screen.getByRole("button", { name: "Next node" });
    expect(prev).toHaveClass("h-10", "w-10");
    expect(next).toHaveClass("h-10", "w-10");
  });

  it("Next button advances the index and renders the right body_after", async () => {
    const user = userEvent.setup();
    render(<TransformationJourney journey={journey} featureType="json" />);

    await user.click(screen.getByRole("button", { name: "Next node" }));
    expect(screen.getByText("Step 2 of 4")).toBeInTheDocument();
    expect(screen.getByText("JSON Expression")).toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: "Next node" }));
    expect(screen.getByText("Step 3 of 4")).toBeInTheDocument();
    expect(screen.getByText("Trim JSON")).toBeInTheDocument();
    // Trim emptied the array.
    expect(body()).toContain('"body": []');
  });

  it("Prev is disabled at the start and Next is disabled at the end (clamped)", async () => {
    const user = userEvent.setup();
    render(<TransformationJourney journey={journey} featureType="json" />);

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
    render(<TransformationJourney journey={journey} featureType="json" />);

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

  it("renders raw string body for HTML features", () => {
    const htmlJourney: JourneyStep[] = [
      {
        index: 0,
        node_id: "start",
        kind: "start",
        label: "Start",
        branch: null,
        body_after: "<html><body>hi</body></html>",
      },
    ];
    render(<TransformationJourney journey={htmlJourney} featureType="html" />);
    expect(body()).toBe("<html><body>hi</body></html>");
  });
});
