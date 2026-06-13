import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { http, HttpResponse } from "msw";
import type { ReactNode } from "react";
import { describe, expect, it, vi } from "vitest";

import { ComponentEditorPage } from "@/components/component-library/ComponentEditorPage";
import { API_BASE } from "@/lib/api/client";
import { server } from "@/test/mocks/server";

const push = vi.fn();
vi.mock("next/navigation", () => ({
  useRouter: () => ({ push }),
}));

// Stub the CodeMirror editor (dynamic, client-only) with a plain textarea so
// jsdom doesn't have to run CodeMirror's DOM-heavy init. The component under
// test only needs value/onChange — the lint gutter is exercised by the
// htmlLintEngine unit tests separately.
vi.mock("@/components/component-library/HtmlCodeEditor", () => ({
  HtmlCodeEditor: ({
    value,
    onChange,
  }: {
    value: string;
    onChange: (v: string) => void;
  }) => (
    <textarea
      aria-label="HTML editor"
      value={value}
      onChange={(e) => onChange(e.target.value)}
    />
  ),
}));

const BASE = `${API_BASE}/api/v1/component-templates`;
const CID = "11111111-1111-1111-1111-111111111111";
const SLUG = "paywall-cta";

const detail = {
  id: CID,
  slug: SLUG,
  name: "Paywall CTA",
  description: null,
  default_mode: "latest",
  default_version_number: 1,
  latest_version_number: 1,
  versions: [
    {
      id: "v1",
      version_number: 1,
      description: null,
      is_default: true,
      created_at: "2026-06-07T00:00:00Z",
      updated_at: "2026-06-07T00:00:00Z",
    },
  ],
  created_at: "2026-06-07T00:00:00Z",
  updated_at: "2026-06-07T00:00:00Z",
};

const versionRead = {
  id: "v1",
  version_number: 1,
  description: null,
  html_body: "<h1>{{headline}}</h1>",
  variables: [{ name: "headline", title: "Headline" }],
  is_default: true,
  created_at: "2026-06-07T00:00:00Z",
  updated_at: "2026-06-07T00:00:00Z",
};

function renderEditor() {
  const client = new QueryClient({
    defaultOptions: { queries: { retry: false }, mutations: { retry: false } },
  });
  function Wrapper({ children }: { children: ReactNode }) {
    return <QueryClientProvider client={client}>{children}</QueryClientProvider>;
  }
  return render(<ComponentEditorPage slug={SLUG} />, { wrapper: Wrapper });
}

function seedReadHandlers() {
  server.use(
    http.get(`${BASE}/by-slug/${SLUG}`, () => HttpResponse.json(detail)),
    http.get(`${BASE}/${CID}`, () => HttpResponse.json(detail)),
    http.get(`${BASE}/${CID}/versions/1`, () => HttpResponse.json(versionRead)),
  );
}

describe("ComponentEditorPage", () => {
  it("loads the component, version, and extracted variables", async () => {
    seedReadHandlers();
    renderEditor();
    expect(await screen.findByText("Paywall CTA")).toBeInTheDocument();
    // The annotated variable surfaces in the Variables panel.
    expect(
      await screen.findByLabelText("Title for headline"),
    ).toBeInTheDocument();
  });

  it("saves edits even when the HTML has lint warnings", async () => {
    seedReadHandlers();
    let patched: { html_body: string } | null = null;
    server.use(
      http.patch(`${BASE}/${CID}/versions/1`, async ({ request }) => {
        patched = (await request.json()) as { html_body: string };
        return HttpResponse.json({ ...versionRead, html_body: patched.html_body });
      }),
    );

    const user = userEvent.setup();
    renderEditor();

    const editor = await screen.findByLabelText("HTML editor");
    // Introduce HTML that the lint engine warns about (unclosed <div>) — Save
    // must still be enabled and must still PATCH (design item 8).
    await user.clear(editor);
    await user.type(editor, "<div><h1>{{headline}}</h1>");

    const save = screen.getByRole("button", { name: /save/i });
    expect(save).toBeEnabled();
    await user.click(save);

    await waitFor(() => expect(patched).not.toBeNull());
    expect(patched!.html_body).toContain("<div>");
  });

  it("includes an edited version description in the PATCH (design §5.3)", async () => {
    seedReadHandlers();
    let patched: { description?: string } | null = null;
    server.use(
      http.patch(`${BASE}/${CID}/versions/1`, async ({ request }) => {
        patched = (await request.json()) as { description?: string };
        return HttpResponse.json({
          ...versionRead,
          description: patched.description ?? null,
        });
      }),
    );

    const user = userEvent.setup();
    renderEditor();

    const desc = await screen.findByLabelText("Version description");
    await user.clear(desc);
    await user.type(desc, "Summer experiment");

    const save = screen.getByRole("button", { name: /save/i });
    expect(save).toBeEnabled();
    await user.click(save);

    await waitFor(() => expect(patched).not.toBeNull());
    expect(patched!.description).toBe("Summer experiment");
  });
});
