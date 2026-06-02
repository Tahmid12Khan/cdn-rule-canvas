import { render } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import { SafeHtml } from "@/components/ui/SafeHtml";

describe("SafeHtml", () => {
  it("forces rel=noopener noreferrer on anchors that have a target", () => {
    const { container } = render(
      <SafeHtml html={'<a href="https://example.com" target="_blank">x</a>'} />,
    );
    const a = container.querySelector("a");
    expect(a).not.toBeNull();
    expect(a?.getAttribute("rel")).toBe("noopener noreferrer");
  });

  it("strips script tags (stays XSS-safe)", () => {
    const { container } = render(
      <SafeHtml html={'<p>ok</p><script>alert(1)</script>'} />,
    );
    expect(container.querySelector("script")).toBeNull();
    expect(container.textContent).toContain("ok");
  });

  it("drops javascript: hrefs", () => {
    const { container } = render(
      <SafeHtml html={'<a href="javascript:alert(1)">click</a>'} />,
    );
    const a = container.querySelector("a");
    // The dangerous scheme is removed; the anchor (if kept) has no href.
    expect(a?.getAttribute("href") ?? "").not.toContain("javascript:");
  });
});
