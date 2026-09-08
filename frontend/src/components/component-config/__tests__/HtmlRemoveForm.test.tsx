import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";

import { HtmlRemoveForm } from "@/components/component-config/HtmlRemoveForm";
import type { HtmlRemoveConfig } from "@/lib/schemas/components";

const BASE: HtmlRemoveConfig = {
  type: "html_remove",
  target_selector: "",
  include_selector: false,
};

function setup(overrides?: Partial<HtmlRemoveConfig>) {
  const onValidSubmit = vi.fn();
  render(
    <div>
      <HtmlRemoveForm
        formId="test-form"
        initial={{ ...BASE, ...overrides }}
        onValidSubmit={onValidSubmit}
      />
      <button type="submit" form="test-form">
        Submit
      </button>
    </div>,
  );
  return { onValidSubmit };
}

describe("HtmlRemoveForm", () => {
  it("renders only a selector and the include-selector toggle", () => {
    setup();
    expect(screen.getByLabelText(/target selector/i)).toBeInTheDocument();
    expect(
      screen.getByRole("checkbox", { name: /remove the matched element too/i }),
    ).toBeInTheDocument();
    // No HTML body / placement mode — this component injects nothing.
    expect(screen.queryByLabelText(/html/i)).not.toBeInTheDocument();
    expect(screen.queryByLabelText(/placement/i)).not.toBeInTheDocument();
  });

  it("rejects an empty selector", async () => {
    const user = userEvent.setup();
    const { onValidSubmit } = setup();
    await user.click(screen.getByRole("button", { name: "Submit" }));
    expect(screen.getByText(/enter a css selector/i)).toBeInTheDocument();
    expect(onValidSubmit).not.toHaveBeenCalled();
  });

  it("submits contents-only removal by default", async () => {
    const user = userEvent.setup();
    const { onValidSubmit } = setup();
    await user.type(
      screen.getByLabelText(/target selector/i),
      "#dn-content-ssr",
    );
    await user.click(screen.getByRole("button", { name: "Submit" }));
    expect(onValidSubmit).toHaveBeenCalledWith({
      type: "html_remove",
      target_selector: "#dn-content-ssr",
      include_selector: false,
    });
  });

  it("submits element removal when the toggle is checked", async () => {
    const user = userEvent.setup();
    const { onValidSubmit } = setup({ target_selector: "#dn-content-ssr" });
    await user.click(
      screen.getByRole("checkbox", { name: /remove the matched element too/i }),
    );
    await user.click(screen.getByRole("button", { name: "Submit" }));
    expect(onValidSubmit).toHaveBeenCalledWith({
      type: "html_remove",
      target_selector: "#dn-content-ssr",
      include_selector: true,
    });
  });
});
