import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { render, screen, waitFor } from "@testing-library/react";
import { http, HttpResponse } from "msw";
import type { ReactNode } from "react";
import { describe, expect, it } from "vitest";

import { BackendStatusCard } from "@/components/BackendStatusCard";
import { API_BASE } from "@/lib/api/client";
import { server } from "@/test/mocks/server";

function wrapper({ children }: { children: ReactNode }) {
  const client = new QueryClient({
    defaultOptions: { queries: { retry: false } },
  });
  return (
    <QueryClientProvider client={client}>{children}</QueryClientProvider>
  );
}

describe("BackendStatusCard", () => {
  it("shows connected with version on success", async () => {
    render(<BackendStatusCard />, { wrapper });
    await waitFor(() =>
      expect(screen.getByText(/connected · v0\.1\.0/i)).toBeInTheDocument(),
    );
  });

  it("shows unreachable on backend error", async () => {
    server.use(
      http.get(`${API_BASE}/health`, () =>
        HttpResponse.json({ error: { code: "X", message: "boom" } }, {
          status: 500,
        }),
      ),
    );
    render(<BackendStatusCard />, { wrapper });
    await waitFor(() =>
      expect(screen.getByText(/backend unreachable/i)).toBeInTheDocument(),
    );
  });
});
