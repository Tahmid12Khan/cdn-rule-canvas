import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { http, HttpResponse } from "msw";
import type { ReactNode } from "react";
import { describe, expect, it, vi } from "vitest";

import { TestPresetDeleteDialog } from "@/components/test-presets/TestPresetDeleteDialog";
import { API_BASE } from "@/lib/api/client";
import type { TestPresetRead } from "@/lib/api/test-presets";
import { server } from "@/test/mocks/server";

const PRESETS_URL = `${API_BASE}/api/v1/test-presets`;

const preset: TestPresetRead = {
  slug: "mobile-paywall",
  name: "Mobile paywall",
  kind: "rule",
  payload: { feature_type: "html" },
  created_at: "2026-06-06T00:00:00Z",
  updated_at: "2026-06-06T00:00:00Z",
};

function makeWrapper() {
  const client = new QueryClient({
    defaultOptions: { queries: { retry: false }, mutations: { retry: false } },
  });
  const invalidateSpy = vi.spyOn(client, "invalidateQueries");
  function Wrapper({ children }: { children: ReactNode }) {
    return <QueryClientProvider client={client}>{children}</QueryClientProvider>;
  }
  return { Wrapper, invalidateSpy };
}

describe("TestPresetDeleteDialog", () => {
  it("deletes the preset: invalidates the query and closes", async () => {
    const user = userEvent.setup();
    server.use(
      http.delete(
        `${PRESETS_URL}/:slug`,
        () => new HttpResponse(null, { status: 204 }),
      ),
    );

    const onOpenChange = vi.fn();
    const { Wrapper, invalidateSpy } = makeWrapper();
    render(
      <TestPresetDeleteDialog preset={preset} open onOpenChange={onOpenChange} />,
      { wrapper: Wrapper },
    );

    await user.click(screen.getByRole("button", { name: /^delete$/i }));

    await waitFor(() =>
      expect(invalidateSpy).toHaveBeenCalledWith({
        queryKey: ["test-presets"],
      }),
    );
    expect(onOpenChange).toHaveBeenCalledWith(false);
  });

  it("shows an error banner and stays open when delete fails", async () => {
    const user = userEvent.setup();
    server.use(
      http.delete(`${PRESETS_URL}/:slug`, () =>
        HttpResponse.json(
          { error: { code: "INTERNAL_ERROR", message: "boom" } },
          { status: 500 },
        ),
      ),
    );

    const onOpenChange = vi.fn();
    const { Wrapper } = makeWrapper();
    render(
      <TestPresetDeleteDialog preset={preset} open onOpenChange={onOpenChange} />,
      { wrapper: Wrapper },
    );

    await user.click(screen.getByRole("button", { name: /^delete$/i }));

    await waitFor(() => expect(screen.getByRole("alert")).toBeInTheDocument());
    expect(onOpenChange).not.toHaveBeenCalledWith(false);
    expect(screen.getByRole("dialog")).toBeInTheDocument();
  });
});
