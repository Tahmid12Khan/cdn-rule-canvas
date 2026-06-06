import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { http, HttpResponse } from "msw";
import type { ReactNode } from "react";
import { describe, expect, it, vi } from "vitest";

import { SiteDeleteDialog } from "@/components/sites/SiteDeleteDialog";
import { API_BASE } from "@/lib/api/client";
import type { SiteRead } from "@/lib/api/sites";
import { server } from "@/test/mocks/server";

const SITES_URL = `${API_BASE}/api/v1/sites`;

const site: SiteRead = {
  slug: "demo-localhost",
  name: "Demo (localhost:9000)",
  source_protocol: "http",
  source_host: "localhost",
  source_port: 9000,
  dest_protocol: "http",
  dest_host: "demo-upstream",
  dest_port: 8081,
  created_at: "2026-06-06T00:00:00Z",
  updated_at: "2026-06-06T00:00:00Z",
};

function makeWrapper() {
  const client = new QueryClient({
    defaultOptions: { queries: { retry: false }, mutations: { retry: false } },
  });
  const invalidateSpy = vi.spyOn(client, "invalidateQueries");
  function Wrapper({ children }: { children: ReactNode }) {
    return (
      <QueryClientProvider client={client}>{children}</QueryClientProvider>
    );
  }
  return { Wrapper, invalidateSpy };
}

describe("SiteDeleteDialog", () => {
  it("deletes the site: invalidates the sites query and closes", async () => {
    const user = userEvent.setup();
    server.use(
      http.delete(`${SITES_URL}/:slug`, () => new HttpResponse(null, { status: 204 })),
    );

    const onOpenChange = vi.fn();
    const { Wrapper, invalidateSpy } = makeWrapper();
    render(<SiteDeleteDialog site={site} open onOpenChange={onOpenChange} />, {
      wrapper: Wrapper,
    });

    await user.click(screen.getByRole("button", { name: /^delete$/i }));

    await waitFor(() =>
      expect(invalidateSpy).toHaveBeenCalledWith({ queryKey: ["sites"] }),
    );
    expect(onOpenChange).toHaveBeenCalledWith(false);
  });

  it("shows an error banner and stays open when delete fails", async () => {
    const user = userEvent.setup();
    server.use(
      http.delete(`${SITES_URL}/:slug`, () =>
        HttpResponse.json(
          { error: { code: "INTERNAL_ERROR", message: "boom" } },
          { status: 500 },
        ),
      ),
    );

    const onOpenChange = vi.fn();
    const { Wrapper } = makeWrapper();
    render(<SiteDeleteDialog site={site} open onOpenChange={onOpenChange} />, {
      wrapper: Wrapper,
    });

    await user.click(screen.getByRole("button", { name: /^delete$/i }));

    await waitFor(() =>
      expect(screen.getByRole("alert")).toBeInTheDocument(),
    );
    // The dialog must NOT close on error.
    expect(onOpenChange).not.toHaveBeenCalledWith(false);
    expect(screen.getByRole("dialog")).toBeInTheDocument();
  });
});
