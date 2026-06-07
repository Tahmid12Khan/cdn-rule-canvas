import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";

import {
  TEST_PRESET_EXAMPLES,
  TestPresetExamples,
} from "@/components/test-presets/TestPresetExamples";

describe("TestPresetExamples", () => {
  it("renders a card per starter template with a human explanation", () => {
    render(<TestPresetExamples onUse={vi.fn()} />);

    for (const example of TEST_PRESET_EXAMPLES) {
      expect(
        screen.getByRole("heading", { name: example.name }),
      ).toBeInTheDocument();
    }
    // Plain-language onboarding labels are present on each card.
    expect(screen.getAllByText(/when to use it/i)).toHaveLength(
      TEST_PRESET_EXAMPLES.length,
    );
    expect(screen.getAllByText(/what it simulates/i)).toHaveLength(
      TEST_PRESET_EXAMPLES.length,
    );
  });

  it("fires onUse with the chosen example when 'Use this template' is clicked", async () => {
    const user = userEvent.setup();
    const onUse = vi.fn();
    render(<TestPresetExamples onUse={onUse} />);

    const buttons = screen.getAllByRole("button", {
      name: /use this template/i,
    });
    await user.click(buttons[1]);

    expect(onUse).toHaveBeenCalledWith(TEST_PRESET_EXAMPLES[1]);
  });
});
