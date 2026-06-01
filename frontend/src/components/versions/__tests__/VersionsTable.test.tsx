import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { render, screen } from "@testing-library/react";
import type { ReactNode } from "react";
import { describe, expect, it } from "vitest";

import { VersionsTable } from "@/components/versions/VersionsTable";
import type { VersionSummary } from "@/lib/api/versions";

function wrapper({ children }: { children: ReactNode }) {
  const client = new QueryClient({
    defaultOptions: { queries: { retry: false } },
  });
  return (
    <QueryClientProvider client={client}>{children}</QueryClientProvider>
  );
}

function makeVersion(overrides: Partial<VersionSummary>): VersionSummary {
  return {
    id: "11111111-1111-1111-1111-111111111111",
    feature_id: "dn-article",
    version_number: 1,
    description: "Initial rollout",
    status: "live",
    last_updated_by: "alice",
    last_updated_at: "2026-05-30T10:00:00Z",
    created_at: "2026-05-29T10:00:00Z",
    ...overrides,
  };
}

describe("VersionsTable", () => {
  it("renders a row per version with the correct status pill", () => {
    const versions: VersionSummary[] = [
      makeVersion({
        id: "a1",
        version_number: 3,
        status: "draft",
        description: "WIP",
      }),
      makeVersion({
        id: "a2",
        version_number: 2,
        status: "staging",
        description: "Testing",
      }),
      makeVersion({ id: "a3", version_number: 1, status: "live" }),
    ];

    render(
      <VersionsTable
        featureId="dn-article"
        featureType="html"
        versions={versions}
      />,
      { wrapper },
    );

    expect(screen.getByText("V3")).toBeInTheDocument();
    expect(screen.getByText("V2")).toBeInTheDocument();
    expect(screen.getByText("V1")).toBeInTheDocument();

    expect(screen.getByText("DRAFT")).toBeInTheDocument();
    expect(screen.getByText("STAGING")).toBeInTheDocument();
    expect(screen.getByText("LIVE")).toBeInTheDocument();
  });

  it("links the version number to the version detail route", () => {
    render(
      <VersionsTable
        featureId="dn-article"
        featureType="html"
        versions={[makeVersion({ version_number: 7 })]}
      />,
      { wrapper },
    );

    const link = screen.getByRole("link", { name: "V7" });
    expect(link).toHaveAttribute(
      "href",
      "/products/features/html/dn-article/7",
    );
  });

  it("shows an empty state when there are no versions", () => {
    render(
      <VersionsTable featureId="dn-article" featureType="html" versions={[]} />,
      { wrapper },
    );
    expect(
      screen.getByText(/no versions match your search/i),
    ).toBeInTheDocument();
  });
});
