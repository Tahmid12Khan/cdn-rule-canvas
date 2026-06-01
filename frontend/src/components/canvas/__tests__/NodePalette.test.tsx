import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it } from "vitest";

import { NodePalette } from "@/components/canvas/palette/NodePalette";

const OUTCOMES = [
  { id: "11111111-1111-1111-1111-111111111111", title: "Show Content" },
];

describe("NodePalette", () => {
  it("renders the enabled Meta Tags chip in the Content category", async () => {
    const user = userEvent.setup();
    render(<NodePalette outcomes={OUTCOMES} draggable />);
    await user.click(screen.getByRole("tab", { name: "Content" }));
    const chip = screen.getByText("Meta Tags").closest("[data-chip-id]");
    expect(chip).toHaveAttribute("data-enabled", "true");
    expect(chip).toHaveAttribute("draggable", "true");
  });

  it("renders disabled chips with aria-disabled", async () => {
    const user = userEvent.setup();
    render(<NodePalette outcomes={OUTCOMES} draggable />);
    await user.click(screen.getByRole("tab", { name: "User" }));
    const chip = screen.getByText("Logged In").closest("[data-chip-id]");
    expect(chip).toHaveAttribute("aria-disabled", "true");
    expect(chip).toHaveAttribute("data-enabled", "false");
  });

  it("search 'device' shows only the Device Type chip", async () => {
    const user = userEvent.setup();
    render(<NodePalette outcomes={OUTCOMES} draggable />);
    await user.click(screen.getByRole("tab", { name: "Search nodes" }));
    await user.type(screen.getByPlaceholderText("Search nodes…"), "device");
    await waitFor(() =>
      expect(screen.getByText("Device Type")).toBeInTheDocument(),
    );
    expect(screen.queryByText("Meta Tags")).not.toBeInTheDocument();
  });

  it("injects one draggable chip per outcome", async () => {
    const user = userEvent.setup();
    render(<NodePalette outcomes={OUTCOMES} draggable />);
    await user.click(screen.getByRole("tab", { name: "Outcomes" }));
    const chip = screen.getByText("Show Content").closest("[data-chip-id]");
    expect(chip).toHaveAttribute("data-enabled", "true");
  });

  it("makes enabled chips non-draggable in view mode", async () => {
    const user = userEvent.setup();
    render(<NodePalette outcomes={OUTCOMES} draggable={false} />);
    await user.click(screen.getByRole("tab", { name: "Content" }));
    const chip = screen.getByText("Meta Tags").closest("[data-chip-id]");
    expect(chip).toHaveAttribute("draggable", "false");
  });
});
