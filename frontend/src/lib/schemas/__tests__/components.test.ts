import { describe, expect, it } from "vitest";

import {
  ComponentConfig,
  ComponentType,
  defaultConfigFor,
} from "@/lib/schemas/components";

describe("ComponentType", () => {
  it("includes the JSON mutation types", () => {
    expect(ComponentType.options).toContain("json_remove");
    expect(ComponentType.options).toContain("json_set");
    expect(ComponentType.options).toContain("json_replace");
  });
});

describe("ComponentConfig — json_remove", () => {
  it("accepts a non-empty target_path", () => {
    const parsed = ComponentConfig.safeParse({
      type: "json_remove",
      target_path: "$.user.premium",
    });
    expect(parsed.success).toBe(true);
  });

  it("rejects an empty target_path", () => {
    const parsed = ComponentConfig.safeParse({
      type: "json_remove",
      target_path: "",
    });
    expect(parsed.success).toBe(false);
  });

  it("rejects a target_path over 500 chars", () => {
    const parsed = ComponentConfig.safeParse({
      type: "json_remove",
      target_path: "$." + "a".repeat(600),
    });
    expect(parsed.success).toBe(false);
  });
});

describe("ComponentConfig — json_set / json_replace", () => {
  it("accepts any JSON value (incl. null)", () => {
    for (const type of ["json_set", "json_replace"] as const) {
      expect(
        ComponentConfig.safeParse({ type, target_path: "$.a", value: null })
          .success,
      ).toBe(true);
      expect(
        ComponentConfig.safeParse({
          type,
          target_path: "$.a",
          value: { nested: [1, 2, 3] },
        }).success,
      ).toBe(true);
      expect(
        ComponentConfig.safeParse({ type, target_path: "$.a", value: "str" })
          .success,
      ).toBe(true);
    }
  });

  it("rejects an empty target_path", () => {
    expect(
      ComponentConfig.safeParse({
        type: "json_set",
        target_path: "",
        value: 1,
      }).success,
    ).toBe(false);
  });
});

describe("defaultConfigFor", () => {
  it("builds empty json_remove defaults", () => {
    expect(defaultConfigFor("json_remove")).toEqual({
      type: "json_remove",
      target_path: "",
    });
  });

  it("builds null-value json_set / json_replace defaults", () => {
    expect(defaultConfigFor("json_set")).toEqual({
      type: "json_set",
      target_path: "",
      value: null,
    });
    expect(defaultConfigFor("json_replace")).toEqual({
      type: "json_replace",
      target_path: "",
      value: null,
    });
  });

  it("still builds the HTML defaults unchanged", () => {
    expect(defaultConfigFor("html_injection").type).toBe("html_injection");
    expect(defaultConfigFor("content_truncation").type).toBe(
      "content_truncation",
    );
  });

  it("builds an html_remove default that keeps the element", () => {
    expect(defaultConfigFor("html_remove")).toEqual({
      type: "html_remove",
      target_selector: "",
      include_selector: false,
    });
  });
});
