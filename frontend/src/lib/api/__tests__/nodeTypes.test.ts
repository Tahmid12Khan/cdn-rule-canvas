import { describe, expect, it } from "vitest";

import { NodeManifest } from "@/lib/api/nodeTypes";

// Regression: expression node types (trim_json/add_attribute/apply_outcome) declare
// a single passthrough output branch with id "out" (NOT yes/no). The manifest schema
// MUST accept it — otherwise NodeManifest.parse throws on the real backend manifest,
// the whole node-type fetch fails, and every node renders as "Unknown node type".
describe("NodeManifest schema", () => {
  const manifestWith = (branchId: string) => ({
    categories: [{ id: "json", label: "JSON" }],
    node_types: [
      {
        kind: "trim_json",
        label: "Trim JSON",
        category: "json",
        applies_to: "json",
        node_kind: "expression",
        summary: "Trims a JSON array.",
        fields: [
          { name: "json_path", label: "JSON path", control: "text", required: true },
          { name: "length", label: "Max length", control: "number", required: true },
        ],
        output: { branches: [{ id: branchId, label: "Next" }] },
      },
    ],
    display: { value_max_chars: 10 },
  });

  it("accepts an expression node's 'out' output branch", () => {
    expect(() => NodeManifest.parse(manifestWith("out"))).not.toThrow();
  });

  it("still accepts decision yes/no branches", () => {
    expect(() => NodeManifest.parse(manifestWith("yes"))).not.toThrow();
  });
});
