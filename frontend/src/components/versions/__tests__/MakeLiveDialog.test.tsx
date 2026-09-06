import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { http, HttpResponse } from "msw";
import type { ReactNode } from "react";
import { describe, expect, it, vi } from "vitest";

import { MakeLiveDialog } from "@/components/versions/MakeLiveDialog";
import { API_BASE } from "@/lib/api/client";
import { server } from "@/test/mocks/server";

function makeWrapper() {
  const client = new QueryClient({
    defaultOptions: { queries: { retry: false }, mutations: { retry: false } },
  });
  function Wrapper({ children }: { children: ReactNode }) {
    return (
      <QueryClientProvider client={client}>{children}</QueryClientProvider>
    );
  }
  return { client, Wrapper };
}

const PUBLISH_URL = `${API_BASE}/api/v1/features/demo-article/versions/2/publish`;

const publishedResponse = {
  id: "22222222-2222-2222-2222-222222222222",
  feature_id: "demo-article",
  version_number: 2,
  description: null,
  status: "live",
  rule_graph: {
    canvas: { nodes: [], edges: [], root_node_id: null },
  },
  created_by: "alice",
  last_updated_by: "alice",
  last_updated_at: "2026-05-31T10:00:00Z",
  created_at: "2026-05-31T10:00:00Z",
};

describe("MakeLiveDialog", () => {
  it("publishes to live on confirm and invalidates version caches", async () => {
    const user = userEvent.setup();
    let body: unknown = null;
    server.use(
      http.post(PUBLISH_URL, async ({ request }) => {
        body = await request.json();
        return HttpResponse.json(publishedResponse);
      }),
    );

    const { client, Wrapper } = makeWrapper();
    const invalidateSpy = vi.spyOn(client, "invalidateQueries");
    const onOpenChange = vi.fn();

    render(
      <MakeLiveDialog
        featureId="demo-article"
        versionNumber={2}
        open
        onOpenChange={onOpenChange}
      />,
      { wrapper: Wrapper },
    );

    await user.click(screen.getByRole("button", { name: /^make live$/i }));

    await waitFor(() => expect(body).toEqual({ environment: "live" }));
    await waitFor(() =>
      expect(invalidateSpy).toHaveBeenCalledWith({
        queryKey: ["feature", "demo-article"],
      }),
    );
    await waitFor(() => expect(onOpenChange).toHaveBeenCalledWith(false));
  });

  it("does not fire a second publish on a rapid double-click", async () => {
    const user = userEvent.setup();
    let calls = 0;
    let release!: () => void;
    const gate = new Promise<void>((r) => {
      release = r;
    });
    server.use(
      http.post(PUBLISH_URL, async () => {
        calls += 1;
        // Hold the response so isPending stays true across the second click,
        // exercising the re-entry guard.
        await gate;
        return HttpResponse.json(publishedResponse);
      }),
    );

    const { Wrapper } = makeWrapper();
    const onOpenChange = vi.fn();
    render(
      <MakeLiveDialog
        featureId="demo-article"
        versionNumber={2}
        open
        onOpenChange={onOpenChange}
      />,
      { wrapper: Wrapper },
    );

    const button = screen.getByRole("button", { name: /^make live$/i });
    // Two rapid clicks before the first publish resolves — the re-entry guard
    // + disabled state must prevent a duplicate publish.
    await user.click(button);
    await user.click(button);

    expect(calls).toBe(1);

    // Let the held request resolve and the dialog close cleanly.
    release();
    await waitFor(() => expect(onOpenChange).toHaveBeenCalledWith(false));
    expect(calls).toBe(1);
  });

  it("surfaces an 'already live' UserError on a 409 and stays open", async () => {
    const user = userEvent.setup();
    server.use(
      http.post(PUBLISH_URL, () =>
        HttpResponse.json(
          {
            error: {
              code: "INVALID_STATUS_TRANSITION",
              message: "already live",
            },
          },
          { status: 409 },
        ),
      ),
    );

    const { Wrapper } = makeWrapper();
    render(
      <MakeLiveDialog
        featureId="demo-article"
        versionNumber={2}
        open
        onOpenChange={() => {}}
      />,
      { wrapper: Wrapper },
    );

    await user.click(screen.getByRole("button", { name: /^make live$/i }));

    expect(await screen.findByRole("alert")).toHaveTextContent(
      /already live/i,
    );
  });
});
