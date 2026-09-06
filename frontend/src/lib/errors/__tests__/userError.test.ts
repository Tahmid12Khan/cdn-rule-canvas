import { describe, expect, it } from "vitest";
import { z } from "zod";

import { ApiError } from "@/lib/api/client";
import { toUserError, zodToUserErrors } from "@/lib/errors/userError";

describe("toUserError", () => {
  it("maps SLUG_CONFLICT to unique-slug guidance with a fieldPath", () => {
    const ue = toUserError(
      new ApiError(409, "SLUG_CONFLICT", "duplicate slug"),
      { surface: "create" },
    );
    expect(ue.title).toMatch(/already taken/i);
    expect(ue.howToFix).toMatch(/unique slug/i);
    expect(ue.fieldPath).toBe("id");
    expect(ue.retryable).toBe(false);
  });

  it("maps VERSION_EDIT_LOCKED to the save-as-new-version path", () => {
    const ue = toUserError(
      new ApiError(409, "VERSION_EDIT_LOCKED", "locked"),
      { surface: "save" },
    );
    expect(ue.title).toMatch(/locked/i);
    expect(ue.howToFix).toMatch(/save as new version/i);
  });

  it("maps a 422 validation error to fix-the-fields guidance", () => {
    const ue = toUserError(
      new ApiError(422, "VALIDATION_ERROR", "invalid", [
        { loc: "rule_graph.canvas.nodes[0]", msg: "bad", rule_id: "x" },
      ]),
      { surface: "save" },
    );
    expect(ue.howToFix).toMatch(/highlighted fields/i);
    expect(ue.retryable).toBe(false);
  });

  it("treats a publish 409 as 'already live'", () => {
    const ue = toUserError(new ApiError(409, "CONFLICT", "conflict"), {
      surface: "publish",
    });
    expect(ue.title).toMatch(/already live/i);
  });

  it("maps a 5xx / INTERNAL_ERROR to a retryable server error", () => {
    const ue = toUserError(new ApiError(500, "INTERNAL_ERROR", "boom"), {
      surface: "create",
    });
    expect(ue.title).toMatch(/server/i);
    expect(ue.retryable).toBe(true);
  });

  it("maps a network TypeError to a reachability message (retryable)", () => {
    const ue = toUserError(new TypeError("Failed to fetch"), {
      surface: "load",
    });
    expect(ue.title).toMatch(/can't reach the server/i);
    expect(ue.retryable).toBe(true);
  });

  it("gives eval-specific copy for an unreachable proxy", () => {
    const ue = toUserError(new TypeError("Failed to fetch"), {
      surface: "eval",
    });
    expect(ue.title).toMatch(/could not reach the evaluator/i);
    expect(ue.howToFix).toMatch(/proxy/i);
  });

  it("falls through to a generic UserError for unknown errors (no stack)", () => {
    const ue = toUserError(new Error("kaboom\n  at foo"));
    expect(ue.title).toBe("Something went wrong");
    expect(ue.why).not.toContain("kaboom");
    expect(ue.retryable).toBe(true);
  });

  it("falls through to a generic UserError for an unknown ApiError code", () => {
    const ue = toUserError(new ApiError(418, "TEAPOT", "I'm a teapot"));
    expect(ue.title).toMatch(/couldn't complete/i);
    expect(ue.why).toContain("I'm a teapot");
  });
});

describe("zodToUserErrors", () => {
  it("keys field messages by the first path segment", () => {
    const schema = z.object({
      slug: z.string().min(1, "Slug needed"),
      name: z.string().min(1, "Name needed"),
    });
    const result = schema.safeParse({ slug: "", name: "" });
    expect(result.success).toBe(false);
    if (result.success) return;
    const mapped = zodToUserErrors(result.error);
    expect(mapped.slug.message).toBe("Slug needed");
    expect(mapped.name.message).toBe("Name needed");
  });
});
