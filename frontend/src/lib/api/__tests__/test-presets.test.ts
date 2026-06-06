import { http, HttpResponse } from "msw";
import { describe, expect, it } from "vitest";

import { API_BASE } from "@/lib/api/client";
import {
  createTestPreset,
  deleteTestPreset,
  listTestPresets,
  TestPresetCreate,
  type TestPresetRead,
  updateTestPreset,
} from "@/lib/api/test-presets";
import { server } from "@/test/mocks/server";

const PRESETS_URL = `${API_BASE}/api/v1/test-presets`;

const sample: TestPresetRead = {
  slug: "mobile-paywall",
  name: "Mobile paywall",
  kind: "rule",
  payload: { feature_type: "html", device_type: "mobile" },
  created_at: "2026-06-06T00:00:00Z",
  updated_at: "2026-06-06T00:00:00Z",
};

function page(items: TestPresetRead[], total = items.length) {
  return { items, page: 1, page_size: 20, total };
}

describe("test-presets api", () => {
  it("listTestPresets builds page/page_size and parses the envelope", async () => {
    let requested: URL | null = null;
    server.use(
      http.get(PRESETS_URL, ({ request }) => {
        requested = new URL(request.url);
        return HttpResponse.json(page([sample]));
      }),
    );

    const res = await listTestPresets({ page: 2, page_size: 5 });
    expect(requested!.searchParams.get("page")).toBe("2");
    expect(requested!.searchParams.get("page_size")).toBe("5");
    expect(requested!.searchParams.has("q")).toBe(false);
    expect(requested!.searchParams.has("kind")).toBe(false);
    expect(res.items[0].slug).toBe("mobile-paywall");
  });

  it("listTestPresets sends trimmed q and kind when provided", async () => {
    let requested: URL | null = null;
    server.use(
      http.get(PRESETS_URL, ({ request }) => {
        requested = new URL(request.url);
        return HttpResponse.json(page([]));
      }),
    );

    await listTestPresets({ page: 1, page_size: 20, q: "  mob  ", kind: "url" });
    expect(requested!.searchParams.get("q")).toBe("mob");
    expect(requested!.searchParams.get("kind")).toBe("url");
  });

  it("createTestPreset POSTs the body and parses TestPresetRead", async () => {
    let body: unknown = null;
    server.use(
      http.post(PRESETS_URL, async ({ request }) => {
        body = await request.json();
        return HttpResponse.json(sample, { status: 201 });
      }),
    );

    const created = await createTestPreset({
      slug: "mobile-paywall",
      name: "Mobile paywall",
      kind: "rule",
      payload: { feature_type: "html", device_type: "mobile" },
    });
    expect((body as { slug: string }).slug).toBe("mobile-paywall");
    expect(created.kind).toBe("rule");
  });

  it("updateTestPreset PATCHes the slug path", async () => {
    let method: string | null = null;
    let body: unknown = null;
    server.use(
      http.patch(`${PRESETS_URL}/mobile-paywall`, async ({ request }) => {
        method = request.method;
        body = await request.json();
        return HttpResponse.json({ ...sample, name: "Renamed" });
      }),
    );

    const updated = await updateTestPreset("mobile-paywall", {
      name: "Renamed",
    });
    expect(method).toBe("PATCH");
    expect((body as { name: string }).name).toBe("Renamed");
    expect(updated.name).toBe("Renamed");
  });

  it("deleteTestPreset resolves on 204", async () => {
    server.use(
      http.delete(`${PRESETS_URL}/mobile-paywall`, () =>
        HttpResponse.text("", { status: 204 }),
      ),
    );
    await expect(deleteTestPreset("mobile-paywall")).resolves.toBeUndefined();
  });
});

describe("TestPresetCreate schema", () => {
  const valid = {
    slug: "mobile-paywall",
    name: "Mobile paywall",
    kind: "rule" as const,
    payload: { feature_type: "html" },
  };

  it("accepts a valid preset", () => {
    expect(TestPresetCreate.safeParse(valid).success).toBe(true);
  });

  it("rejects a non-kebab slug", () => {
    expect(
      TestPresetCreate.safeParse({ ...valid, slug: "Bad_Slug" }).success,
    ).toBe(false);
  });

  it("rejects a kind outside rule/url", () => {
    expect(
      TestPresetCreate.safeParse({ ...valid, kind: "other" }).success,
    ).toBe(false);
  });
});
