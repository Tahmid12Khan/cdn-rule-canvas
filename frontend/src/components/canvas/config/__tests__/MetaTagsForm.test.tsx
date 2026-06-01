import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";

import { MetaTagsForm } from "@/components/canvas/config/MetaTagsForm";
import type { ProcessorConfig } from "@/lib/canvas/types";

const META: Extract<ProcessorConfig, { type: "meta_tags" }> = {
  type: "meta_tags",
  tag_name: "paywall",
  operator: "contains",
  value: "true",
};

describe("MetaTagsForm", () => {
  it("hides the Value input when operator is 'exists'", async () => {
    const user = userEvent.setup();
    render(<MetaTagsForm initial={META} onChange={() => undefined} />);
    expect(screen.getByLabelText("Value")).toBeInTheDocument();
    await user.selectOptions(screen.getByLabelText("Operator"), "exists");
    expect(screen.queryByLabelText("Value")).not.toBeInTheDocument();
  });

  it("reports invalid when tag name is empty", async () => {
    const onChange = vi.fn();
    const user = userEvent.setup();
    render(
      <MetaTagsForm
        initial={{ ...META, tag_name: "" }}
        onChange={onChange}
      />,
    );
    // Clearing then typing triggers onChange with validity flags.
    await user.clear(screen.getByLabelText("Tag name"));
    const lastCall = onChange.mock.calls.at(-1);
    expect(lastCall?.[1]).toBe(false);
  });

  it("reports valid when tag name + value are present", async () => {
    const onChange = vi.fn();
    const user = userEvent.setup();
    render(<MetaTagsForm initial={META} onChange={onChange} />);
    await user.type(screen.getByLabelText("Tag name"), "x");
    const lastCall = onChange.mock.calls.at(-1);
    expect(lastCall?.[1]).toBe(true);
  });
});
