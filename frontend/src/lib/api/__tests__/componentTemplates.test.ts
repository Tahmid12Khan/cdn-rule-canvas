import { http, HttpResponse } from "msw";
import { describe, expect, it } from "vitest";

import { API_BASE } from "@/lib/api/client";
import {
  createComponentTemplate,
  createVersion,
  deleteComponentTemplate,
  deleteVersion,
  getComponentTemplate,
  listComponentTemplates,
  makeDefaultVersion,
  resolveComponent,
  updateVersion,
} from "@/lib/api/componentTemplates";
import { ComponentTemplateCreate } from "@/lib/schemas/componentTemplates";
import { server } from "@/test/mocks/server";

const BASE = `${API_BASE}/api/v1/component-templates`;
const CID = "11111111-1111-1111-1111-111111111111";

const versionSummary = {
  id: "22222222-2222-2222-2222-222222222222",
  version_number: 1,
  description: "v1",
  is_default: true,
  created_at: "2026-06-07T00:00:00Z",
  updated_at: "2026-06-07T00:00:00Z",
};

const detail = {
  id: CID,
  slug: "paywall-cta",
  name: "Paywall CTA",
  description: "Subscribe prompt",
  default_mode: "latest",
  default_version_number: 1,
  latest_version_number: 1,
  versions: [versionSummary],
  created_at: "2026-06-07T00:00:00Z",
  updated_at: "2026-06-07T00:00:00Z",
};

const versionRead = {
  id: versionSummary.id,
  version_number: 1,
  description: "v1",
  html_body: "<h1>{{headline}}</h1>",
  variables: [{ name: "headline", title: "Headline" }],
  is_default: true,
  created_at: "2026-06-07T00:00:00Z",
  updated_at: "2026-06-07T00:00:00Z",
};

const summary = {
  id: CID,
  slug: "paywall-cta",
  name: "Paywall CTA",
  description: "Subscribe prompt",
  default_version_number: 1,
  latest_version_number: 1,
  updated_at: "2026-06-07T00:00:00Z",
};

describe("componentTemplates api", () => {
  it("listComponentTemplates builds page/page_size + trimmed q and parses Page", async () => {
    let requested: URL | null = null;
    server.use(
      http.get(BASE, ({ request }) => {
        requested = new URL(request.url);
        return HttpResponse.json({
          items: [summary],
          page: 1,
          page_size: 20,
          total: 1,
        });
      }),
    );
    const res = await listComponentTemplates({
      page: 1,
      page_size: 20,
      q: "  pay  ",
    });
    expect(requested!.searchParams.get("q")).toBe("pay");
    expect(res.items[0].slug).toBe("paywall-cta");
  });

  it("getComponentTemplate parses ComponentTemplateRead", async () => {
    server.use(http.get(`${BASE}/${CID}`, () => HttpResponse.json(detail)));
    const res = await getComponentTemplate(CID);
    expect(res.default_mode).toBe("latest");
    expect(res.versions).toHaveLength(1);
  });

  it("createComponentTemplate POSTs body and parses read", async () => {
    let body: unknown = null;
    server.use(
      http.post(BASE, async ({ request }) => {
        body = await request.json();
        return HttpResponse.json(detail, { status: 201 });
      }),
    );
    const created = await createComponentTemplate({
      slug: "paywall-cta",
      name: "Paywall CTA",
    });
    expect((body as { slug: string }).slug).toBe("paywall-cta");
    expect(created.id).toBe(CID);
  });

  it("createVersion POSTs to the versions sub-route", async () => {
    let method: string | null = null;
    server.use(
      http.post(`${BASE}/${CID}/versions`, ({ request }) => {
        method = request.method;
        return HttpResponse.json(versionRead, { status: 201 });
      }),
    );
    const v = await createVersion(CID, { make_default: true });
    expect(method).toBe("POST");
    expect(v.version_number).toBe(1);
  });

  it("updateVersion PATCHes the version path", async () => {
    let body: unknown = null;
    server.use(
      http.patch(`${BASE}/${CID}/versions/1`, async ({ request }) => {
        body = await request.json();
        return HttpResponse.json(versionRead);
      }),
    );
    const v = await updateVersion(CID, 1, {
      html_body: "<h1>{{headline}}</h1>",
      variables: [{ name: "headline", title: "Headline" }],
    });
    expect((body as { html_body: string }).html_body).toContain("headline");
    expect(v.variables[0].name).toBe("headline");
  });

  it("makeDefaultVersion POSTs make-default and parses read", async () => {
    server.use(
      http.post(`${BASE}/${CID}/versions/1/make-default`, () =>
        HttpResponse.json({ ...detail, default_mode: "pinned" }),
      ),
    );
    const res = await makeDefaultVersion(CID, 1);
    expect(res.default_mode).toBe("pinned");
  });

  it("resolveComponent hits the resolve route with the version selector", async () => {
    let requested: URL | null = null;
    server.use(
      http.get(`${BASE}/${CID}/resolve`, ({ request }) => {
        requested = new URL(request.url);
        return HttpResponse.json({
          version_number: 1,
          html_body: "<h1>{{headline}}</h1>",
          variables: [{ name: "headline", title: "Headline" }],
        });
      }),
    );
    const res = await resolveComponent(CID, "default");
    expect(requested!.searchParams.get("version")).toBe("default");
    expect(res.version_number).toBe(1);
  });

  it("deleteComponentTemplate + deleteVersion resolve on 204", async () => {
    server.use(
      http.delete(`${BASE}/${CID}`, () =>
        HttpResponse.text("", { status: 204 }),
      ),
      http.delete(`${BASE}/${CID}/versions/1`, () =>
        HttpResponse.text("", { status: 204 }),
      ),
    );
    await expect(deleteComponentTemplate(CID)).resolves.toBeUndefined();
    await expect(deleteVersion(CID, 1)).resolves.toBeUndefined();
  });
});

describe("ComponentTemplateCreate schema", () => {
  const valid = { slug: "paywall-cta", name: "Paywall CTA" };

  it("accepts a minimal valid component", () => {
    expect(ComponentTemplateCreate.safeParse(valid).success).toBe(true);
  });

  it("rejects a non-kebab slug", () => {
    expect(
      ComponentTemplateCreate.safeParse({ ...valid, slug: "Bad_Slug" }).success,
    ).toBe(false);
  });

  it("rejects a too-short slug", () => {
    expect(
      ComponentTemplateCreate.safeParse({ ...valid, slug: "ab" }).success,
    ).toBe(false);
  });
});
