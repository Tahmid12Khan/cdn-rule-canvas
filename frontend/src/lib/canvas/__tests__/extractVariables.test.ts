import { describe, expect, it } from "vitest";

import { extractVariables } from "@/lib/canvas/extractVariables";

describe("extractVariables", () => {
  it("returns ordered unique names from escaped mustache tags", () => {
    expect(
      extractVariables("<h1>{{headline}}</h1><p>{{body}}</p>{{headline}}"),
    ).toEqual(["headline", "body"]);
  });

  it("handles triple-brace (raw) tags", () => {
    expect(extractVariables("<div>{{{raw_html}}}</div>")).toEqual(["raw_html"]);
  });

  it("tolerates inner whitespace", () => {
    expect(extractVariables("{{  spaced  }}")).toEqual(["spaced"]);
  });

  it("supports dotted names", () => {
    expect(extractVariables("{{user.name}} {{user.email}}")).toEqual([
      "user.name",
      "user.email",
    ]);
  });

  it("returns an empty array for no variables", () => {
    expect(extractVariables("<p>plain html</p>")).toEqual([]);
  });

  it("preserves first-seen order across mixed forms", () => {
    expect(extractVariables("{{a}}{{{b}}}{{a}}{{c}}")).toEqual(["a", "b", "c"]);
  });
});
