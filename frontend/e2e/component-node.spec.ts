import { expect, test, type Locator, type Page } from "@playwright/test";

// Component-node integration E2E (component-editor design §5.4 / §6 deliverable).
// Exercises the apply_component node path end-to-end through the admin UI:
//
//   1. author a Component in the global library (create → edit HTML with a
//      `{{variable}}` → annotate the variable → Save → reopen → it persisted),
//   2. use it in an HTML rule: open the seeded `demo-article` DRAFT version,
//      enter edit mode, drop an `apply_component` node, pick the component +
//      "Default" version, fill the variable's value,
//   3. see the live preview: the node form's <ComponentPreview> renders the
//      authored markup with the entered value (the client render mirrors the
//      proxy's request-time render — the "journey reflects rendered output"
//      check, design §5.3/§5.4),
//   4. save the graph (Save → "Saved ·").
//
// Requires the full docker-compose stack (frontend + backend + seeded
// `demo-article` LIVE v1 + DRAFT v2) on the configured baseURL, exactly like
// canvas-roundtrip.spec.ts / full-demo.spec.ts. NOT part of `make frontend-check`
// (lint/typecheck/test/build) — run via `npm run e2e` against `make up`.

// The seed creates `demo-article` with an editable DRAFT at version 2.
const DRAFT_VERSION_URL = "/products/features/html/demo-article/2";

// Unique per run so re-runs don't trip the slug UNIQUE constraint (components
// have no quick teardown between runs).
const RUN = Date.now().toString(36);
const SLUG = `e2e-cta-${RUN}`;
const NAME = `E2E CTA ${RUN}`;
const HEADLINE = "Subscribe today";
// The authored mustache body: one escaped {{headline}} interpolation.
const HTML_BODY = `<div class="e2e-cta"><h2>{{headline}}</h2></div>`;

// Type a value into a CodeMirror 6 editor. Its content lives in a focusable
// `.cm-content` contenteditable; selectAll+type replaces the whole document.
async function setCodeMirror(page: Page, value: string) {
  const cm = page.locator(".cm-content");
  await cm.click();
  await page.keyboard.press(
    process.platform === "darwin" ? "Meta+A" : "Control+A",
  );
  await page.keyboard.press("Delete");
  await cm.pressSequentially(value);
}

// Drop a palette chip onto the canvas. The chip stashes its ChipPayload JSON on
// the DataTransfer at dragstart; the canvas reads it at drop. Native HTML5 DnD
// is flaky to synthesize via mouse moves, so we fire dragstart → drop sharing a
// single DataTransfer instance in-page (the deterministic pattern for the
// `application/rre-chip` payload the RuleBuilderCanvas onDrop expects).
async function dropChip(page: Page, chip: Locator, canvas: Locator) {
  const chipHandle = await chip.elementHandle();
  const canvasHandle = await canvas.elementHandle();
  if (!chipHandle || !canvasHandle) throw new Error("chip/canvas not found");
  await page.evaluate(
    ([source, target]) => {
      const dt = new DataTransfer();
      source.dispatchEvent(
        new DragEvent("dragstart", { bubbles: true, dataTransfer: dt }),
      );
      const rect = target.getBoundingClientRect();
      const init = {
        bubbles: true,
        cancelable: true,
        dataTransfer: dt,
        clientX: rect.left + rect.width / 2,
        clientY: rect.top + rect.height / 2,
      } as const;
      target.dispatchEvent(new DragEvent("dragover", init));
      target.dispatchEvent(new DragEvent("drop", init));
      source.dispatchEvent(
        new DragEvent("dragend", { bubbles: true, dataTransfer: dt }),
      );
    },
    [chipHandle, canvasHandle] as const,
  );
}

test.describe("component-node integration", () => {
  test("author a component, use it in an HTML rule, see live preview, save", async ({
    page,
  }) => {
    // ── 1. Author a Component in the library ─────────────────────────────────
    await page.goto("/products/components");
    await expect(
      page.getByRole("heading", { name: "Components", exact: true }),
    ).toBeVisible();

    // Open the create modal (header button, or the empty-state action).
    await page
      .getByRole("button", { name: /Create a Component/ })
      .first()
      .click();
    await page.getByLabel("Slug").fill(SLUG);
    await page.getByLabel("Name").fill(NAME);
    await page.getByRole("button", { name: "Create Component" }).click();

    // On success the modal navigates to the new component's editor.
    await expect(page).toHaveURL(new RegExp(`/products/components/${SLUG}$`));
    await expect(page.getByRole("heading", { name: NAME })).toBeVisible();

    // Author the HTML body with a {{headline}} mustache variable.
    await setCodeMirror(page, HTML_BODY);

    // The extracted variable shows up in the Variables panel; annotate its
    // title. The panel labels its title input by the variable name.
    const titleInput = page.getByLabel("Title for headline");
    await expect(titleInput).toBeVisible();
    await titleInput.fill("Headline");

    // Save the version (advisory lint never blocks Save).
    await page.getByRole("button", { name: "Save", exact: true }).click();
    // After a successful save the Save button returns to disabled (not dirty).
    await expect(
      page.getByRole("button", { name: "Save", exact: true }),
    ).toBeDisabled();

    // Reload → the authored body + variable persisted.
    await page.reload();
    await expect(page.getByLabel("Title for headline")).toHaveValue("Headline");

    // ── 2. Use it in an HTML rule ────────────────────────────────────────────
    await page.goto(DRAFT_VERSION_URL);
    await expect(
      page.getByRole("heading", { name: /Version 2/ }),
    ).toBeVisible();

    // Enter edit mode (only DRAFT versions are editable).
    await page.getByRole("button", { name: "Edit" }).click();

    // Open the Content palette tab and drop the Component (apply_component) chip.
    await page.getByRole("tab", { name: "Content" }).click();
    const componentChip = page.locator('[data-chip-id="node:apply_component"]');
    await expect(componentChip).toBeVisible();
    const canvas = page.getByRole("region", { name: /Rule canvas/ });
    await dropChip(page, componentChip, canvas);

    // A new expression node renders on the canvas.
    const expressionNode = page.getByTestId("expression-node").last();
    await expect(expressionNode).toBeVisible();

    // ── 3. Configure: pick component + version, fill the variable ────────────
    await expressionNode.dblclick();

    // Pick the authored component from the dynamic component_select dropdown.
    await page.getByLabel("Component", { exact: true }).selectOption({
      label: NAME,
    });
    // Version stays at the default (follows component).
    await expect(page.getByLabel("Version", { exact: true })).toHaveValue(
      "default",
    );
    // Fill a CSS target selector (required by the manifest).
    await page.getByLabel("Target selector").fill("main");

    // The Variables sub-form resolves the component's declared variables; fill
    // the headline value.
    const headlineInput = page.getByLabel("Headline", { exact: true });
    await expect(headlineInput).toBeVisible();
    await headlineInput.fill(HEADLINE);

    // ── 4. Live preview reflects the rendered output ─────────────────────────
    // <ComponentPreview> renders the mustache body with the entered value — the
    // same render the proxy performs at request time (design §5.3/§5.4).
    const previewWrap = page.getByTestId("component-preview-wrap");
    await expect(previewWrap).toBeVisible();
    await expect(previewWrap).toContainText(HEADLINE);

    // Save the node config back onto the canvas.
    await page.getByRole("button", { name: "Save", exact: true }).click();

    // ── 5. Save the graph ────────────────────────────────────────────────────
    // The inline Save persists the DRAFT rule_graph; "Saved ·" confirms it.
    await page
      .getByRole("button", { name: /^Save$/ })
      .last()
      .click();
    await expect(page.getByText(/Saved ·/)).toBeVisible();

    // Reload → the apply_component node persisted on the DRAFT canvas.
    await page.reload();
    await page.getByRole("button", { name: "Edit" }).click();
    await expect(page.getByTestId("expression-node").last()).toBeVisible();
  });
});
