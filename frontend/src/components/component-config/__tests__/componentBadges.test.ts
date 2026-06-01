import { describe, expect, it } from "vitest";

import { componentBadges } from "@/lib/canvas/componentBadges";

describe("componentBadges", () => {
  it("derives badges from an html_injection config", () => {
    const badges = componentBadges({
      type: "html_injection",
      target_selector: ".article",
      placement_mode: "append",
      html_body: "<p>x</p>",
      theme: null,
    });
    const labels = badges.map((b) => b.label);
    expect(labels).toContain("HTML Injection");
    expect(labels).toContain("Append");
    expect(labels).toContain(".article");
  });

  it("derives badges from a content_truncation config with fade out", () => {
    const badges = componentBadges({
      type: "content_truncation",
      target_selector: ".body",
      word_count: 120,
      fade_out: true,
    });
    const labels = badges.map((b) => b.label);
    expect(labels).toContain("Content Truncation");
    expect(labels).toContain("120 words");
    expect(labels).toContain("Fade out");
  });

  it("returns an Unknown badge for malformed config", () => {
    const badges = componentBadges({ type: "nope" });
    expect(badges).toEqual([{ label: "Unknown", tone: "info" }]);
  });
});
