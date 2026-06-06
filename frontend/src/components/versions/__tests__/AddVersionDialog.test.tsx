import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { http, HttpResponse } from "msw";
import type { ReactNode } from "react";
import { describe, expect, it, vi } from "vitest";

import { AddVersionDialog } from "@/components/versions/AddVersionDialog";
import { API_BASE } from "@/lib/api/client";
import { server } from "@/test/mocks/server";

function makeWrapper() {
  const client = new QueryClient({
    defaultOptions: { queries: { retry: false } },
  });
  function Wrapper({ children }: { children: ReactNode }) {
    return (
      <QueryClientProvider client={client}>{children}</QueryClientProvider>
    );
  }
  return { client, Wrapper };
}

const versionResponse = {
  id: "11111111-1111-1111-1111-111111111111",
  feature_id: "demo-article",
  version_number: 4,
  description: "New rules",
  status: "draft",
  rule_graph: {
    anonymous: { nodes: [], edges: [], root_node_id: null },
    registered: { nodes: [], edges: [], root_node_id: null },
    customer: { nodes: [], edges: [], root_node_id: null },
  },
  created_by: "alice",
  last_updated_by: "alice",
  last_updated_at: "2026-05-31T10:00:00Z",
  created_at: "2026-05-31T10:00:00Z",
};

describe("AddVersionDialog", () => {
  it("submits the description, closes, and invalidates the versions query", async () => {
    const user = userEvent.setup();
    let receivedBody: unknown = null;

    server.use(
      http.post(
        `${API_BASE}/api/v1/features/demo-article/versions`,
        async ({ request }) => {
          receivedBody = await request.json();
          return HttpResponse.json(versionResponse, { status: 201 });
        },
      ),
    );

    const { client, Wrapper } = makeWrapper();
    const invalidateSpy = vi.spyOn(client, "invalidateQueries");

    render(<AddVersionDialog featureId="demo-article" />, { wrapper: Wrapper });

    await user.click(
      screen.getByRole("button", { name: /add a new version/i }),
    );

    const textarea = await screen.findByLabelText(/description/i);
    await user.type(textarea, "New rules");
    await user.click(screen.getByRole("button", { name: /create version/i }));

    await waitFor(() =>
      expect(receivedBody).toEqual({ description: "New rules" }),
    );

    await waitFor(() =>
      expect(invalidateSpy).toHaveBeenCalledWith({
        queryKey: ["versions", "demo-article"],
      }),
    );

    await waitFor(() =>
      expect(
        screen.queryByRole("button", { name: /create version/i }),
      ).not.toBeInTheDocument(),
    );
  });

  it("surfaces a server error and keeps the dialog open", async () => {
    const user = userEvent.setup();
    server.use(
      http.post(`${API_BASE}/api/v1/features/demo-article/versions`, () =>
        HttpResponse.json(
          { error: { code: "INTERNAL_ERROR", message: "boom" } },
          { status: 500 },
        ),
      ),
    );

    const { Wrapper } = makeWrapper();
    render(<AddVersionDialog featureId="demo-article" />, { wrapper: Wrapper });

    await user.click(
      screen.getByRole("button", { name: /add a new version/i }),
    );
    await user.click(screen.getByRole("button", { name: /create version/i }));

    expect(await screen.findByRole("alert")).toHaveTextContent(
      /something went wrong on the server/i,
    );
    expect(
      screen.getByRole("button", { name: /create version/i }),
    ).toBeInTheDocument();
  });
});
