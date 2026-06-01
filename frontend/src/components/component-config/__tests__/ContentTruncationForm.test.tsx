import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";

import { ContentTruncationForm } from "@/components/component-config/ContentTruncationForm";
import type { ContentTruncationConfig } from "@/lib/schemas/components";

const BASE: ContentTruncationConfig = {
  type: "content_truncation",
  target_selector: "",
  word_count: 100,
  fade_out: false,
};

function setup(overrides?: Partial<ContentTruncationConfig>) {
  const onValidSubmit = vi.fn();
  render(
    <div>
      <ContentTruncationForm
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

describe("ContentTruncationForm", () => {
  it("renders selector, word count, and fade-out fields", () => {
    setup();
    expect(screen.getByLabelText(/target selector/i)).toBeInTheDocument();
    expect(screen.getByLabelText(/word count/i)).toBeInTheDocument();
    expect(
      screen.getByLabelText(/fade out truncated content/i),
    ).toBeInTheDocument();
  });

  it("rejects an empty selector", async () => {
    const user = userEvent.setup();
    const { onValidSubmit } = setup({ word_count: 50 });
    await user.click(screen.getByRole("button", { name: "Submit" }));
    expect(
      screen.getByText(/enter a css selector/i),
    ).toBeInTheDocument();
    expect(onValidSubmit).not.toHaveBeenCalled();
  });

  it("rejects an out-of-range word count", async () => {
    const user = userEvent.setup();
    const { onValidSubmit } = setup();
    await user.type(screen.getByLabelText(/target selector/i), ".body");
    const wc = screen.getByLabelText(/word count/i);
    await user.clear(wc);
    await user.type(wc, "0");
    await user.click(screen.getByRole("button", { name: "Submit" }));
    expect(screen.getByText(/at least 1/i)).toBeInTheDocument();
    expect(onValidSubmit).not.toHaveBeenCalled();
  });

  it("submits a valid content_truncation config with fade_out toggled", async () => {
    const user = userEvent.setup();
    const { onValidSubmit } = setup();
    await user.type(screen.getByLabelText(/target selector/i), ".body");
    const wc = screen.getByLabelText(/word count/i);
    await user.clear(wc);
    await user.type(wc, "250");
    await user.click(screen.getByLabelText(/fade out truncated content/i));
    await user.click(screen.getByRole("button", { name: "Submit" }));

    expect(onValidSubmit).toHaveBeenCalledTimes(1);
    expect(onValidSubmit).toHaveBeenCalledWith({
      type: "content_truncation",
      target_selector: ".body",
      word_count: 250,
      fade_out: true,
    });
  });
});
