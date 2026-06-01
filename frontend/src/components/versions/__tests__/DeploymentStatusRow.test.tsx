import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import { DeploymentStatusRow } from "@/components/versions/DeploymentStatusRow";

describe("DeploymentStatusRow", () => {
  it("resolves staging/live UUIDs to version numbers", () => {
    const numbers: Record<string, number> = {
      "staging-id": 2,
      "live-id": 1,
    };
    render(
      <DeploymentStatusRow
        stagingVersionId="staging-id"
        liveVersionId="live-id"
        versionNumberById={(id) => numbers[id]}
      />,
    );

    expect(screen.getByText(/STAGING V2/)).toBeInTheDocument();
    expect(screen.getByText(/LIVE V1/)).toBeInTheDocument();
  });

  it("renders nothing when neither deployment is set", () => {
    const { container } = render(
      <DeploymentStatusRow
        stagingVersionId={null}
        liveVersionId={null}
        versionNumberById={() => undefined}
      />,
    );
    expect(container).toBeEmptyDOMElement();
  });

  it("shows a dash when the deployment version is not on the current page", () => {
    render(
      <DeploymentStatusRow
        stagingVersionId={null}
        liveVersionId="live-id"
        versionNumberById={() => undefined}
      />,
    );
    expect(screen.getByText(/LIVE —/)).toBeInTheDocument();
  });
});
