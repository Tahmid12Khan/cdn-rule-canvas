import { describe, expect, it } from "vitest";

import {
  CUSTOM_LABEL_MESSAGE,
  validateCustomLabel,
} from "@/lib/canvas/customLabel";

// spec §v2.3 — snake_case (^[a-z0-9]+(_[a-z0-9]+)*$); empty/absent is VALID.
describe("validateCustomLabel", () => {
  it("accepts empty (no custom name)", () => {
    expect(validateCustomLabel("")).toBeNull();
  });

  it("accepts valid snake_case values", () => {
    for (const v of [
      "paywall",
      "show_paywall",
      "a",
      "a1",
      "trim_json_body",
      "v2_thing_3",
    ]) {
      expect(validateCustomLabel(v)).toBeNull();
    }
  });

  it("rejects uppercase with the locked message", () => {
    expect(validateCustomLabel("Paywall")).toBe(CUSTOM_LABEL_MESSAGE);
    expect(validateCustomLabel("showPaywall")).toBe(CUSTOM_LABEL_MESSAGE);
  });

  it("rejects spaces with the locked message", () => {
    expect(validateCustomLabel("show paywall")).toBe(CUSTOM_LABEL_MESSAGE);
  });

  it("rejects leading / trailing / double underscores", () => {
    for (const v of ["_paywall", "paywall_", "show__paywall"]) {
      expect(validateCustomLabel(v)).toBe(CUSTOM_LABEL_MESSAGE);
    }
  });
});
