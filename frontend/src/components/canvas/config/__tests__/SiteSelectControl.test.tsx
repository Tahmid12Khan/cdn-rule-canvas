import { screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { http, HttpResponse } from "msw";
import { describe, expect, it, vi } from "vitest";

import { SiteSelectControl } from "@/components/canvas/config/SiteSelectControl";
import { API_BASE } from "@/lib/api/client";
import type { SiteRead } from "@/lib/api/sites";
import { renderWithQuery } from "@/test/renderWithQuery";
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
  headers: {},
  created_at: "2026-06-06T00:00:00Z",
  updated_at: "2026-06-06T00:00:00Z",
};

describe("SiteSelectControl", () => {
  it("opens on focus, searches, and stores the selected slug", async () => {
    const user = userEvent.setup();
    const onChange = vi.fn();
    server.use(
      http.get(SITES_URL, () =>
        HttpResponse.json({ items: [site], page: 1, page_size: 10, total: 1 }),
      ),
    );

    renderWithQuery(
      <SiteSelectControl id="field-site" value="" onChange={onChange} />,
    );

    await user.click(screen.getByRole("combobox"));
    await waitFor(() =>
      expect(screen.getByText("Demo (localhost:9000)")).toBeInTheDocument(),
    );

    // Each option is a <li role="option"> wrapping a <button>; the click
    // handler lives on the button.
    await user.click(screen.getByRole("button", { name: /demo/i }));
    expect(onChange).toHaveBeenCalledWith("demo-localhost");
  });

  it("resolves the selected slug to 'name (slug)' and clears it", async () => {
    const user = userEvent.setup();
    const onChange = vi.fn();
    server.use(
      http.get(`${SITES_URL}/:slug`, () => HttpResponse.json(site)),
    );

    renderWithQuery(
      <SiteSelectControl
        id="field-site"
        value="demo-localhost"
        onChange={onChange}
      />,
    );

    // getSite resolves -> the selection reads "name (slug)" (spec §6).
    await waitFor(() =>
      expect(
        screen.getByText("Demo (localhost:9000) (demo-localhost)"),
      ).toBeInTheDocument(),
    );
    await user.click(screen.getByRole("button", { name: /clear/i }));
    expect(onChange).toHaveBeenCalledWith("");
  });

  it("falls back to the bare slug while the site is unresolved", () => {
    const onChange = vi.fn();
    server.use(
      http.get(`${SITES_URL}/:slug`, () =>
        HttpResponse.json(
          { error: { code: "NOT_FOUND", message: "gone" } },
          { status: 404 },
        ),
      ),
    );

    renderWithQuery(
      <SiteSelectControl
        id="field-site"
        value="ghost-site"
        onChange={onChange}
      />,
    );

    // Before/without resolution the raw slug is shown.
    expect(screen.getByText("ghost-site")).toBeInTheDocument();
  });
});
