import { screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it } from "vitest";

import { NodeConfigDrawer } from "@/components/canvas/config/NodeConfigDrawer";
import { renderWithQuery } from "@/test/renderWithQuery";
import { META_TAGS_DEFAULT } from "@/test/fixtures/nodeTypes";
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
  data: { processor: { ...META_TAGS_DEFAULT } }, // tag_name: "" -> invalid
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

  it("renders the manifest-driven Meta Tags form for a meta_tags node", async () => {
    renderWithQuery(<NodeConfigDrawer canvasKey="anonymous" />);
    expect(await screen.findByLabelText(/Tag name/)).toBeInTheDocument();
    expect(screen.getByLabelText(/Operator/)).toBeInTheDocument();
  });

  it("disables Save until the form is valid, then persists on Save", async () => {
    const user = userEvent.setup();
    renderWithQuery(<NodeConfigDrawer canvasKey="anonymous" />);
    const saveBtn = screen.getByRole("button", { name: "Save" });
    await screen.findByLabelText(/Tag name/);
    expect(saveBtn).toBeDisabled();

    await user.type(screen.getByLabelText(/Tag name/), "paywall");
    await user.type(screen.getByLabelText(/Value/), "true");
    expect(saveBtn).toBeEnabled();

    await user.click(saveBtn);
    const node = useRuleBuilderStore
      .getState()
      .canvases.anonymous.nodes.find((n) => n.id === "d1");
    expect(
      node && "processor" in node.data
        ? node.data.processor.tag_name
        : undefined,
    ).toBe("paywall");
    // Drawer closes (configNodeId cleared).
    expect(useRuleBuilderStore.getState().configNodeId).toBeNull();
  });

  it("Delete node removes the node and its edges from the store", async () => {
    const user = userEvent.setup();
    renderWithQuery(<NodeConfigDrawer canvasKey="anonymous" />);
    await user.click(screen.getByRole("button", { name: "Delete node" }));
    // d1 is gone. Deleting the last real node resets the canvas to the default
    // start → end state (spec §6), so only the start + end bookends remain.
    const nodes = useRuleBuilderStore.getState().canvases.anonymous.nodes;
    expect(
      nodes.filter((n) => n.type !== "startNode" && n.type !== "endNode"),
    ).toHaveLength(0);
    expect(nodes.find((n) => n.id === "d1")).toBeUndefined();
  });
});

describe("NodeConfigDrawer (view-only mode)", () => {
  // seedFromRuleGraph leaves isEditing=false, so the drawer is read-only.
  it("shows a Close button and no Save / Delete for a decision node", () => {
    renderWithQuery(<NodeConfigDrawer canvasKey="anonymous" />);
    expect(screen.getByRole("button", { name: "Close" })).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Save" })).toBeNull();
    expect(screen.queryByRole("button", { name: "Delete node" })).toBeNull();
  });

  it("renders the decision form disabled (inspect, not edit)", async () => {
    renderWithQuery(<NodeConfigDrawer canvasKey="anonymous" />);
    expect(await screen.findByLabelText(/Tag name/)).toBeDisabled();
    expect(screen.getByLabelText(/Operator/)).toBeDisabled();
  });

  it("renders the apply_outcome form (outcome dropdown) for an expression node", async () => {
    const s = useRuleBuilderStore.getState();
    s.addNode("anonymous", {
      id: "a1",
      type: "expressionNode",
      position: { x: 0, y: 0 },
      data: {
        action: { type: "apply_outcome", outcome_id: "" },
      },
    });
    s.openNodeConfig("a1");
    renderWithQuery(
      <NodeConfigDrawer
        canvasKey="anonymous"
        outcomes={[
          { id: "11111111-1111-1111-1111-111111111111", title: "Show Paywall" },
        ]}
      />,
    );
    // The outcome_select control renders the supplied outcome as an option.
    const select = (await screen.findByLabelText(/Outcome/)) as HTMLSelectElement;
    expect(
      Array.from(select.options).map((o) => o.textContent),
    ).toContain("Show Paywall");
  });

  it("surfaces the start node's contents when inspected", () => {
    // start node only exists once a real node is added.
    useRuleBuilderStore.getState().openNodeConfig("start");
    renderWithQuery(<NodeConfigDrawer canvasKey="anonymous" />);
    expect(screen.getByTestId("start-inspect")).toBeInTheDocument();
  });

  it("surfaces the end node's contents when inspected", () => {
    useRuleBuilderStore.getState().openNodeConfig("end");
    renderWithQuery(<NodeConfigDrawer canvasKey="anonymous" />);
    expect(screen.getByTestId("end-inspect")).toBeInTheDocument();
  });
});

describe("NodeConfigDrawer required_unless", () => {
  beforeEach(() => {
    useRuleBuilderStore.getState().toggleEdit();
  });

  it("allows save with empty value once operator is 'exists' (required_unless)", async () => {
    const user = userEvent.setup();
    renderWithQuery(<NodeConfigDrawer canvasKey="anonymous" />);
    await user.type(await screen.findByLabelText(/Tag name/), "paywall");
    // value is still empty -> invalid until operator becomes "exists".
    const saveBtn = screen.getByRole("button", { name: "Save" });
    await waitFor(() => expect(saveBtn).toBeDisabled());
    await user.selectOptions(screen.getByLabelText(/Operator/), "exists");
    await waitFor(() => expect(saveBtn).toBeEnabled());
  });
});
