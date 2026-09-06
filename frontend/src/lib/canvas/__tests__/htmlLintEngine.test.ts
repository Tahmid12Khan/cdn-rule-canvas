import { describe, expect, it } from "vitest";

import { htmlLintEngine } from "@/lib/canvas/htmlLintEngine";

describe("htmlLintEngine", () => {
  it("returns no diagnostics for clean, balanced HTML + valid mustache", () => {
    expect(htmlLintEngine("<div><p>{{headline}}</p></div>")).toEqual([]);
  });

  it("returns no diagnostics for empty input", () => {
    expect(htmlLintEngine("   ")).toEqual([]);
  });

  it("flags an unclosed tag", () => {
    const diags = htmlLintEngine("<div><p>oops");
    const messages = diags.map((d) => d.message).join(" | ");
    expect(messages).toMatch(/Unclosed/);
  });

  it("flags a stray closing tag", () => {
    const diags = htmlLintEngine("<p>ok</p></span>");
    expect(diags.some((d) => /Stray closing tag/.test(d.message))).toBe(true);
  });

  it("ignores void elements in the balance check", () => {
    expect(
      htmlLintEngine('<div><img src="x.png"><br></div>').filter((d) =>
        /Unclosed|Stray/.test(d.message),
      ),
    ).toEqual([]);
  });

  it("flags unbalanced mustache braces", () => {
    const diags = htmlLintEngine("<p>{{headline</p>");
    expect(diags.some((d) => /Unbalanced mustache/.test(d.message))).toBe(true);
  });

  it("flags an empty mustache expression", () => {
    const diags = htmlLintEngine("<p>{{   }}</p>");
    expect(diags.some((d) => /Empty mustache/.test(d.message))).toBe(true);
  });

  it("produces character offsets within the source range", () => {
    const html = "<div><p>oops";
    for (const d of htmlLintEngine(html)) {
      expect(d.from).toBeGreaterThanOrEqual(0);
      expect(d.to).toBeLessThanOrEqual(html.length);
      expect(d.from).toBeLessThanOrEqual(d.to);
    }
  });
});
