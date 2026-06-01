import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";

import { HtmlInjectionForm } from "@/components/component-config/HtmlInjectionForm";
import type { HtmlInjectionConfig } from "@/lib/schemas/components";

const EMPTY: HtmlInjectionConfig = {
  type: "html_injection",
  target_selector: "",
  placement_mode: "append",
  html_body: "",
  theme: null,
};

function setup(overrides?: Partial<HtmlInjectionConfig>) {
  const onValidSubmit = vi.fn();
  render(
    <div>
      <HtmlInjectionForm
        formId="test-form"
        initial={{ ...EMPTY, ...overrides }}
        onValidSubmit={onValidSubmit}
      />
      <button type="submit" form="test-form">
        Submit
      </button>
    </div>,
  );
  return { onValidSubmit };
}

describe("HtmlInjectionForm", () => {
  it("renders selector, placement mode, and the HTML editor tabs", () => {
    setup();
    expect(screen.getByLabelText(/target selector/i)).toBeInTheDocument();
    expect(screen.getByLabelText(/placement mode/i)).toBeInTheDocument();
    expect(screen.getByRole("tab", { name: "Visual" })).toBeInTheDocument();
    expect(screen.getByRole("tab", { name: "HTML" })).toBeInTheDocument();
  });

  it("shows validation errors and does not submit when fields are empty", async () => {
    const user = userEvent.setup();
    const { onValidSubmit } = setup();
    await user.click(screen.getByRole("button", { name: "Submit" }));
    expect(
      screen.getByText(/enter a css selector/i),
    ).toBeInTheDocument();
    expect(screen.getByText(/add the html to inject/i)).toBeInTheDocument();
    expect(onValidSubmit).not.toHaveBeenCalled();
  });

  it("submits a valid html_injection config", async () => {
    const user = userEvent.setup();
    const { onValidSubmit } = setup();
    await user.type(screen.getByLabelText(/target selector/i), ".article");
    await user.selectOptions(
      screen.getByLabelText(/placement mode/i),
      "prepend",
    );
    // Visual tab textarea is the html body field.
    const textareas = screen.getAllByRole("textbox");
    await user.type(textareas[textareas.length - 1], "<p>hi</p>");
    await user.click(screen.getByRole("button", { name: "Submit" }));

    expect(onValidSubmit).toHaveBeenCalledTimes(1);
    expect(onValidSubmit).toHaveBeenCalledWith({
      type: "html_injection",
      target_selector: ".article",
      placement_mode: "prepend",
      html_body: "<p>hi</p>",
      theme: null,
    });
  });

  it("switches between Visual and HTML tabs without losing content", async () => {
    const user = userEvent.setup();
    setup({ html_body: "<span>keep</span>" });
    await user.click(screen.getByRole("tab", { name: "HTML" }));
    expect(screen.getByRole("tab", { name: "HTML" })).toHaveAttribute(
      "aria-selected",
      "true",
    );
    // The code-tab textarea still holds the body content.
    expect(screen.getByDisplayValue("<span>keep</span>")).toBeInTheDocument();
  });
});
