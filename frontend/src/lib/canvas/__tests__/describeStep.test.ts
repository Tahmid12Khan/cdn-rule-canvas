import { describe, expect, it } from "vitest";

import { describeStep } from "@/lib/canvas/describeStep";
import type { ProcessorConfig } from "@/lib/canvas/types";

// describeStep produces the plain-English Transformation Journey sentence. These
// cover the Component node cases (component-editor design §5.4 item 5) alongside
// the pre-existing apply_outcome phrasing for parity.
describe("describeStep — apply_component / apply_component_json", () => {
  it("apply_component uses the cached name + selector + placement", () => {
    const cfg: ProcessorConfig = {
      type: "apply_component",
      component_id: "c-1",
      version: "default",
      variables: { headline: "Hi" },
      target_selector: "main .article-body",
      placement_mode: "append",
    };
    // `label` carries the resolved component name (TransformationJourney passes
    // componentName ?? step.label).
    const out = describeStep("expression", cfg, null, "Paywall CTA");
    expect(out).toBe(
      "Rendered the component ‘Paywall CTA’ and applied it (append) at `main .article-body`.",
    );
  });

  it("apply_component falls back to a generic target/placement when unset", () => {
    const cfg: ProcessorConfig = {
      type: "apply_component",
      component_id: "c-1",
      version: "default",
    };
    const out = describeStep("expression", cfg, null, "Banner");
    expect(out).toBe(
      "Rendered the component ‘Banner’ and applied it (append) at `the page`.",
    );
  });

  it("apply_component_json uses the cached name + target path", () => {
    const cfg: ProcessorConfig = {
      type: "apply_component_json",
      component_id: "c-2",
      version: 3,
      variables: {},
      target_path: "$.content.html",
    };
    const out = describeStep("expression", cfg, null, "Inline Promo");
    expect(out).toBe(
      "Rendered the component ‘Inline Promo’ and set it at `$.content.html`.",
    );
  });

  it("apply_component_json falls back to $ when the path is unset", () => {
    const cfg: ProcessorConfig = {
      type: "apply_component_json",
      component_id: "c-2",
      version: "default",
    };
    const out = describeStep("expression", cfg, null, "Inline Promo");
    expect(out).toBe(
      "Rendered the component ‘Inline Promo’ and set it at `$`.",
    );
  });

  it("still renders the apply_outcome phrasing (unchanged)", () => {
    const cfg: ProcessorConfig = {
      type: "apply_outcome",
      outcome_id: "o-1",
    };
    expect(describeStep("expression", cfg, null, "Show Paywall")).toBe(
      "Applied the outcome ‘Show Paywall’.",
    );
  });
});
