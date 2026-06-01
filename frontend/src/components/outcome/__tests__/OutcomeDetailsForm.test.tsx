import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { useState } from "react";
import { describe, expect, it } from "vitest";

import {
  OutcomeDetailsForm,
  validateOutcomeDetails,
  type OutcomeDetailsValue,
} from "@/components/outcome/OutcomeDetailsForm";

function Harness({ initial }: { initial: OutcomeDetailsValue }) {
  const [value, setValue] = useState(initial);
  return <OutcomeDetailsForm value={value} onChange={setValue} />;
}

describe("validateOutcomeDetails", () => {
  it("flags an empty title", () => {
    const errors = validateOutcomeDetails({ title: "", description: "" });
    expect(errors.title).toMatch(/give this outcome a title/i);
  });

  it("flags a whitespace-only title", () => {
    expect(
      validateOutcomeDetails({ title: "   ", description: "" }),
    ).toHaveProperty("title");
  });

  it("flags an over-long description", () => {
    const errors = validateOutcomeDetails({
      title: "ok",
      description: "x".repeat(501),
    });
    expect(errors.description).toMatch(/under 500 characters/);
  });

  it("passes a valid value", () => {
    expect(
      validateOutcomeDetails({ title: "Regwall", description: "fine" }),
    ).toEqual({});
  });
});

describe("OutcomeDetailsForm", () => {
  it("shows a validation error when the title is cleared", async () => {
    const user = userEvent.setup();
    render(<Harness initial={{ title: "Regwall", description: "" }} />);

    const title = screen.getByLabelText(/title/i);
    await user.clear(title);

    expect(screen.getByRole("alert")).toHaveTextContent(
      /give this outcome a title/i,
    );
  });

  it("updates the character count as the description changes", async () => {
    const user = userEvent.setup();
    render(<Harness initial={{ title: "Regwall", description: "" }} />);

    await user.type(screen.getByLabelText(/description/i), "hello");
    expect(screen.getByText("5/500")).toBeInTheDocument();
  });
});
