import { screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { http, HttpResponse } from "msw";
import { describe, expect, it, vi } from "vitest";

import { GenericNodeForm } from "@/components/canvas/config/GenericNodeForm";
import { NODE_TYPES_FIXTURE } from "@/test/fixtures/nodeTypes";
import { server } from "@/test/mocks/server";
import { renderWithQuery } from "@/test/renderWithQuery";
import { defaultProcessor } from "@/lib/canvas/manifest";
import type { ProcessorConfig } from "@/lib/canvas/types";

const API_BASE = process.env.NEXT_PUBLIC_API_BASE ?? "http://localhost:8000";

const APPLY_COMPONENT = NODE_TYPES_FIXTURE.node_types.find(
  (s) => s.kind === "apply_component",
)!;

const COMPONENT_ID = "11111111-1111-1111-1111-111111111111";

// Mock the three component endpoints the form depends on: list (component
// dropdown), detail (version dropdown), resolve (declared variables + preview
// html). Resolve is keyed off the ?version selector so switching versions can
// return a different variable set if needed.
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
    http.get(
      `${API_BASE}/api/v1/component-templates/${COMPONENT_ID}`,
      () =>
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

describe("GenericNodeForm — Component node Variables sub-form", () => {
  it("round-trips the variable value-map onto action.variables", async () => {
    mockComponentEndpoints();
    const onChange = vi.fn();
    const user = userEvent.setup();

    renderWithQuery(
      <GenericNodeForm
        spec={APPLY_COMPONENT}
        initial={defaultProcessor(APPLY_COMPONENT)}
        onChange={onChange}
      />,
    );

    // Pick the component from the dynamic dropdown (wait for the list to load).
    const componentSelect = screen.getByLabelText(/Component/) as HTMLSelectElement;
    await screen.findByRole("option", { name: "Paywall CTA" });
    await user.selectOptions(componentSelect, COMPONENT_ID);

    // Once resolved, the declared variables render labelled by their title, with
    // the description as helper text (design §5.4 item 7).
    const headline = (await screen.findByLabelText(
      "Headline",
    )) as HTMLInputElement;
    expect(screen.getByText("The big text")).toBeInTheDocument();
    const cta = screen.getByLabelText("Call to action") as HTMLInputElement;

    // Enter values; they are stored on action.variables (the value map).
    await user.type(headline, "Subscribe now");
    await user.type(cta, "Join");

    const last = onChange.mock.calls.at(-1)?.[0] as ProcessorConfig;
    expect(last.type).toBe("apply_component");
    expect(last.component_id).toBe(COMPONENT_ID);
    expect(last.version).toBe("default");
    expect(last.variables).toEqual({
      headline: "Subscribe now",
      cta: "Join",
    });

    // A live preview of the rendered result is shown.
    expect(screen.getByTestId("component-preview-wrap")).toBeInTheDocument();
  });

  it("lists each version of the chosen component plus the Default option", async () => {
    mockComponentEndpoints();
    const user = userEvent.setup();

    renderWithQuery(
      <GenericNodeForm
        spec={APPLY_COMPONENT}
        initial={defaultProcessor(APPLY_COMPONENT)}
        onChange={() => undefined}
      />,
    );

    await screen.findByRole("option", { name: "Paywall CTA" });
    await user.selectOptions(
      screen.getByLabelText(/Component/) as HTMLSelectElement,
      COMPONENT_ID,
    );

    const versionSelect = (await screen.findByLabelText(
      /Version/,
    )) as HTMLSelectElement;
    await waitFor(() =>
      expect(Array.from(versionSelect.options).map((o) => o.value)).toEqual([
        "default",
        "2",
        "1",
      ]),
    );

    // Selecting a numeric version is reflected in the select value.
    await user.selectOptions(versionSelect, "1");
    expect(versionSelect.value).toBe("1");
  });

  it("resets version back to default when the component changes", async () => {
    mockComponentEndpoints();
    const onChange = vi.fn();
    const user = userEvent.setup();

    renderWithQuery(
      <GenericNodeForm
        spec={APPLY_COMPONENT}
        initial={{
          type: "apply_component",
          component_id: COMPONENT_ID,
          version: 1,
          variables: {},
          target_selector: "main",
          placement_mode: "append",
        }}
        onChange={onChange}
      />,
    );

    const versionSelect = (await screen.findByLabelText(
      /Version/,
    )) as HTMLSelectElement;
    await waitFor(() => expect(versionSelect.value).toBe("1"));

    // Re-selecting the (same) component clears version back to "default".
    await user.selectOptions(
      screen.getByLabelText(/Component/) as HTMLSelectElement,
      COMPONENT_ID,
    );
    const last = onChange.mock.calls.at(-1)?.[0] as ProcessorConfig;
    expect(last.version).toBe("default");
  });
});
