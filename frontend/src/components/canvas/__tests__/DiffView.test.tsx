import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeAll, describe, expect, it, vi } from "vitest";

import { DiffView } from "@/components/canvas/DiffView";

// jsdom doesn't implement scrollIntoView; the nav buttons call it.
beforeAll(() => {
  Element.prototype.scrollIntoView = vi.fn();
});

// A body with two changes separated by a long run of unchanged lines, so the
// collapsed view yields a gap and groupHunks yields two hunks.
const ctx = Array.from({ length: 12 }, (_, i) => `line ${i}`).join("\n");
const before = `head\n${ctx}\ntail`;
const after = `HEAD\n${ctx}\nTAIL`;

describe("DiffView", () => {
  it("renders nothing-but-a-note when before === after", () => {
    render(<DiffView before="a\nb" after="a\nb" />);
    expect(screen.getByTestId("diff-view")).toHaveTextContent("No changes.");
  });

  it("collapsed by default: shows a gap row for the hidden middle context", () => {
    render(<DiffView before={before} after={after} />);
    // Default toggle label is "Expanded" (the action you can take).
    expect(
      screen.getByRole("button", { name: "Expanded" }),
    ).toBeInTheDocument();
    expect(screen.getByTestId("diff-gap")).toHaveTextContent(/unchanged lines/);
  });

  it("toggling to Expanded shows all lines and removes the gap", async () => {
    const user = userEvent.setup();
    render(<DiffView before={before} after={after} />);

    await user.click(screen.getByRole("button", { name: "Expanded" }));
    expect(screen.queryByTestId("diff-gap")).not.toBeInTheDocument();
    // Every middle context line is now visible.
    expect(screen.getByText(/line 6/)).toBeInTheDocument();
    // Toggle now offers to go back to Collapsed.
    expect(
      screen.getByRole("button", { name: "Collapsed" }),
    ).toBeInTheDocument();
  });

  it("navigates hunks with the prev/next buttons and reports change X of N", async () => {
    const user = userEvent.setup();
    render(<DiffView before={before} after={after} />);

    const counter = screen.getByTestId("diff-hunk-counter");
    expect(counter).toHaveTextContent("change 1 of 2");

    const next = screen.getByRole("button", { name: "Next change" });
    const prev = screen.getByRole("button", { name: "Previous change" });
    expect(prev).toBeDisabled();

    await user.click(next);
    expect(counter).toHaveTextContent("change 2 of 2");
    expect(next).toBeDisabled();
    expect(Element.prototype.scrollIntoView).toHaveBeenCalled();

    await user.click(prev);
    expect(counter).toHaveTextContent("change 1 of 2");
  });

  it("Ctrl+ArrowDown / Ctrl+ArrowUp step hunks when the view is focused", async () => {
    const user = userEvent.setup();
    render(<DiffView before={before} after={after} />);

    screen.getByTestId("diff-view").focus();
    const counter = screen.getByTestId("diff-hunk-counter");

    await user.keyboard("{Control>}{ArrowDown}{/Control}");
    expect(counter).toHaveTextContent("change 2 of 2");

    await user.keyboard("{Control>}{ArrowUp}{/Control}");
    expect(counter).toHaveTextContent("change 1 of 2");
  });

  it("renders a title in the header when provided", () => {
    render(<DiffView before={before} after={after} title="Start → End" />);
    expect(screen.getByText("Start → End")).toBeInTheDocument();
  });

  it("state is per instance: two DiffViews navigate independently", async () => {
    const user = userEvent.setup();
    render(
      <>
        <DiffView before={before} after={after} title="one" />
        <DiffView before={before} after={after} title="two" />
      </>,
    );

    const counters = screen.getAllByTestId("diff-hunk-counter");
    const nexts = screen.getAllByRole("button", { name: "Next change" });

    await user.click(nexts[0]);
    expect(counters[0]).toHaveTextContent("change 2 of 2");
    // Second instance untouched.
    expect(counters[1]).toHaveTextContent("change 1 of 2");
  });
});
