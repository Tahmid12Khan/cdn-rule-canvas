import { describe, expect, it } from "vitest";

import { EvalResponse, EvalSummary } from "@/lib/api/evalTest";

// features-matched-spec §v2.2 — the summary reshape (expressions[] + same-shape
// expensive_nodes; the old outcome_ids/outcome_labels are gone).
describe("EvalSummary (v2.2 reshape)", () => {
  const summary = {
    expressions: [
      {
        expression_id: "t_body",
        expression_label: "trim_json",
        custom_expression_label: "",
        expression_time_ms: "0.02",
      },
      {
        expression_id: "a_pw",
        expression_label: "add_attribute",
        custom_expression_label: "show_paywall",
        expression_time_ms: "0.80",
      },
    ],
    time_took_ms: "1.29",
    expensive_nodes: [
      {
        expression_id: "a_pw",
        expression_label: "add_attribute",
        custom_expression_label: "show_paywall",
        expression_time_ms: "0.80",
      },
    ],
  };

  it("parses the new expressions[] shape", () => {
    const parsed = EvalSummary.parse(summary);
    expect(parsed.expressions).toHaveLength(2);
    expect(parsed.expressions[0]).toEqual({
      expression_id: "t_body",
      expression_label: "trim_json",
      custom_expression_label: "",
      expression_time_ms: "0.02",
    });
    expect(parsed.expensive_nodes[0].expression_id).toBe("a_pw");
    expect(parsed.time_took_ms).toBe("1.29");
  });

  it("rejects the old outcome_ids/outcome_labels shape", () => {
    expect(
      EvalSummary.safeParse({
        outcome_ids: ["t_body"],
        outcome_labels: ["Trim JSON"],
        time_took_ms: "1.29",
        expensive_nodes: [],
      }).success,
    ).toBe(false);
  });

  it("is absent-safe on EvalResponse (defaults to null)", () => {
    const res = EvalResponse.parse({
      matched_node_id: null,
      traversed_node_ids: [],
      traversed_edge_ids: [],
      steps: [],
    });
    expect(res.summary).toBeNull();
  });

  it("parses an EvalResponse carrying the new summary", () => {
    const res = EvalResponse.parse({
      matched_node_id: "a_pw",
      traversed_node_ids: ["a_pw"],
      traversed_edge_ids: [],
      steps: [],
      summary,
    });
    expect(res.summary?.expressions[1].custom_expression_label).toBe(
      "show_paywall",
    );
  });
});
