import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";

import type { DraftComponent } from "@/components/outcome/ComponentRow";
import { SortableComponentList } from "@/components/outcome/SortableComponentList";

function makeComponent(over: Partial<DraftComponent>): DraftComponent {
  return {
    id: "11111111-1111-1111-1111-111111111111",
    outcome_id: "22222222-2222-2222-2222-222222222222",
    slug: "banner",
    type: "html_injection",
    config: {
      type: "html_injection",
      target_selector: ".body",
      placement_mode: "append",
      html_body: "<p>hi</p>",
    },
    placement: "inline",
    order_index: 0,
    created_at: "2024-01-01T00:00:00Z",
    updated_at: "2024-01-01T00:00:00Z",
    ...over,
  };
}

describe("SortableComponentList", () => {
  it("renders an empty hint when there are no components", () => {
    render(
      <SortableComponentList
        components={[]}
        onReorder={vi.fn()}
        onEdit={vi.fn()}
        onDelete={vi.fn()}
      />,
    );
    expect(screen.getByText(/no components yet/i)).toBeInTheDocument();
  });

  it("renders a row per component with derived badges and a drag handle", () => {
    render(
      <SortableComponentList
        components={[
          makeComponent({ id: "a", slug: "first" }),
          makeComponent({ id: "b", slug: "second", placement: "sticky_footer" }),
        ]}
        onReorder={vi.fn()}
        onEdit={vi.fn()}
        onDelete={vi.fn()}
      />,
    );

    expect(screen.getByText("first")).toBeInTheDocument();
    expect(screen.getByText("second")).toBeInTheDocument();
    // Drag handles are real, keyboard-operable buttons (a11y §7).
    expect(
      screen.getByRole("button", { name: /reorder first/i }),
    ).toBeInTheDocument();
    // type badge for both, plus a placement badge for the sticky-footer row.
    expect(screen.getAllByText("HTML Injection")).toHaveLength(2);
    expect(screen.getByText("Sticky Footer")).toBeInTheDocument();
  });

  it("fires edit and delete callbacks for a row", async () => {
    const user = userEvent.setup();
    const onEdit = vi.fn();
    const onDelete = vi.fn();
    render(
      <SortableComponentList
        components={[makeComponent({ id: "a", slug: "first" })]}
        onReorder={vi.fn()}
        onEdit={onEdit}
        onDelete={onDelete}
      />,
    );

    await user.click(screen.getByRole("button", { name: /edit first/i }));
    expect(onEdit).toHaveBeenCalledWith("a");

    await user.click(screen.getByRole("button", { name: /delete first/i }));
    expect(onDelete).toHaveBeenCalledWith("a");
  });
});
