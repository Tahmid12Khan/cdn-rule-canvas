import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";

import { ComponentConfigModal } from "@/components/component-config/ComponentConfigModal";

describe("ComponentConfigModal", () => {
  it("renders slug, placement, and the HTML Injection form by default", () => {
    render(
      <ComponentConfigModal open onOpenChange={() => {}} onSubmit={() => {}} />,
    );
    expect(screen.getByRole("dialog")).toBeInTheDocument();
    expect(screen.getByLabelText(/slug/i)).toBeInTheDocument();
    expect(screen.getByLabelText(/^placement$/i)).toBeInTheDocument();
    expect(
      screen.getByRole("tab", { name: "HTML Injection", selected: true }),
    ).toBeInTheDocument();
    // HTML Injection form field present.
    expect(screen.getByLabelText(/target selector/i)).toBeInTheDocument();
  });

  it("switches the active form when selecting the Content Truncation tab", async () => {
    const user = userEvent.setup();
    render(
      <ComponentConfigModal open onOpenChange={() => {}} onSubmit={() => {}} />,
    );
    await user.click(screen.getByRole("tab", { name: "Content Truncation" }));
    expect(
      screen.getByRole("tab", { name: "Content Truncation", selected: true }),
    ).toBeInTheDocument();
    // Word count is unique to the content truncation form.
    expect(screen.getByLabelText(/word count/i)).toBeInTheDocument();
  });

  it("submits an assembled create payload when fields are valid", async () => {
    const user = userEvent.setup();
    const onSubmit = vi.fn();
    render(
      <ComponentConfigModal open onOpenChange={() => {}} onSubmit={onSubmit} />,
    );
    await user.type(screen.getByLabelText(/slug/i), "paywall-banner");
    await user.type(screen.getByLabelText(/target selector/i), ".article");
    const textareas = screen.getAllByRole("textbox");
    // last textbox is the HTML body (Visual tab textarea); first is slug,
    // second is target selector.
    await user.type(textareas[textareas.length - 1], "<p>x</p>");
    await user.click(screen.getByRole("button", { name: /save component/i }));

    expect(onSubmit).toHaveBeenCalledTimes(1);
    expect(onSubmit).toHaveBeenCalledWith({
      slug: "paywall-banner",
      type: "html_injection",
      placement: "inline",
      config: {
        type: "html_injection",
        target_selector: ".article",
        placement_mode: "append",
        html_body: "<p>x</p>",
        theme: null,
      },
    });
  });

  it("offers a Component tab for HTML features", () => {
    render(
      <ComponentConfigModal
        open
        featureType="html"
        onOpenChange={() => {}}
        onSubmit={() => {}}
      />,
    );
    expect(
      screen.getByRole("tab", { name: "HTML Injection" }),
    ).toBeInTheDocument();
    expect(screen.getByRole("tab", { name: "Component" })).toBeInTheDocument();
  });

  it("offers a Component tab for JSON features", () => {
    render(
      <ComponentConfigModal
        open
        featureType="json"
        onOpenChange={() => {}}
        onSubmit={() => {}}
      />,
    );
    expect(screen.getByRole("tab", { name: "JSON Remove" })).toBeInTheDocument();
    expect(screen.getByRole("tab", { name: "Component" })).toBeInTheDocument();
  });

  it("locks the type tabs in edit mode", () => {
    render(
      <ComponentConfigModal
        open
        mode="edit"
        initialType="content_truncation"
        initialSlug="existing"
        initialConfig={{
          type: "content_truncation",
          target_selector: ".body",
          word_count: 80,
          fade_out: true,
        }}
        onOpenChange={() => {}}
        onSubmit={() => {}}
      />,
    );
    const htmlTab = screen.getByRole("tab", { name: "HTML Injection" });
    expect(htmlTab).toBeDisabled();
    expect(
      screen.getByRole("tab", { name: "Content Truncation", selected: true }),
    ).toBeInTheDocument();
    expect(screen.getByLabelText(/word count/i)).toHaveValue(80);
  });
});
