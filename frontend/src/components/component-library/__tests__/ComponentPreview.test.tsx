import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import { ComponentPreview } from "@/components/component-library/ComponentPreview";

describe("ComponentPreview", () => {
  it("renders interpolated mustache values", () => {
    render(
      <ComponentPreview
        html="<h1>{{headline}}</h1><p>{{body}}</p>"
        values={{ headline: "Subscribe now", body: "Get full access." }}
      />,
    );
    expect(screen.getByText("Subscribe now")).toBeInTheDocument();
    expect(screen.getByText("Get full access.")).toBeInTheDocument();
  });

  it("renders missing variables as empty", () => {
    const { container } = render(
      <ComponentPreview html="<p>{{absent}}!</p>" values={{}} />,
    );
    expect(container.querySelector("p")?.textContent).toBe("!");
  });

  it("escapes interpolated values (double-brace) so they stay text", () => {
    const { container } = render(
      <ComponentPreview
        html="<div>{{name}}</div>"
        values={{ name: "<script>alert(1)</script>" }}
      />,
    );
    // Mustache escapes the value; SafeHtml then renders it as text, never a
    // live <script> element.
    expect(container.querySelector("script")).toBeNull();
    expect(container.querySelector("div")?.textContent).toContain("script");
  });

  it("falls back to the raw body on a malformed template instead of crashing", () => {
    const { container } = render(
      <ComponentPreview html="<p>{{unclosed</p>" values={{}} />,
    );
    expect(container.querySelector("p")).not.toBeNull();
  });
});
