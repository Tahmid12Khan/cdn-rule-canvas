import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { useState } from "react";
import { describe, expect, it, vi } from "vitest";

import { RichTextEditor } from "@/components/component-config/RichTextEditor";

function Controlled({ onChange }: { onChange?: (v: string) => void }) {
  const [value, setValue] = useState("<p>hello</p>");
  return (
    <RichTextEditor
      value={value}
      onChange={(next) => {
        setValue(next);
        onChange?.(next);
      }}
    />
  );
}

describe("RichTextEditor", () => {
  it("renders Visual and HTML tabs with Visual selected by default", () => {
    render(<Controlled />);
    const visual = screen.getByRole("tab", { name: "Visual" });
    expect(visual).toHaveAttribute("aria-selected", "true");
    expect(screen.getByRole("tab", { name: "HTML" })).toHaveAttribute(
      "aria-selected",
      "false",
    );
  });

  it("renders a sanitized preview in the Visual tab", () => {
    render(<Controlled />);
    // SafeHtml renders the sanitized markup; the text "hello" should be visible.
    expect(screen.getByText("hello")).toBeInTheDocument();
  });

  it("switches to the HTML tab and exposes the raw markup in a code editor", async () => {
    const user = userEvent.setup();
    render(<Controlled />);
    await user.click(screen.getByRole("tab", { name: "HTML" }));
    expect(screen.getByRole("tab", { name: "HTML" })).toHaveAttribute(
      "aria-selected",
      "true",
    );
    expect(screen.getByDisplayValue("<p>hello</p>")).toBeInTheDocument();
  });

  it("propagates edits via onChange", async () => {
    const user = userEvent.setup();
    const onChange = vi.fn();
    render(<Controlled onChange={onChange} />);
    const textarea = screen.getByRole("textbox");
    await user.type(textarea, "!");
    expect(onChange).toHaveBeenCalled();
  });
});
