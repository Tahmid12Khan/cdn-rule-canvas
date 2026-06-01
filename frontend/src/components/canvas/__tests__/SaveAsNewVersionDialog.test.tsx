import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";

import { SaveAsNewVersionDialog } from "@/components/canvas/SaveAsNewVersionDialog";

function setup() {
  const onConfirm = vi.fn();
  render(
    <SaveAsNewVersionDialog
      open
      onOpenChange={() => {}}
      saving={false}
      onConfirm={onConfirm}
    />,
  );
  return { onConfirm };
}

describe("SaveAsNewVersionDialog", () => {
  it("defaults to Draft and confirms with status='draft'", async () => {
    const user = userEvent.setup();
    const { onConfirm } = setup();

    await user.type(
      screen.getByLabelText(/description/i),
      "tweaked the rules",
    );
    await user.click(screen.getByRole("button", { name: /create version/i }));

    expect(onConfirm).toHaveBeenCalledWith("tweaked the rules", "draft");
  });

  it("does not call onConfirm while saving (re-entry guard against double-submit)", async () => {
    const user = userEvent.setup();
    const onConfirm = vi.fn();
    render(
      <SaveAsNewVersionDialog
        open
        onOpenChange={() => {}}
        saving
        onConfirm={onConfirm}
      />,
    );

    // The confirm button is disabled while saving and the click handler early-
    // returns when saving — together these guard a fast double-click / Enter+
    // click race that would otherwise create a duplicate draft version.
    const confirm = screen.getByRole("button", { name: /creating…/i });
    await user.click(confirm);
    await user.click(confirm);

    expect(onConfirm).not.toHaveBeenCalled();
  });

  it("switches to Live, shows a production warning, and confirms with status='live'", async () => {
    const user = userEvent.setup();
    const { onConfirm } = setup();

    await user.click(screen.getByRole("radio", { name: "Live" }));
    expect(
      screen.getByText(/take effect in production immediately/i),
    ).toBeInTheDocument();

    // The confirm button label makes the production consequence explicit.
    await user.click(
      screen.getByRole("button", { name: /create & publish live/i }),
    );
    expect(onConfirm).toHaveBeenCalledWith("", "live");
  });
});
