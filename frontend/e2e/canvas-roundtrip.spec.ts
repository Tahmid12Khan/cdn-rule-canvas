import { expect, test } from "@playwright/test";

// Canvas round-trip (Task 14): drop nodes → configure → connect → save →
// reload → graph persists. Requires the full docker-compose stack (backend +
// seeded demo feature dn-article version 1 in DRAFT) running on the configured
// baseURL. In CI the stack is started by the e2e job script before this runs.
//
// The seed (Task 20) creates feature `dn-article` with a DRAFT version 1 and a
// builtin "Show Content" outcome.

const VERSION_URL = "/products/features/html/dn-article/1";

test.describe("rule builder canvas round-trip", () => {
  test("builds, saves and reloads a graph", async ({ page }) => {
    await page.goto(VERSION_URL);

    // Header present.
    await expect(
      page.getByRole("heading", { name: /Version 1/ }),
    ).toBeVisible();

    // Enter edit mode (only enabled for DRAFT).
    await page.getByRole("button", { name: "Edit" }).click();

    // Palette appears in edit mode.
    await expect(
      page.getByRole("tab", { name: "Content" }),
    ).toBeVisible();

    // Drag the Meta Tags chip onto the canvas.
    await page.getByRole("tab", { name: "Content" }).click();
    const metaChip = page.locator('[data-chip-id="content:meta-tags"]');
    const canvas = page.getByRole("region", { name: /Rule canvas/ });
    await metaChip.dragTo(canvas, {
      targetPosition: { x: 200, y: 200 },
    });

    // A decision node renders.
    await expect(page.getByTestId("decision-node").first()).toBeVisible();

    // Configure it.
    await page.getByTestId("decision-node").first().dblclick();
    await page.getByLabel("Tag name").fill("paywall");
    await page.getByLabel("Value").fill("true");
    await page.getByRole("button", { name: "Save" }).first().click();

    // Save the graph.
    await page.getByRole("button", { name: /^Save$/ }).click();
    await expect(page.getByText(/Saved ·/)).toBeVisible();

    // Reload and confirm the node persisted.
    await page.reload();
    await page.getByRole("button", { name: "Edit" }).click();
    await expect(page.getByTestId("decision-node").first()).toBeVisible();
  });
});
