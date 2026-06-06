import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { http, HttpResponse } from "msw";
import type { ReactNode } from "react";
import { describe, expect, it, vi } from "vitest";

import { RowActionsMenu } from "@/components/versions/RowActionsMenu";
import { API_BASE } from "@/lib/api/client";
import type { VersionStatus } from "@/lib/api/enums";
import { server } from "@/test/mocks/server";

function wrapper({ children }: { children: ReactNode }) {
  const client = new QueryClient({
    defaultOptions: { queries: { retry: false } },
  });
  return (
    <QueryClientProvider client={client}>{children}</QueryClientProvider>
  );
}

function renderMenu(status: VersionStatus) {
  const onMakeLive = vi.fn();
  const onEditDescription = vi.fn();
  const onDelete = vi.fn();
  render(
    <RowActionsMenu
      featureId="demo-article"
      versionNumber={1}
      status={status}
      onMakeLive={onMakeLive}
      onEditDescription={onEditDescription}
      onDelete={onDelete}
    />,
    { wrapper },
  );
  return { onMakeLive, onEditDescription, onDelete };
}

describe("RowActionsMenu", () => {
  it("enables Delete and disables Unpublish for a DRAFT version", async () => {
    const user = userEvent.setup();
    renderMenu("draft");

    await user.click(
      screen.getByRole("button", { name: /actions for version 1/i }),
    );

    expect(screen.getByRole("menuitem", { name: "Unpublish" })).toHaveAttribute(
      "aria-disabled",
      "true",
    );
    expect(screen.getByRole("menuitem", { name: "Delete" })).not.toHaveAttribute(
      "aria-disabled",
      "true",
    );
  });

  it("enables Unpublish and disables Delete for a LIVE version", async () => {
    const user = userEvent.setup();
    renderMenu("live");

    await user.click(
      screen.getByRole("button", { name: /actions for version 1/i }),
    );

    expect(
      screen.getByRole("menuitem", { name: "Unpublish" }),
    ).not.toHaveAttribute("aria-disabled", "true");
    expect(screen.getByRole("menuitem", { name: "Delete" })).toHaveAttribute(
      "aria-disabled",
      "true",
    );
  });

  it("fires onEditDescription when Edit Description is chosen", async () => {
    const user = userEvent.setup();
    const { onEditDescription } = renderMenu("prev");

    await user.click(
      screen.getByRole("button", { name: /actions for version 1/i }),
    );
    await user.click(screen.getByRole("menuitem", { name: "Edit Description" }));

    expect(onEditDescription).toHaveBeenCalledOnce();
  });

  it("fires onDelete for a deletable (PREV) version", async () => {
    const user = userEvent.setup();
    const { onDelete } = renderMenu("prev");

    await user.click(
      screen.getByRole("button", { name: /actions for version 1/i }),
    );
    await user.click(screen.getByRole("menuitem", { name: "Delete" }));

    expect(onDelete).toHaveBeenCalledOnce();
  });

  it("enables Make Live for a DRAFT version and fires onMakeLive", async () => {
    const user = userEvent.setup();
    const { onMakeLive } = renderMenu("draft");

    await user.click(
      screen.getByRole("button", { name: /actions for version 1/i }),
    );
    const makeLive = screen.getByRole("menuitem", { name: "Make Live" });
    expect(makeLive).not.toHaveAttribute("aria-disabled", "true");
    await user.click(makeLive);
    expect(onMakeLive).toHaveBeenCalledOnce();
  });

  it("disables Make Live for an already-LIVE version", async () => {
    const user = userEvent.setup();
    renderMenu("live");

    await user.click(
      screen.getByRole("button", { name: /actions for version 1/i }),
    );
    expect(screen.getByRole("menuitem", { name: "Make Live" })).toHaveAttribute(
      "aria-disabled",
      "true",
    );
  });

  it("surfaces an error banner when Unpublish fails", async () => {
    const user = userEvent.setup();
    server.use(
      http.post(
        `${API_BASE}/api/v1/features/demo-article/versions/1/unpublish`,
        () =>
          HttpResponse.json(
            { error: { code: "INTERNAL_ERROR", message: "boom" } },
            { status: 500 },
          ),
      ),
    );
    renderMenu("live");

    await user.click(
      screen.getByRole("button", { name: /actions for version 1/i }),
    );
    await user.click(screen.getByRole("menuitem", { name: "Unpublish" }));

    await waitFor(() => expect(screen.getByRole("alert")).toBeInTheDocument());
    expect(
      screen.getByRole("button", { name: /dismiss/i }),
    ).toBeInTheDocument();
  });
});
