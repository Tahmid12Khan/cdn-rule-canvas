import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it } from "vitest";

import { NodeConfigDrawer } from "@/components/canvas/config/NodeConfigDrawer";
import { DEFAULT_META_TAGS } from "@/lib/canvas/nodeTemplates";
import { useRuleBuilderStore } from "@/state/ruleBuilderStore";
import type { RFNode } from "@/lib/canvas/types";
import type { RuleGraph } from "@/lib/api/ruleGraph";

const emptyGraph: RuleGraph = {
  anonymous: { nodes: [], edges: [], root_node_id: null },
  registered: { nodes: [], edges: [], root_node_id: null },
  customer: { nodes: [], edges: [], root_node_id: null },
};

const decisionNode: RFNode = {
  id: "d1",
  type: "decisionNode",
  position: { x: 0, y: 0 },
  data: { processor: { ...DEFAULT_META_TAGS } }, // tag_name: "" -> invalid
};

beforeEach(() => {
  const s = useRuleBuilderStore.getState();
  s.seedFromRuleGraph(emptyGraph, "draft", () => "X");
  s.addNode("anonymous", decisionNode);
  s.openNodeConfig("d1");
});

describe("NodeConfigDrawer (edit mode)", () => {
  beforeEach(() => {
    // seedFromRuleGraph resets isEditing to false; enter edit mode for the
    // editable-behavior tests.
    useRuleBuilderStore.getState().toggleEdit();
  });

  it("renders the Meta Tags form for a meta_tags node", () => {
    render(<NodeConfigDrawer canvasKey="anonymous" />);
    expect(screen.getByLabelText("Tag name")).toBeInTheDocument();
    expect(screen.getByLabelText("Operator")).toBeInTheDocument();
  });

  it("disables Save until the form is valid, then persists on Save", async () => {
    const user = userEvent.setup();
    render(<NodeConfigDrawer canvasKey="anonymous" />);
    const saveBtn = screen.getByRole("button", { name: "Save" });
    expect(saveBtn).toBeDisabled();

    await user.type(screen.getByLabelText("Tag name"), "paywall");
    await user.type(screen.getByLabelText("Value"), "true");
    expect(saveBtn).toBeEnabled();

    await user.click(saveBtn);
    const node = useRuleBuilderStore
      .getState()
      .canvases.anonymous.nodes.find((n) => n.id === "d1");
    expect(
      node && "processor" in node.data && node.data.processor.type === "meta_tags"
        ? node.data.processor.tag_name
        : undefined,
    ).toBe("paywall");
    // Drawer closes (configNodeId cleared).
    expect(useRuleBuilderStore.getState().configNodeId).toBeNull();
  });

  it("Delete node removes the node and its edges from the store", async () => {
    const user = userEvent.setup();
    render(<NodeConfigDrawer canvasKey="anonymous" />);
    await user.click(screen.getByRole("button", { name: "Delete node" }));
    // d1 is gone; only the frontend-only start node remains.
    expect(
      useRuleBuilderStore
        .getState()
        .canvases.anonymous.nodes.filter((n) => n.type !== "startNode"),
    ).toHaveLength(0);
  });
});

describe("NodeConfigDrawer (view-only mode)", () => {
  // seedFromRuleGraph leaves isEditing=false, so the drawer is read-only.
  it("shows a Close button and no Save / Delete for a decision node", () => {
    render(<NodeConfigDrawer canvasKey="anonymous" />);
    expect(screen.getByRole("button", { name: "Close" })).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Save" })).toBeNull();
    expect(
      screen.queryByRole("button", { name: "Delete node" }),
    ).toBeNull();
  });

  it("renders the decision form disabled (inspect, not edit)", () => {
    render(<NodeConfigDrawer canvasKey="anonymous" />);
    expect(screen.getByLabelText("Tag name")).toBeDisabled();
    expect(screen.getByLabelText("Operator")).toBeDisabled();
  });

  it("surfaces an outcome node's contents when inspected", () => {
    const s = useRuleBuilderStore.getState();
    s.addNode("anonymous", {
      id: "o1",
      type: "outcomeNode",
      position: { x: 0, y: 0 },
      data: {
        outcomeId: "11111111-1111-1111-1111-111111111111",
        title: "Show Paywall",
      },
    });
    s.openNodeConfig("o1");
    render(<NodeConfigDrawer canvasKey="anonymous" />);
    expect(screen.getByTestId("outcome-inspect")).toBeInTheDocument();
    expect(screen.getByText("Show Paywall")).toBeInTheDocument();
  });

  it("surfaces the start node's contents when inspected", () => {
    useRuleBuilderStore.getState().openNodeConfig("start");
    render(<NodeConfigDrawer canvasKey="anonymous" />);
    expect(screen.getByTestId("start-inspect")).toBeInTheDocument();
  });
});
