import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { http, HttpResponse } from "msw";
import type { ReactNode } from "react";
import { describe, expect, it, vi } from "vitest";

import { SiteFormModal } from "@/components/sites/SiteFormModal";
import { API_BASE } from "@/lib/api/client";
import type { SiteRead } from "@/lib/api/sites";
import { server } from "@/test/mocks/server";

function wrapper({ children }: { children: ReactNode }) {
  const client = new QueryClient({
    defaultOptions: { queries: { retry: false }, mutations: { retry: false } },
  });
  return (
    <QueryClientProvider client={client}>{children}</QueryClientProvider>
  );
}

const SITES_URL = `${API_BASE}/api/v1/sites`;

const trigger = <button type="button">+ Add A New Site</button>;

const existingSite: SiteRead = {
  slug: "demo-localhost",
  name: "Demo (localhost:9000)",
  source_protocol: "http",
  source_host: "localhost",
  source_port: 9000,
  dest_protocol: "http",
  dest_host: "demo-upstream",
  dest_port: 8081,
  headers: {},
  created_at: "2026-06-06T00:00:00Z",
  updated_at: "2026-06-06T00:00:00Z",
};

async function openModal(user: ReturnType<typeof userEvent.setup>) {
  await user.click(screen.getByRole("button", { name: /add a new site/i }));
  await screen.findByRole("dialog");
}

describe("SiteFormModal", () => {
  it("shows validation errors and does not submit on invalid input", async () => {
    const user = userEvent.setup();
    const onPost = vi.fn();
    server.use(
      http.post(SITES_URL, () => {
        onPost();
        return HttpResponse.json({}, { status: 201 });
      }),
    );

    render(<SiteFormModal trigger={trigger} />, { wrapper });
    await openModal(user);

    // Invalid slug + empty everything else.
    await user.type(screen.getByLabelText(/slug/i), "Bad_Slug");
    await user.click(screen.getByRole("button", { name: /create site/i }));

    await waitFor(() =>
      expect(screen.getByText(/lowercase kebab-case/i)).toBeInTheDocument(),
    );
    expect(screen.getByText(/name is required/i)).toBeInTheDocument();
    expect(onPost).not.toHaveBeenCalled();
  });

  it("submits valid input (coerced ports) and closes the modal", async () => {
    const user = userEvent.setup();
    let body: unknown = null;
    server.use(
      http.post(SITES_URL, async ({ request }) => {
        body = await request.json();
        return HttpResponse.json(
          {
            slug: "demo-localhost",
            name: "Demo",
            source_protocol: "http",
            source_host: "localhost",
            source_port: 9000,
            dest_protocol: "http",
            dest_host: "demo-upstream",
            dest_port: 8081,
            created_at: "2026-06-06T00:00:00Z",
            updated_at: "2026-06-06T00:00:00Z",
          },
          { status: 201 },
        );
      }),
    );

    render(<SiteFormModal trigger={trigger} />, { wrapper });
    await openModal(user);

    await user.type(screen.getByLabelText(/slug/i), "demo-localhost");
    await user.type(screen.getByLabelText(/^name$/i), "Demo");
    const hosts = screen.getAllByLabelText(/host/i);
    await user.type(hosts[0], "localhost");
    await user.type(hosts[1], "demo-upstream");
    const ports = screen.getAllByLabelText(/port/i);
    await user.type(ports[0], "9000");
    await user.type(ports[1], "8081");
    await user.click(screen.getByRole("button", { name: /create site/i }));

    await waitFor(() =>
      expect(screen.queryByRole("dialog")).not.toBeInTheDocument(),
    );
    expect(body).toEqual({
      slug: "demo-localhost",
      name: "Demo",
      source_protocol: "http",
      source_host: "localhost",
      source_port: 9000,
      dest_protocol: "http",
      dest_host: "demo-upstream",
      dest_port: 8081,
      headers: {},
    });
  });

  it("surfaces a 409 conflict with sites-aware copy and keeps the dialog open", async () => {
    const user = userEvent.setup();
    // Backend returns 409 SLUG_CONFLICT for slug/name/source uniqueness alike.
    server.use(
      http.post(SITES_URL, () =>
        HttpResponse.json(
          {
            error: {
              code: "SLUG_CONFLICT",
              message: "a site with this slug/name/source already exists",
            },
          },
          { status: 409 },
        ),
      ),
    );

    render(<SiteFormModal trigger={trigger} />, { wrapper });
    await openModal(user);

    await user.type(screen.getByLabelText(/slug/i), "demo-localhost");
    await user.type(screen.getByLabelText(/^name$/i), "Demo");
    const hosts = screen.getAllByLabelText(/host/i);
    await user.type(hosts[0], "localhost");
    await user.type(hosts[1], "demo-upstream");
    const ports = screen.getAllByLabelText(/port/i);
    await user.type(ports[0], "9000");
    await user.type(ports[1], "8081");
    await user.click(screen.getByRole("button", { name: /create site/i }));

    // The site-conflict copy must NOT leak the version-locked / generic
    // "That slug is already taken" wording.
    await waitFor(() =>
      expect(
        screen.getByText(
          /a site with this slug, name, or source host:port already exists/i,
        ),
      ).toBeInTheDocument(),
    );
    expect(screen.getByRole("alert")).toBeInTheDocument();
    expect(screen.queryByText(/save as new version/i)).not.toBeInTheDocument();
    expect(screen.getByRole("dialog")).toBeInTheDocument();
  });

  it("edits a site: slug is locked and PATCH omits the slug", async () => {
    const user = userEvent.setup();
    let patchBody: unknown = null;
    let patchedSlug: string | null = null;
    server.use(
      http.patch(`${SITES_URL}/:slug`, async ({ request, params }) => {
        patchedSlug = params.slug as string;
        patchBody = await request.json();
        return HttpResponse.json(
          { ...existingSite, name: "Renamed Demo" },
          { status: 200 },
        );
      }),
    );

    const onOpenChange = vi.fn();
    render(
      <SiteFormModal site={existingSite} open onOpenChange={onOpenChange} />,
      { wrapper },
    );
    await screen.findByRole("dialog");

    // Slug is immutable in edit mode.
    const slugInput = screen.getByLabelText(/slug/i);
    expect(slugInput).toBeDisabled();

    // Edit the name, then save.
    const nameInput = screen.getByLabelText(/^name$/i);
    await user.clear(nameInput);
    await user.type(nameInput, "Renamed Demo");
    await user.click(screen.getByRole("button", { name: /save site/i }));

    await waitFor(() => expect(patchBody).not.toBeNull());
    expect(patchedSlug).toBe("demo-localhost");
    // Slug must be omitted from the PATCH body (it is path-derived / immutable).
    expect(patchBody).not.toHaveProperty("slug");
    expect(patchBody).toMatchObject({ name: "Renamed Demo" });
    // The modal asks the parent to close on success.
    await waitFor(() => expect(onOpenChange).toHaveBeenCalledWith(false));
  });

  // Fill the required base fields so a submit only hinges on the header rows.
  async function fillBaseFields(user: ReturnType<typeof userEvent.setup>) {
    await user.type(screen.getByLabelText(/slug/i), "demo-localhost");
    await user.type(screen.getByLabelText(/^name$/i), "Demo");
    const hosts = screen.getAllByLabelText(/^host$/i);
    await user.type(hosts[0], "localhost");
    await user.type(hosts[1], "demo-upstream");
    const ports = screen.getAllByLabelText(/^port$/i);
    await user.type(ports[0], "9000");
    await user.type(ports[1], "8081");
  }

  it("serializes two header rows into the headers map on submit", async () => {
    const user = userEvent.setup();
    let body: { headers?: Record<string, string> } | null = null;
    server.use(
      http.post(SITES_URL, async ({ request }) => {
        body = (await request.json()) as { headers?: Record<string, string> };
        return HttpResponse.json({ ...existingSite }, { status: 201 });
      }),
    );

    render(<SiteFormModal trigger={trigger} />, { wrapper });
    await openModal(user);
    await fillBaseFields(user);

    const addHeader = screen.getByRole("button", { name: /add header/i });
    await user.click(addHeader);
    await user.click(addHeader);

    await user.type(screen.getByLabelText(/header name 1/i), "X-One");
    await user.type(screen.getByLabelText(/header value 1/i), "val1");
    await user.type(screen.getByLabelText(/header name 2/i), "X-Two");
    await user.type(screen.getByLabelText(/header value 2/i), "val2");

    await user.click(screen.getByRole("button", { name: /create site/i }));

    await waitFor(() => expect(body).not.toBeNull());
    expect(body!.headers).toEqual({ "X-One": "val1", "X-Two": "val2" });
  });

  it("blocks submit with an inline error on an invalid header name", async () => {
    const user = userEvent.setup();
    const onPost = vi.fn();
    server.use(
      http.post(SITES_URL, () => {
        onPost();
        return HttpResponse.json({}, { status: 201 });
      }),
    );

    render(<SiteFormModal trigger={trigger} />, { wrapper });
    await openModal(user);
    await fillBaseFields(user);

    await user.click(screen.getByRole("button", { name: /add header/i }));
    // Space is not a valid HTTP token character.
    await user.type(screen.getByLabelText(/header name 1/i), "Bad Header");
    await user.type(screen.getByLabelText(/header value 1/i), "x");

    await user.click(screen.getByRole("button", { name: /create site/i }));

    await waitFor(() =>
      expect(screen.getByText(/invalid header name/i)).toBeInTheDocument(),
    );
    expect(onPost).not.toHaveBeenCalled();
  });

  it("removing a header row drops it from the submitted map", async () => {
    const user = userEvent.setup();
    let body: { headers?: Record<string, string> } | null = null;
    server.use(
      http.post(SITES_URL, async ({ request }) => {
        body = (await request.json()) as { headers?: Record<string, string> };
        return HttpResponse.json({ ...existingSite }, { status: 201 });
      }),
    );

    render(<SiteFormModal trigger={trigger} />, { wrapper });
    await openModal(user);
    await fillBaseFields(user);

    const addHeader = screen.getByRole("button", { name: /add header/i });
    await user.click(addHeader);
    await user.click(addHeader);

    await user.type(screen.getByLabelText(/header name 1/i), "X-Keep");
    await user.type(screen.getByLabelText(/header value 1/i), "keep");
    await user.type(screen.getByLabelText(/header name 2/i), "X-Drop");
    await user.type(screen.getByLabelText(/header value 2/i), "drop");

    // Remove the second row.
    await user.click(screen.getByRole("button", { name: /remove header 2/i }));

    await user.click(screen.getByRole("button", { name: /create site/i }));

    await waitFor(() => expect(body).not.toBeNull());
    expect(body!.headers).toEqual({ "X-Keep": "keep" });
  });

  it("seeds header rows from an existing site in edit mode", async () => {
    render(
      <SiteFormModal
        site={{ ...existingSite, headers: { "X-Seed": "seeded" } }}
        open
        onOpenChange={vi.fn()}
      />,
      { wrapper },
    );
    await screen.findByRole("dialog");

    expect(screen.getByLabelText(/header name 1/i)).toHaveValue("X-Seed");
    expect(screen.getByLabelText(/header value 1/i)).toHaveValue("seeded");
  });
});
