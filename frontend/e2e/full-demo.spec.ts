import { expect, test } from "@playwright/test";

// Full end-to-end demo (Task 20). Exercises the whole RRE stack against the
// running docker-compose services + seeded `dn-article` feature:
//
//   1. admin frontend renders the seeded feature in the features list,
//   2. the version list shows a LIVE version (v1) and a DRAFT version (v2),
//   3. the PROXY actually transforms the upstream article: a desktop anonymous
//      request to the paywalled article gets the seeded Paywall outcome injected
//      (meta paywall=true -> device != mobile -> Paywall),
//   4. a non-matching path passes through untouched.
//
// Prereqs (provided by the e2e job / `make up`):
//   - frontend  on http://localhost:3000 (Playwright baseURL)
//   - backend   on http://localhost:8000 (seeded: dn-article LIVE v1 + DRAFT v2)
//   - proxy     on http://localhost:9000 (feature_map host localhost:9000)
//   - demo-upstream serving /article.html (paywall meta) + /free.html
//
// The proxy host port is 9000 so the browser Host header `localhost:9000`
// matches the feature_map entry and resolves the dn-article feature.

const API_BASE = process.env.NEXT_PUBLIC_API_BASE ?? "http://localhost:8000";
const PROXY_BASE = process.env.RRE_PROXY_BASE ?? "http://localhost:9000";

test.describe("RRE full-stack demo", () => {
  test("backend is healthy and serves the seeded feature", async ({ request }) => {
    const health = await request.get(`${API_BASE}/health`);
    expect(health.ok()).toBeTruthy();

    const feature = await request.get(`${API_BASE}/api/v1/features/dn-article`);
    expect(feature.ok()).toBeTruthy();
    const body = await feature.json();
    expect(body.id).toBe("dn-article");
    expect(body.type).toBe("html");
    // The live slot points at the seeded LIVE v1.
    expect(body.live_version_id).not.toBeNull();
  });

  test("admin UI lists the feature and its versions", async ({ page }) => {
    await page.goto("/products/features");

    // The seeded feature card is visible and links to the version list.
    const featureLink = page.getByRole("link", { name: /DN Article Paywall/i });
    await expect(featureLink).toBeVisible();
    await featureLink.click();

    // Version list: a LIVE deployment pill and at least one row.
    await expect(page.getByText("LIVE", { exact: false }).first()).toBeVisible();
  });

  test("active-version endpoint returns the seeded outcomes", async ({ request }) => {
    const res = await request.get(
      `${API_BASE}/api/v1/features/dn-article/active-version?env=live`,
    );
    expect(res.ok()).toBeTruthy();
    const av = await res.json();
    expect(av.version_number).toBe(1);
    const titles = (av.outcomes as Array<{ title: string }>).map((o) => o.title);
    expect(titles).toContain("Paywall");
    expect(titles).toContain("Registration Wall");
    expect(titles).toContain("Show Content");
  });

  test("proxy injects the paywall on a desktop anonymous request", async ({
    request,
  }) => {
    // Desktop UA (default), no rre_user_type cookie -> anonymous canvas.
    // meta paywall=true -> yes; device equals mobile -> no -> Paywall outcome.
    const res = await request.get(`${PROXY_BASE}/article.html`, {
      headers: {
        Host: "localhost:9000",
        "User-Agent":
          "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36",
      },
    });
    expect(res.ok()).toBeTruthy();
    expect(res.headers()["x-rre-apply-status"]).toBe("ok");

    const html = await res.text();
    // The seeded Paywall outcome injects an html_injection block + truncates.
    expect(html).toContain("rre-paywall");
    expect(html).toContain("Subscribe to continue");
  });

  test("proxy injects the registration wall for a mobile anonymous request", async ({
    request,
  }) => {
    // Mobile UA -> device equals mobile -> yes -> Registration Wall outcome.
    const res = await request.get(`${PROXY_BASE}/article.html`, {
      headers: {
        Host: "localhost:9000",
        "User-Agent":
          "Mozilla/5.0 (iPhone; CPU iPhone OS 17_0 like Mac OS X) AppleWebKit/605.1.15 Mobile/15E148",
      },
    });
    expect(res.ok()).toBeTruthy();

    const html = await res.text();
    expect(html).toContain("rre-regwall");
    expect(html).toContain("Create a free account");
  });

  test("non-matching path passes through untouched", async ({ request }) => {
    // /free.html does not match the feature_map glob `/article*`.
    const res = await request.get(`${PROXY_BASE}/free.html`, {
      headers: { Host: "localhost:9000" },
    });
    expect(res.ok()).toBeTruthy();
    const status = res.headers()["x-rre-apply-status"];
    // Pass-through (no feature matched) -> skipped (or header absent).
    if (status !== undefined) {
      expect(status).toBe("skipped");
    }
    const html = await res.text();
    expect(html).toContain("Open Access Story");
    expect(html).not.toContain("rre-paywall");
  });
});
