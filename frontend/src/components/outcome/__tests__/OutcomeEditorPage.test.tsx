import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { http, HttpResponse } from "msw";
import type { ReactNode } from "react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import { OutcomeEditorPage } from "@/components/outcome/OutcomeEditorPage";
import { API_BASE } from "@/lib/api/client";
import { server } from "@/test/mocks/server";

const push = vi.fn();
vi.mock("next/navigation", () => ({
  useRouter: () => ({ push }),
}));

const OUTCOME_ID = "33333333-3333-3333-3333-333333333333";
const VERSION_ID = "44444444-4444-4444-4444-444444444444";
const COMP_A = "aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa";
const COMP_B = "bbbbbbbb-bbbb-bbbb-bbbb-bbbbbbbbbbbb";

function component(id: string, slug: string, order: number) {
  return {
    id,
    outcome_id: OUTCOME_ID,
    slug,
    type: "html_injection",
    config: {
      type: "html_injection",
      target_selector: ".body",
      placement_mode: "append",
      html_body: "<p>x</p>",
    },
    placement: "inline",
    order_index: order,
    created_at: "2024-01-01T00:00:00Z",
    updated_at: "2024-01-01T00:00:00Z",
  };
}

function outcomePayload(over: Record<string, unknown> = {}) {
  return {
    id: OUTCOME_ID,
    version_id: VERSION_ID,
    title: "DN Regwall 1.0",
    description: "Original",
    is_builtin: false,
    order_index: 1,
    components: [component(COMP_A, "alpha", 0), component(COMP_B, "beta", 1)],
    created_at: "2024-01-01T00:00:00Z",
    updated_at: "2024-01-01T00:00:00Z",
    ...over,
  };
}

const emptyCanvas = { nodes: [], edges: [], root_node_id: null };

function versionPayload(status: string) {
  return {
    id: VERSION_ID,
    feature_id: "dn-article",
    version_number: 3,
    description: null,
    status,
    rule_graph: {
      anonymous: emptyCanvas,
      registered: emptyCanvas,
      customer: emptyCanvas,
    },
    created_by: "tester",
    last_updated_by: "tester",
    last_updated_at: "2024-01-01T00:00:00Z",
    created_at: "2024-01-01T00:00:00Z",
  };
}

function renderPage() {
  const client = new QueryClient({
    defaultOptions: { queries: { retry: false } },
  });
  function wrapper({ children }: { children: ReactNode }) {
    return <QueryClientProvider client={client}>{children}</QueryClientProvider>;
  }
  return render(
    <OutcomeEditorPage
      featureType="html"
      featureSlug="dn-article"
      vnum="3"
      outcomeId={OUTCOME_ID}
    />,
    { wrapper },
  );
}

beforeEach(() => {
  push.mockReset();
  server.use(
    http.get(`${API_BASE}/api/v1/outcomes/${OUTCOME_ID}`, () =>
      HttpResponse.json(outcomePayload()),
    ),
    // Default: a DRAFT version so the editor is editable.
    http.get(`${API_BASE}/api/v1/features/dn-article/versions/3`, () =>
      HttpResponse.json(versionPayload("draft")),
    ),
  );
});

describe("OutcomeEditorPage", () => {
  it("renders the outcome details and component rows", async () => {
    renderPage();
    expect(
      await screen.findByDisplayValue("DN Regwall 1.0"),
    ).toBeInTheDocument();
    expect(screen.getByText("alpha")).toBeInTheDocument();
    expect(screen.getByText("beta")).toBeInTheDocument();
    expect(
      screen.getByRole("heading", { name: /edit an outcome/i }),
    ).toBeInTheDocument();
  });

  it("disables Save when there are no changes and when the title is empty", async () => {
    const user = userEvent.setup();
    renderPage();
    await screen.findByDisplayValue("DN Regwall 1.0");

    const save = screen.getByRole("button", { name: /^save$/i });
    expect(save).toBeDisabled(); // pristine

    // Make it dirty but invalid: clear the title.
    const title = screen.getByLabelText(/title/i);
    await user.clear(title);
    expect(
      screen.getByText(/give this outcome a title/i),
    ).toBeInTheDocument();
    expect(save).toBeDisabled();

    // Valid edit re-enables Save.
    await user.type(title, "New Title");
    expect(save).toBeEnabled();
  });

  it("Cancel with unsaved changes shows a discard confirm before navigating", async () => {
    const user = userEvent.setup();
    renderPage();
    const title = await screen.findByDisplayValue("DN Regwall 1.0");

    await user.type(title, " edited");
    await user.click(screen.getByRole("button", { name: /^cancel$/i }));

    // Confirm dialog appears; navigation has not happened yet.
    expect(screen.getByText(/discard changes\?/i)).toBeInTheDocument();
    expect(push).not.toHaveBeenCalled();

    await user.click(screen.getByRole("button", { name: /^discard$/i }));
    expect(push).toHaveBeenCalledWith(
      "/products/features/html/dn-article/3",
    );
  });

  it("Cancel without changes navigates immediately", async () => {
    const user = userEvent.setup();
    renderPage();
    await screen.findByDisplayValue("DN Regwall 1.0");

    await user.click(screen.getByRole("button", { name: /^cancel$/i }));
    expect(screen.queryByText(/discard changes\?/i)).not.toBeInTheDocument();
    expect(push).toHaveBeenCalledWith("/products/features/html/dn-article/3");
  });

  it("deletes a persisted component and fires DELETE on Save", async () => {
    const user = userEvent.setup();
    const deleted: string[] = [];
    const reorderBodies: unknown[] = [];
    server.use(
      http.delete(`${API_BASE}/api/v1/components/:cid`, ({ params }) => {
        deleted.push(params.cid as string);
        return new HttpResponse(null, { status: 204 });
      }),
      http.post(
        `${API_BASE}/api/v1/outcomes/${OUTCOME_ID}/reorder`,
        async ({ request }) => {
          reorderBodies.push(await request.json());
          return HttpResponse.json([]);
        },
      ),
    );

    renderPage();
    await screen.findByText("alpha");

    await user.click(screen.getByRole("button", { name: /delete alpha/i }));
    expect(screen.queryByText("alpha")).not.toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: /^save$/i }));

    await waitFor(() => expect(deleted).toContain(COMP_A));
    // The remaining component is reindexed from 0.
    expect(reorderBodies[0]).toEqual([{ id: COMP_B, order_index: 0 }]);
  });

  it("adds a component via the config modal and POSTs it with a sequential order_index on Save", async () => {
    const user = userEvent.setup();
    let postBody: Record<string, unknown> | null = null;
    const NEW_ID = "cccccccc-cccc-cccc-cccc-cccccccccccc";
    server.use(
      http.post(
        `${API_BASE}/api/v1/outcomes/${OUTCOME_ID}/components`,
        async ({ request }) => {
          postBody = (await request.json()) as Record<string, unknown>;
          return HttpResponse.json({
            ...component(NEW_ID, postBody.slug as string, 2),
          });
        },
      ),
      http.post(`${API_BASE}/api/v1/outcomes/${OUTCOME_ID}/reorder`, () =>
        HttpResponse.json([]),
      ),
    );

    renderPage();
    await screen.findByText("alpha");

    // Open the add drawer and pick HTML Injection.
    await user.click(
      screen.getByRole("button", { name: /add a component or form/i }),
    );
    const drawer = await screen.findByRole("dialog");
    await user.click(
      within(drawer).getByRole("button", { name: /html injection/i }),
    );

    // Fill the config modal (slug + selector + html body) and submit.
    const dialog = await screen.findByRole("dialog");
    const scoped = within(dialog);
    await user.type(scoped.getByLabelText(/^slug$/i), "promo");
    await user.type(scoped.getByLabelText(/target selector/i), ".article");
    await user.type(scoped.getByLabelText(/html content/i), "<p>promo</p>");
    await user.click(
      scoped.getByRole("button", { name: /save component/i }),
    );

    // The new row appears with a "New" marker.
    await waitFor(() => expect(screen.getByText("promo")).toBeInTheDocument());
    expect(screen.getByText("New")).toBeInTheDocument();

    // Save commits it.
    await user.click(screen.getByRole("button", { name: /^save$/i }));
    await waitFor(() => expect(postBody).not.toBeNull());
    expect(postBody).toMatchObject({
      slug: "promo",
      type: "html_injection",
      placement: "inline",
      order_index: 2,
      config: { type: "html_injection", target_selector: ".article" },
    });
  });

  it("a sticky_footer placement is reachable via the config modal's Placement select", async () => {
    const user = userEvent.setup();
    renderPage();
    await screen.findByText("alpha");

    // Add a component via the drawer, then switch placement to sticky_footer.
    await user.click(
      screen.getByRole("button", { name: /add a component or form/i }),
    );
    const drawer = await screen.findByRole("dialog");
    await user.click(
      within(drawer).getByRole("button", { name: /html injection/i }),
    );

    const dialog = await screen.findByRole("dialog");
    const placement = within(dialog).getByLabelText(/^placement$/i);
    // Defaults to inline, but sticky_footer is selectable.
    expect(placement).toHaveValue("inline");
    await user.selectOptions(placement, "sticky_footer");
    expect(placement).toHaveValue("sticky_footer");
  });

  it("renders read-only with no Save button on a non-draft (locked) version", async () => {
    server.use(
      http.get(`${API_BASE}/api/v1/features/dn-article/versions/3`, () =>
        HttpResponse.json(versionPayload("live")),
      ),
    );
    renderPage();

    expect(await screen.findByDisplayValue("DN Regwall 1.0")).toBeDisabled();
    expect(
      screen.getByRole("heading", { name: /view an outcome/i }),
    ).toBeInTheDocument();
    expect(
      screen.getByText(/this version is published and locked/i),
    ).toBeInTheDocument();
    expect(
      screen.queryByRole("button", { name: /^save$/i }),
    ).not.toBeInTheDocument();
  });
});
