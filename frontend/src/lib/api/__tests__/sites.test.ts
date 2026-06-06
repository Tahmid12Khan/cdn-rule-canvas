import { http, HttpResponse } from "msw";
import { describe, expect, it } from "vitest";

import { API_BASE } from "@/lib/api/client";
import {
  createSite,
  deleteSite,
  listSites,
  searchSites,
  SiteCreate,
  type SiteRead,
  updateSite,
} from "@/lib/api/sites";
import { server } from "@/test/mocks/server";

const SITES_URL = `${API_BASE}/api/v1/sites`;

const sample: SiteRead = {
  slug: "demo-localhost",
  name: "Demo (localhost:9000)",
  source_protocol: "http",
  source_host: "localhost",
  source_port: 9000,
  dest_protocol: "http",
  dest_host: "demo-upstream",
  dest_port: 8081,
  headers: {},
  created_at: "2026-06-06T00:00:00Z",
  updated_at: "2026-06-06T00:00:00Z",
};

function page(items: SiteRead[], total = items.length) {
  return { items, page: 1, page_size: 20, total };
}

describe("sites api", () => {
  it("listSites builds the page/page_size query and parses the envelope", async () => {
    let requested: URL | null = null;
    server.use(
      http.get(SITES_URL, ({ request }) => {
        requested = new URL(request.url);
        return HttpResponse.json(page([sample]));
      }),
    );

    const res = await listSites({ page: 2, page_size: 5 });
    expect(requested!.searchParams.get("page")).toBe("2");
    expect(requested!.searchParams.get("page_size")).toBe("5");
    // No q param when omitted.
    expect(requested!.searchParams.has("q")).toBe(false);
    expect(res.items[0].slug).toBe("demo-localhost");
    expect(res.items[0].source_port).toBe(9000);
  });

  it("listSites sends a trimmed q only when non-empty", async () => {
    let requested: URL | null = null;
    server.use(
      http.get(SITES_URL, ({ request }) => {
        requested = new URL(request.url);
        return HttpResponse.json(page([]));
      }),
    );

    await listSites({ page: 1, page_size: 20, q: "  demo  " });
    expect(requested!.searchParams.get("q")).toBe("demo");
  });

  it("searchSites wraps list with a small page_size and the query", async () => {
    let requested: URL | null = null;
    server.use(
      http.get(SITES_URL, ({ request }) => {
        requested = new URL(request.url);
        return HttpResponse.json(page([sample]));
      }),
    );

    await searchSites("loc");
    expect(requested!.searchParams.get("q")).toBe("loc");
    expect(requested!.searchParams.get("page_size")).toBe("10");
  });

  it("createSite POSTs the body and parses SiteRead", async () => {
    let body: unknown = null;
    server.use(
      http.post(SITES_URL, async ({ request }) => {
        body = await request.json();
        return HttpResponse.json(sample, { status: 201 });
      }),
    );

    const created = await createSite({
      slug: "demo-localhost",
      name: "Demo (localhost:9000)",
      source_protocol: "http",
      source_host: "localhost",
      source_port: 9000,
      dest_protocol: "http",
      dest_host: "demo-upstream",
      dest_port: 8081,
      headers: {},
    });
    expect((body as { slug: string }).slug).toBe("demo-localhost");
    expect(created.name).toBe("Demo (localhost:9000)");
  });

  it("updateSite PATCHes the slug path", async () => {
    let method: string | null = null;
    server.use(
      http.patch(`${SITES_URL}/demo-localhost`, ({ request }) => {
        method = request.method;
        return HttpResponse.json({ ...sample, name: "Renamed" });
      }),
    );

    const updated = await updateSite("demo-localhost", { name: "Renamed" });
    expect(method).toBe("PATCH");
    expect(updated.name).toBe("Renamed");
  });

  it("deleteSite resolves on 204", async () => {
    server.use(
      http.delete(`${SITES_URL}/demo-localhost`, () =>
        HttpResponse.text("", { status: 204 }),
      ),
    );
    await expect(deleteSite("demo-localhost")).resolves.toBeUndefined();
  });
});

describe("SiteCreate schema", () => {
  const valid = {
    slug: "demo-localhost",
    name: "Demo",
    source_protocol: "http",
    source_host: "localhost",
    source_port: "9000",
    dest_protocol: "https",
    dest_host: "demo-upstream",
    dest_port: "8081",
  };

  it("coerces string ports to numbers", () => {
    const parsed = SiteCreate.parse(valid);
    expect(parsed.source_port).toBe(9000);
    expect(parsed.dest_port).toBe(8081);
  });

  it("rejects a non-kebab slug", () => {
    expect(SiteCreate.safeParse({ ...valid, slug: "Bad_Slug" }).success).toBe(
      false,
    );
  });

  it("rejects an out-of-range port", () => {
    expect(
      SiteCreate.safeParse({ ...valid, source_port: "70000" }).success,
    ).toBe(false);
    expect(SiteCreate.safeParse({ ...valid, dest_port: "0" }).success).toBe(
      false,
    );
  });

  it("rejects a protocol outside http/https", () => {
    expect(
      SiteCreate.safeParse({ ...valid, source_protocol: "ftp" }).success,
    ).toBe(false);
  });
});
