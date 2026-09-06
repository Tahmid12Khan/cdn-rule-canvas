import { screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { http, HttpResponse } from "msw";
import { describe, expect, it, vi } from "vitest";

import {
  ComponentRefForm,
  type ComponentRefAnyConfig,
} from "@/components/component-config/ComponentRefForm";
import { server } from "@/test/mocks/server";
import { renderWithQuery } from "@/test/renderWithQuery";
import { defaultConfigFor } from "@/lib/schemas/components";

const API_BASE = process.env.NEXT_PUBLIC_API_BASE ?? "http://localhost:8000";
const COMPONENT_ID = "11111111-1111-1111-1111-111111111111";
const FORM_ID = "component-config-form";

// Mock the three endpoints the form depends on: list (picker), detail (version
// select), resolve (declared variables + preview html).
function mockComponentEndpoints() {
  server.use(
    http.get(`${API_BASE}/api/v1/component-templates`, () =>
      HttpResponse.json({
        items: [
          {
            id: COMPONENT_ID,
            slug: "paywall-cta",
            name: "Paywall CTA",
            description: null,
            default_version_number: 2,
            latest_version_number: 2,
            updated_at: "2026-06-07T00:00:00Z",
          },
        ],
        page: 1,
        page_size: 100,
        total: 1,
      }),
    ),
    http.get(`${API_BASE}/api/v1/component-templates/${COMPONENT_ID}`, () =>
      HttpResponse.json({
        id: COMPONENT_ID,
        slug: "paywall-cta",
        name: "Paywall CTA",
        description: null,
        default_mode: "latest",
        default_version_number: 2,
        latest_version_number: 2,
        versions: [
          {
            id: "v1",
            version_number: 1,
            description: null,
            is_default: false,
            created_at: "2026-06-07T00:00:00Z",
            updated_at: "2026-06-07T00:00:00Z",
          },
          {
            id: "v2",
            version_number: 2,
            description: null,
            is_default: true,
            created_at: "2026-06-07T00:00:00Z",
            updated_at: "2026-06-07T00:00:00Z",
          },
        ],
        created_at: "2026-06-07T00:00:00Z",
        updated_at: "2026-06-07T00:00:00Z",
      }),
    ),
    http.get(
      `${API_BASE}/api/v1/component-templates/${COMPONENT_ID}/resolve`,
      () =>
        HttpResponse.json({
          version_number: 2,
          html_body: "<h1>{{headline}}</h1><a>{{cta}}</a>",
          variables: [
            { name: "headline", title: "Headline", description: "The big text" },
            { name: "cta", title: "Call to action" },
          ],
        }),
    ),
  );
}

function Harness({
  type,
  onValidSubmit,
}: {
  type: "component_ref" | "component_ref_json";
  onValidSubmit: (c: ComponentRefAnyConfig) => void;
}) {
  const initial: ComponentRefAnyConfig =
    type === "component_ref_json"
      ? defaultConfigFor("component_ref_json")
      : defaultConfigFor("component_ref");
  return (
    <>
      <ComponentRefForm
        formId={FORM_ID}
        type={type}
        initial={initial}
        onValidSubmit={onValidSubmit}
      />
      <button type="submit" form={FORM_ID}>
        Save
      </button>
    </>
  );
}

describe("ComponentRefForm", () => {
  it("round-trips the variable value-map and selector into a component_ref config", async () => {
    mockComponentEndpoints();
    const onValidSubmit = vi.fn();
    const user = userEvent.setup();

    renderWithQuery(<Harness type="component_ref" onValidSubmit={onValidSubmit} />);

    await screen.findByRole("option", { name: "Paywall CTA" });
    await user.selectOptions(screen.getByLabelText("Component"), COMPONENT_ID);

    // Declared variables render labelled by their title, with description help.
    const headline = (await screen.findByLabelText(
      "Headline",
    )) as HTMLInputElement;
    expect(screen.getByText("The big text")).toBeInTheDocument();
    const cta = screen.getByLabelText("Call to action") as HTMLInputElement;

    await user.type(headline, "Subscribe now");
    await user.type(cta, "Join");
    await user.type(screen.getByLabelText(/target selector/i), ".article");

    // Live preview of the rendered result.
    expect(
      await screen.findByTestId("component-ref-preview-wrap"),
    ).toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: /save/i }));

    expect(onValidSubmit).toHaveBeenCalledTimes(1);
    expect(onValidSubmit).toHaveBeenCalledWith({
      type: "component_ref",
      component_id: COMPONENT_ID,
      version: "default",
      variables: { headline: "Subscribe now", cta: "Join" },
      target_selector: ".article",
      placement_mode: "append",
    });
  });

  it("stores a pinned numeric version (round-trips through the version select)", async () => {
    mockComponentEndpoints();
    const onValidSubmit = vi.fn();
    const user = userEvent.setup();

    renderWithQuery(<Harness type="component_ref" onValidSubmit={onValidSubmit} />);

    await screen.findByRole("option", { name: "Paywall CTA" });
    await user.selectOptions(screen.getByLabelText("Component"), COMPONENT_ID);

    const versionSelect = (await screen.findByLabelText(
      /version/i,
    )) as HTMLSelectElement;
    await waitFor(() =>
      expect(Array.from(versionSelect.options).map((o) => o.value)).toEqual([
        "default",
        "2",
        "1",
      ]),
    );

    await user.selectOptions(versionSelect, "1");
    expect(versionSelect.value).toBe("1");

    await user.type(screen.getByLabelText(/target selector/i), ".article");
    await user.click(screen.getByRole("button", { name: /save/i }));

    expect(onValidSubmit).toHaveBeenCalledTimes(1);
    const submitted = onValidSubmit.mock.calls[0][0];
    expect(submitted.version).toBe(1);
  });

  it("uses target_path (not a selector) for a component_ref_json config", async () => {
    mockComponentEndpoints();
    const onValidSubmit = vi.fn();
    const user = userEvent.setup();

    renderWithQuery(
      <Harness type="component_ref_json" onValidSubmit={onValidSubmit} />,
    );

    await screen.findByRole("option", { name: "Paywall CTA" });
    await user.selectOptions(screen.getByLabelText("Component"), COMPONENT_ID);

    // JSON variant exposes a target path, not a CSS selector.
    expect(screen.queryByLabelText(/target selector/i)).not.toBeInTheDocument();
    await user.type(screen.getByLabelText(/target path/i), "$.content.html");

    await user.click(screen.getByRole("button", { name: /save/i }));

    expect(onValidSubmit).toHaveBeenCalledTimes(1);
    expect(onValidSubmit).toHaveBeenCalledWith({
      type: "component_ref_json",
      component_id: COMPONENT_ID,
      version: "default",
      variables: {},
      target_path: "$.content.html",
    });
  });

  it("blocks submit and shows an error when no component is picked", async () => {
    mockComponentEndpoints();
    const onValidSubmit = vi.fn();
    const user = userEvent.setup();

    renderWithQuery(<Harness type="component_ref" onValidSubmit={onValidSubmit} />);

    await screen.findByRole("option", { name: "Paywall CTA" });
    await user.click(screen.getByRole("button", { name: /save/i }));

    expect(onValidSubmit).not.toHaveBeenCalled();
    expect(screen.getByText("Pick a component")).toBeInTheDocument();
  });
});
