import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import { BrandHeader } from "@/components/BrandHeader";

describe("BrandHeader", () => {
  it("renders the product name", () => {
    render(<BrandHeader />);
    expect(
      screen.getByRole("heading", { name: /response rule engine/i }),
    ).toBeInTheDocument();
  });

  it("renders the brand accent strip", () => {
    const { container } = render(<BrandHeader />);
    expect(container.querySelector(".bg-brand-500")).not.toBeNull();
  });
});
