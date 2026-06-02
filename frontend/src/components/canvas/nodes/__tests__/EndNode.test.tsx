import { render, screen } from "@testing-library/react";
import { ReactFlowProvider } from "@xyflow/react";
import { describe, expect, it } from "vitest";

import { EndNode } from "@/components/canvas/nodes/EndNode";
import type { EndNodeData } from "@/lib/canvas/types";

function nodeProps(data: EndNodeData) {
  return {
    id: "end",
    data,
    selected: false,
    type: "endNode",
    isConnectable: true,
    xPos: 0,
    yPos: 0,
    dragging: false,
    zIndex: 0,
  } as unknown as Parameters<typeof EndNode>[0];
}

function wrapper({ children }: { children: React.ReactNode }) {
  return <ReactFlowProvider>{children}</ReactFlowProvider>;
}

describe("EndNode", () => {
  it("renders the terminal END pill", () => {
    render(<EndNode {...nodeProps({ label: "END" })} />, { wrapper });
    const node = screen.getByTestId("end-node");
    expect(node).toHaveTextContent("END");
  });

  it("falls back to 'END' when no label is given", () => {
    render(<EndNode {...nodeProps({ label: "" })} />, { wrapper });
    expect(screen.getByTestId("end-node")).toHaveTextContent("END");
  });
});
