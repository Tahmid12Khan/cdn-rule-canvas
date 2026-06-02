import { describe, expect, it } from "vitest";

import type { NodeManifest } from "@/lib/api/nodeTypes";
import { buildPalette } from "@/lib/canvas/nodeTemplates";
import { NODE_TYPES_FIXTURE } from "@/test/fixtures/nodeTypes";

const OUTCOMES = [
  { id: "11111111-1111-1111-1111-111111111111", title: "Show Content" },
];

describe("buildPalette (manifest-driven)", () => {
  it("returns an empty palette before the manifest loads", () => {
    expect(buildPalette(undefined, OUTCOMES)).toEqual([]);
  });

  it("builds a category per manifest category, in order (no injected Outcomes category)", () => {
    const palette = buildPalette(NODE_TYPES_FIXTURE, OUTCOMES, "json");
    const ids = palette.map((c) => c.id);
    // Apply Outcome is now a manifest expression node — no dynamic Outcomes
    // category. With a JSON feature, html-only categories are dropped.
    expect(ids).toEqual(["session", "user", "content", "json", "advanced"]);
  });

  it("places enabled decision chips with a default processor", () => {
    const palette = buildPalette(NODE_TYPES_FIXTURE, OUTCOMES);
    const content = palette.find((c) => c.id === "content")!;
    const metaChip = content.chips.find((c) => c.label === "Meta Tags")!;
    expect(metaChip.enabled).toBe(true);
    expect(metaChip.payload).toEqual({
      kind: "decision",
      processor: { type: "meta_tags", tag_name: "", operator: "contains", value: "" },
    });
  });

  it("builds an expression chip for an expression node_kind (Trim JSON)", () => {
    const palette = buildPalette(NODE_TYPES_FIXTURE, OUTCOMES, "json");
    const json = palette.find((c) => c.id === "json")!;
    const trim = json.chips.find((c) => c.label === "Trim JSON")!;
    expect(trim.enabled).toBe(true);
    expect(trim.payload).toEqual({
      kind: "expression",
      action: { type: "trim_json", json_path: "", length: 0 },
    });
  });

  it("builds an Apply expression chip in the content category", () => {
    const palette = buildPalette(NODE_TYPES_FIXTURE, OUTCOMES);
    const content = palette.find((c) => c.id === "content")!;
    const apply = content.chips.find((c) => c.label === "Apply")!;
    expect(apply.enabled).toBe(true);
    expect(apply.payload).toEqual({
      kind: "expression",
      action: { type: "apply_outcome", outcome_id: "" },
    });
  });

  it("renders coming_soon categories with no node types as a disabled placeholder", () => {
    const palette = buildPalette(NODE_TYPES_FIXTURE, OUTCOMES);
    const user = palette.find((c) => c.id === "user")!;
    expect(user.chips).toHaveLength(1);
    expect(user.chips[0].enabled).toBe(false);
  });

  // Headline acceptance: a brand-new node type appears with ZERO code change.
  it("surfaces a new decision node type as an enabled chip", () => {
    const manifest: NodeManifest = {
      categories: [{ id: "content", label: "Content" }],
      node_types: [
        {
          kind: "referrer",
          label: "Referrer",
          category: "content",
          summary: "Matches the HTTP referrer.",
          fields: [
            {
              name: "value",
              label: "Value",
              control: "text",
              required: true,
              default: "",
            },
          ],
          output: {
            branches: [
              { id: "yes", label: "Yes" },
              { id: "no", label: "No" },
            ],
          },
        },
      ],
    };
    const palette = buildPalette(manifest, []);
    const content = palette.find((c) => c.id === "content")!;
    const chip = content.chips.find((c) => c.label === "Referrer")!;
    expect(chip.enabled).toBe(true);
    expect(chip.payload).toEqual({
      kind: "decision",
      processor: { type: "referrer", value: "" },
    });
  });
});
