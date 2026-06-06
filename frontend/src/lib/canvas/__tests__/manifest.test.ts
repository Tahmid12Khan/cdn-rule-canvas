import { describe, expect, it } from "vitest";

import {
  defaultProcessor,
  fieldDisplayValue,
  isFieldRequired,
  isProcessorValid,
  nodeTitle,
  validateProcessor,
} from "@/lib/canvas/manifest";
import { NODE_TYPES_FIXTURE } from "@/test/fixtures/nodeTypes";

const META = NODE_TYPES_FIXTURE.node_types.find((s) => s.kind === "meta_tags")!;
const DEVICE = NODE_TYPES_FIXTURE.node_types.find(
  (s) => s.kind === "device_type",
)!;
const SITE_MATCH = NODE_TYPES_FIXTURE.node_types.find(
  (s) => s.kind === "site_match",
)!;

describe("defaultProcessor", () => {
  it("builds the dropped-node config from field defaults", () => {
    expect(defaultProcessor(META)).toEqual({
      type: "meta_tags",
      tag_name: "",
      operator: "contains",
      value: "",
    });
  });
});

describe("validateProcessor — required", () => {
  it("flags an empty required field with its required_message", () => {
    const errors = validateProcessor(META, defaultProcessor(META));
    expect(errors.tag_name).toBe(
      "Enter the meta tag name to match (e.g. og:type)",
    );
  });

  it("passes when required fields are non-empty", () => {
    const ok = {
      type: "meta_tags",
      tag_name: "paywall",
      operator: "contains",
      value: "true",
    };
    expect(isProcessorValid(META, ok)).toBe(true);
  });

  it("treats whitespace-only strings as empty", () => {
    const errors = validateProcessor(META, {
      type: "meta_tags",
      tag_name: "   ",
      operator: "contains",
      value: "true",
    });
    expect(errors.tag_name).toBeDefined();
  });
});

describe("validateProcessor — required_unless", () => {
  it("requires value when operator !== 'exists'", () => {
    const errors = validateProcessor(META, {
      type: "meta_tags",
      tag_name: "paywall",
      operator: "contains",
      value: "",
    });
    expect(errors.value).toBe(
      "Enter a value to compare against, or switch the operator to 'exists'",
    );
    expect(isFieldRequired(META.fields[2], {
      type: "meta_tags",
      operator: "contains",
    })).toBe(true);
  });

  it("makes value optional when operator === 'exists'", () => {
    const errors = validateProcessor(META, {
      type: "meta_tags",
      tag_name: "paywall",
      operator: "exists",
      value: "",
    });
    expect(errors.value).toBeUndefined();
    expect(isFieldRequired(META.fields[2], {
      type: "meta_tags",
      operator: "exists",
    })).toBe(false);
  });
});

describe("validateProcessor — select options", () => {
  it("rejects a value outside the option set", () => {
    const errors = validateProcessor(DEVICE, {
      type: "device_type",
      operator: "equals",
      value: "smartwatch",
    });
    expect(errors.value).toBeDefined();
  });

  it("does not check option membership when the value is empty (required covers it)", () => {
    const errors = validateProcessor(DEVICE, {
      type: "device_type",
      operator: "equals",
      value: "",
    });
    // The error is the required error, not an option error.
    expect(errors.value).toBeDefined();
    expect(errors.value).not.toMatch(/options/);
  });

  it("accepts a valid option value", () => {
    expect(
      isProcessorValid(DEVICE, {
        type: "device_type",
        operator: "equals",
        value: "mobile",
      }),
    ).toBe(true);
  });
});

describe("display helpers", () => {
  it("nodeTitle uses the manifest label, falling back to the kind", () => {
    expect(nodeTitle(META, { type: "meta_tags" })).toBe("Meta Tags");
    expect(nodeTitle(undefined, { type: "future_kind" })).toBe("future_kind");
  });

  it("fieldDisplayValue maps select values to option labels", () => {
    const op = META.fields[1];
    expect(
      fieldDisplayValue(op, { type: "meta_tags", operator: "contains" }),
    ).toBe("contains");
    expect(fieldDisplayValue(op, { type: "meta_tags" })).toBe("—");
  });

  it("fieldDisplayValue shows a site_select's stored slug verbatim", () => {
    const siteField = SITE_MATCH.fields[0];
    expect(
      fieldDisplayValue(siteField, {
        type: "site_match",
        site: "demo-localhost",
      }),
    ).toBe("demo-localhost");
    expect(fieldDisplayValue(siteField, { type: "site_match" })).toBe("—");
  });
});
