import { screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it } from "vitest";

import { NodePalette } from "@/components/canvas/palette/NodePalette";
import { renderWithQuery } from "@/test/renderWithQuery";

const OUTCOMES = [
  { id: "11111111-1111-1111-1111-111111111111", title: "Show Content" },
];

describe("NodePalette (manifest-driven)", () => {
  it("renders the enabled Meta Tags chip in the Content category", async () => {
    const user = userEvent.setup();
    renderWithQuery(<NodePalette outcomes={OUTCOMES} draggable />);
    // Wait for the manifest-driven Content tab to appear.
    await screen.findByRole("tab", { name: "Content" });
    await user.click(screen.getByRole("tab", { name: "Content" }));
    const chip = screen.getByText("Meta Tags").closest("[data-chip-id]");
    expect(chip).toHaveAttribute("data-enabled", "true");
    expect(chip).toHaveAttribute("draggable", "true");
  });

  it("renders a coming_soon category's chips disabled", async () => {
    const user = userEvent.setup();
    renderWithQuery(<NodePalette outcomes={OUTCOMES} draggable />);
    await screen.findByRole("tab", { name: "User" });
    await user.click(screen.getByRole("tab", { name: "User" }));
    // The User category has no node types in the fixture -> placeholder chip.
    const chip = screen.getByText("Coming soon").closest("[data-chip-id]");
    expect(chip).toHaveAttribute("aria-disabled", "true");
    expect(chip).toHaveAttribute("data-enabled", "false");
  });

  it("search 'device' shows only the Device Type chip", async () => {
    const user = userEvent.setup();
    renderWithQuery(<NodePalette outcomes={OUTCOMES} draggable />);
    await user.click(screen.getByRole("tab", { name: "Search nodes" }));
    await user.type(screen.getByPlaceholderText("Search nodes…"), "device");
    await waitFor(() =>
      expect(screen.getByText("Device Type")).toBeInTheDocument(),
    );
    expect(screen.queryByText("Meta Tags")).not.toBeInTheDocument();
  });

  it("surfaces the Apply Outcome action chip in the Content category", async () => {
    const user = userEvent.setup();
    renderWithQuery(<NodePalette outcomes={OUTCOMES} draggable />);
    await screen.findByRole("tab", { name: "Content" });
    await user.click(screen.getByRole("tab", { name: "Content" }));
    const chip = screen.getByText("Apply Outcome").closest("[data-chip-id]");
    expect(chip).toHaveAttribute("data-enabled", "true");
    expect(chip).toHaveAttribute("draggable", "true");
  });

  it("makes enabled chips non-draggable in view mode", async () => {
    const user = userEvent.setup();
    renderWithQuery(<NodePalette outcomes={OUTCOMES} draggable={false} />);
    await screen.findByRole("tab", { name: "Content" });
    await user.click(screen.getByRole("tab", { name: "Content" }));
    const chip = screen.getByText("Meta Tags").closest("[data-chip-id]");
    expect(chip).toHaveAttribute("draggable", "false");
  });

  it("hides the JSON-only category for an HTML feature", async () => {
    renderWithQuery(
      <NodePalette outcomes={OUTCOMES} draggable featureType="html" />,
    );
    await screen.findByRole("tab", { name: "Content" });
    // json_expression is applies_to:"json" — its "JSON" category is dropped for
    // an HTML feature.
    expect(
      screen.queryByRole("tab", { name: "JSON" }),
    ).not.toBeInTheDocument();
  });

  it("shows the JSON Expression chip and hides Meta Tags for a JSON feature", async () => {
    const user = userEvent.setup();
    renderWithQuery(
      <NodePalette outcomes={OUTCOMES} draggable featureType="json" />,
    );
    // JSON category appears for a JSON feature.
    await screen.findByRole("tab", { name: "JSON" });
    await user.click(screen.getByRole("tab", { name: "JSON" }));
    expect(screen.getByText("JSON Expression")).toBeInTheDocument();

    // Meta Tags (applies_to:"html") is gated out: the Content tab now only has
    // the "all" article_url chip, not the html-only Meta Tags chip.
    await user.click(screen.getByRole("tab", { name: "Content" }));
    expect(screen.queryByText("Meta Tags")).not.toBeInTheDocument();
    expect(screen.getByText("URL")).toBeInTheDocument();
  });
});
