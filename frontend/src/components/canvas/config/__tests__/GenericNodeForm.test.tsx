import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";

import { GenericNodeForm } from "@/components/canvas/config/GenericNodeForm";
import { NODE_TYPES_FIXTURE } from "@/test/fixtures/nodeTypes";
import { renderWithQuery } from "@/test/renderWithQuery";
import { defaultProcessor } from "@/lib/canvas/manifest";

const META = NODE_TYPES_FIXTURE.node_types.find((s) => s.kind === "meta_tags")!;
const DEVICE = NODE_TYPES_FIXTURE.node_types.find(
  (s) => s.kind === "device_type",
)!;
const SITE_MATCH = NODE_TYPES_FIXTURE.node_types.find(
  (s) => s.kind === "site_match",
)!;

describe("GenericNodeForm", () => {
  it("renders a control per field in spec order (select + text)", () => {
    render(
      <GenericNodeForm
        spec={META}
        initial={defaultProcessor(META)}
        onChange={() => undefined}
      />,
    );
    expect(screen.getByLabelText(/Tag name/)).toBeInTheDocument();
    const operator = screen.getByLabelText(/Operator/) as HTMLSelectElement;
    expect(operator.tagName).toBe("SELECT");
    expect(screen.getByLabelText(/Value/)).toBeInTheDocument();
  });

  it("only exposes manifest options on a select control", () => {
    render(
      <GenericNodeForm
        spec={DEVICE}
        initial={defaultProcessor(DEVICE)}
        onChange={() => undefined}
      />,
    );
    const valueSelect = screen.getByLabelText(/Value/) as HTMLSelectElement;
    expect(Array.from(valueSelect.options).map((o) => o.value)).toEqual([
      "mobile",
      "desktop",
      "tablet",
    ]);
  });

  it("reports invalid while a required field is empty, valid once filled", async () => {
    const onChange = vi.fn();
    const user = userEvent.setup();
    render(
      <GenericNodeForm
        spec={META}
        initial={defaultProcessor(META)}
        onChange={onChange}
      />,
    );
    // Initial: tag_name + value empty -> invalid.
    expect(onChange.mock.calls.at(-1)?.[1]).toBe(false);

    await user.type(screen.getByLabelText(/Tag name/), "paywall");
    await user.type(screen.getByLabelText(/Value/), "true");
    expect(onChange.mock.calls.at(-1)?.[1]).toBe(true);
    expect(onChange.mock.calls.at(-1)?.[0]).toMatchObject({
      type: "meta_tags",
      tag_name: "paywall",
      operator: "contains",
      value: "true",
    });
  });

  it("required_unless: value optional once operator is 'exists'", async () => {
    const onChange = vi.fn();
    const user = userEvent.setup();
    render(
      <GenericNodeForm
        spec={META}
        initial={{
          type: "meta_tags",
          tag_name: "paywall",
          operator: "contains",
          value: "",
        }}
        onChange={onChange}
      />,
    );
    // value empty + operator contains -> invalid.
    expect(onChange.mock.calls.at(-1)?.[1]).toBe(false);
    await user.selectOptions(screen.getByLabelText(/Operator/), "exists");
    expect(onChange.mock.calls.at(-1)?.[1]).toBe(true);
  });

  it("renders a site_select field as a searchable combobox (manifest-driven)", () => {
    // site_match is wholly manifest-driven: a single `site` field with the
    // `site_select` control. GenericNodeForm must render the combobox with no
    // per-node-type code. Uses renderWithQuery since the control fetches sites.
    renderWithQuery(
      <GenericNodeForm
        spec={SITE_MATCH}
        initial={defaultProcessor(SITE_MATCH)}
        onChange={() => undefined}
      />,
    );
    const combobox = screen.getByRole("combobox");
    expect(combobox).toHaveAttribute("placeholder", "Search sites by name…");
    // Required + empty -> the node is intentionally incomplete.
    expect(screen.getByText("Site")).toBeInTheDocument();
  });

  it("site_select reports invalid when the required slug is empty", () => {
    const onChange = vi.fn();
    renderWithQuery(
      <GenericNodeForm
        spec={SITE_MATCH}
        initial={defaultProcessor(SITE_MATCH)}
        onChange={onChange}
      />,
    );
    expect(onChange.mock.calls.at(-1)?.[1]).toBe(false);
  });

  it("site_select reports valid once a slug is stored", () => {
    const onChange = vi.fn();
    renderWithQuery(
      <GenericNodeForm
        spec={SITE_MATCH}
        initial={{ type: "site_match", site: "demo-localhost" }}
        onChange={onChange}
      />,
    );
    expect(onChange.mock.calls.at(-1)?.[1]).toBe(true);
    expect(onChange.mock.calls.at(-1)?.[0]).toMatchObject({
      type: "site_match",
      site: "demo-localhost",
    });
  });

  it("shows the field's required_message inline when empty", () => {
    render(
      <GenericNodeForm
        spec={META}
        initial={defaultProcessor(META)}
        onChange={() => undefined}
      />,
    );
    expect(
      screen.getByText("Enter the meta tag name to match (e.g. og:type)"),
    ).toBeInTheDocument();
  });
});
