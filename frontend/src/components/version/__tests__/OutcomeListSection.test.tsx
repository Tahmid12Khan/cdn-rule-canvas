import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { http, HttpResponse } from "msw";
import type { ReactNode } from "react";
import { describe, expect, it, vi } from "vitest";

import { OutcomeListSection } from "@/components/version/OutcomeListSection";
import { API_BASE } from "@/lib/api/client";
import { server } from "@/test/mocks/server";

vi.mock("next/link", () => ({
  default: ({ children, href }: { children: ReactNode; href: string }) => (
    <a href={href}>{children}</a>
  ),
}));

const VERSION_ID = "aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa";

function wrapper({ children }: { children: ReactNode }) {
  const client = new QueryClient({
    defaultOptions: { queries: { retry: false } },
  });
  return (
    <QueryClientProvider client={client}>{children}</QueryClientProvider>
  );
}

const OUTCOMES = [
  {
    id: "11111111-1111-1111-1111-111111111111",
    version_id: VERSION_ID,
    title: "Show Content",
    is_builtin: true,
    order_index: 0,
  },
  {
    id: "22222222-2222-2222-2222-222222222222",
    version_id: VERSION_ID,
    title: "Paywall",
    is_builtin: false,
    order_index: 1,
  },
];

describe("OutcomeListSection", () => {
  it("lists outcomes and hides clone/delete for builtin ones", async () => {
    server.use(
      http.get(`${API_BASE}/api/v1/versions/${VERSION_ID}/outcomes`, () =>
        HttpResponse.json(OUTCOMES),
      ),
    );
    render(
      <OutcomeListSection
        versionId={VERSION_ID}
        routeBase="/products/features/html/dn-article/1"
        editable
      />,
      { wrapper },
    );

    await waitFor(() =>
      expect(screen.getByText("Show Content")).toBeInTheDocument(),
    );
    expect(screen.getByText("Paywall")).toBeInTheDocument();
    // Builtin: only one Delete button (for the non-builtin Paywall).
    expect(screen.getAllByText("Delete")).toHaveLength(1);
    expect(screen.getByText("Built-in")).toBeInTheDocument();
  });

  it("labels the link 'View' and hides clone/delete when not editable", async () => {
    server.use(
      http.get(`${API_BASE}/api/v1/versions/${VERSION_ID}/outcomes`, () =>
        HttpResponse.json(OUTCOMES),
      ),
    );
    render(
      <OutcomeListSection
        versionId={VERSION_ID}
        routeBase="/products/features/html/dn-article/1"
        editable={false}
      />,
      { wrapper },
    );

    await waitFor(() =>
      expect(screen.getByText("Paywall")).toBeInTheDocument(),
    );
    expect(screen.getAllByText("View")).toHaveLength(2);
    expect(screen.queryByText("Edit")).not.toBeInTheDocument();
    expect(screen.queryByText("Delete")).not.toBeInTheDocument();
    expect(screen.queryByText("Clone")).not.toBeInTheDocument();
  });

  it("renders an empty state when there are no outcomes", async () => {
    server.use(
      http.get(`${API_BASE}/api/v1/versions/${VERSION_ID}/outcomes`, () =>
        HttpResponse.json([]),
      ),
    );
    render(
      <OutcomeListSection
        versionId={VERSION_ID}
        routeBase="/products/features/html/dn-article/1"
        editable
      />,
      { wrapper },
    );
    await waitFor(() =>
      expect(screen.getByText("No outcomes yet.")).toBeInTheDocument(),
    );
  });

  it("surfaces an error banner when Add Outcome fails", async () => {
    const user = userEvent.setup();
    server.use(
      http.get(`${API_BASE}/api/v1/versions/${VERSION_ID}/outcomes`, () =>
        HttpResponse.json([]),
      ),
      http.post(`${API_BASE}/api/v1/versions/${VERSION_ID}/outcomes`, () =>
        HttpResponse.json(
          { error: { code: "INTERNAL_ERROR", message: "boom" } },
          { status: 500 },
        ),
      ),
    );
    render(
      <OutcomeListSection
        versionId={VERSION_ID}
        routeBase="/products/features/html/dn-article/1"
        editable
      />,
      { wrapper },
    );
    await waitFor(() =>
      expect(screen.getByText("No outcomes yet.")).toBeInTheDocument(),
    );

    await user.click(screen.getByRole("button", { name: /add outcome/i }));
    await waitFor(() =>
      expect(screen.getByRole("alert")).toBeInTheDocument(),
    );
  });
});
