import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { http, HttpResponse } from "msw";
import type { ReactNode } from "react";
import { describe, expect, it, vi } from "vitest";

import { TEST_PRESET_EXAMPLES } from "@/components/test-presets/TestPresetExamples";
import { TestPresetFormModal } from "@/components/test-presets/TestPresetFormModal";
import { API_BASE } from "@/lib/api/client";
import type { TestPresetRead } from "@/lib/api/test-presets";
import { server } from "@/test/mocks/server";

function wrapper({ children }: { children: ReactNode }) {
  const client = new QueryClient({
    defaultOptions: { queries: { retry: false }, mutations: { retry: false } },
  });
  return <QueryClientProvider client={client}>{children}</QueryClientProvider>;
}

const PRESETS_URL = `${API_BASE}/api/v1/test-presets`;
const trigger = <button type="button">+ Add A Test Preset</button>;

const existing: TestPresetRead = {
  slug: "mobile-paywall",
  name: "Mobile paywall",
  kind: "rule",
  payload: { feature_type: "html", device_type: "mobile" },
  created_at: "2026-06-06T00:00:00Z",
  updated_at: "2026-06-06T00:00:00Z",
};

async function openModal(user: ReturnType<typeof userEvent.setup>) {
  await user.click(screen.getByRole("button", { name: /add a test preset/i }));
  await screen.findByRole("dialog");
}

describe("TestPresetFormModal", () => {
  it("shows validation errors and does not submit on invalid input", async () => {
    const user = userEvent.setup();
    const onPost = vi.fn();
    server.use(
      http.post(PRESETS_URL, () => {
        onPost();
        return HttpResponse.json({}, { status: 201 });
      }),
    );

    render(<TestPresetFormModal trigger={trigger} />, { wrapper });
    await openModal(user);

    await user.type(screen.getByLabelText(/^slug$/i), "Bad_Slug");
    await user.click(screen.getByRole("button", { name: /create preset/i }));

    // Match the validation error ("Use lowercase…") specifically — the always-on
    // help text ("Lowercase kebab-case, e.g. …") also mentions kebab-case.
    await waitFor(() =>
      expect(
        screen.getByText(/use lowercase kebab-case/i),
      ).toBeInTheDocument(),
    );
    expect(screen.getByText(/name is required/i)).toBeInTheDocument();
    expect(onPost).not.toHaveBeenCalled();
  });

  it("blocks submit with an inline error on a non-object payload", async () => {
    const user = userEvent.setup();
    const onPost = vi.fn();
    server.use(
      http.post(PRESETS_URL, () => {
        onPost();
        return HttpResponse.json({}, { status: 201 });
      }),
    );

    render(<TestPresetFormModal trigger={trigger} />, { wrapper });
    await openModal(user);

    await user.type(screen.getByLabelText(/slug/i), "good-slug");
    await user.type(screen.getByLabelText(/^name$/i), "Good");
    const payload = screen.getByLabelText(/payload/i);
    await user.clear(payload);
    // A bare number is valid JSON but NOT an object (avoids userEvent's special
    // handling of [ ] { } characters).
    await user.type(payload, "42");
    await user.click(screen.getByRole("button", { name: /create preset/i }));

    await waitFor(() =>
      expect(screen.getByText(/must be a json object/i)).toBeInTheDocument(),
    );
    expect(onPost).not.toHaveBeenCalled();
  });

  it("submits valid input and closes the modal", async () => {
    const user = userEvent.setup();
    let body: unknown = null;
    server.use(
      http.post(PRESETS_URL, async ({ request }) => {
        body = await request.json();
        return HttpResponse.json(existing, { status: 201 });
      }),
    );

    render(<TestPresetFormModal trigger={trigger} />, { wrapper });
    await openModal(user);

    await user.type(screen.getByLabelText(/slug/i), "mobile-paywall");
    await user.type(screen.getByLabelText(/^name$/i), "Mobile paywall");
    // The payload textarea defaults to "{}" (a valid empty object); leave it.
    await user.click(screen.getByRole("button", { name: /create preset/i }));

    await waitFor(() =>
      expect(screen.queryByRole("dialog")).not.toBeInTheDocument(),
    );
    expect(body).toMatchObject({
      slug: "mobile-paywall",
      name: "Mobile paywall",
      kind: "rule",
      payload: {},
    });
  });

  it("edits a preset: slug + kind locked, PATCH carries name + payload", async () => {
    const user = userEvent.setup();
    let patchBody: unknown = null;
    let patchedSlug: string | null = null;
    server.use(
      http.patch(`${PRESETS_URL}/:slug`, async ({ request, params }) => {
        patchedSlug = params.slug as string;
        patchBody = await request.json();
        return HttpResponse.json(
          { ...existing, name: "Renamed" },
          { status: 200 },
        );
      }),
    );

    const onOpenChange = vi.fn();
    render(
      <TestPresetFormModal
        preset={existing}
        open
        onOpenChange={onOpenChange}
      />,
      { wrapper },
    );
    await screen.findByRole("dialog");

    expect(screen.getByLabelText(/slug/i)).toBeDisabled();
    expect(screen.getByLabelText(/kind/i)).toBeDisabled();

    const nameInput = screen.getByLabelText(/^name$/i);
    await user.clear(nameInput);
    await user.type(nameInput, "Renamed");
    await user.click(screen.getByRole("button", { name: /save preset/i }));

    await waitFor(() => expect(patchBody).not.toBeNull());
    expect(patchedSlug).toBe("mobile-paywall");
    expect(patchBody).not.toHaveProperty("slug");
    expect(patchBody).toMatchObject({ name: "Renamed" });
    await waitFor(() => expect(onOpenChange).toHaveBeenCalledWith(false));
  });

  it("quick mode: hides Kind + Payload and saves the passed payload + kind", async () => {
    const user = userEvent.setup();
    let body: unknown = null;
    server.use(
      http.post(PRESETS_URL, async ({ request }) => {
        body = await request.json();
        return HttpResponse.json(existing, { status: 201 });
      }),
    );

    render(
      <TestPresetFormModal
        mode="quick"
        presetKind="url"
        presetPayload={{ url: "https://example.com/a", headers: {} }}
        trigger={trigger}
      />,
      { wrapper },
    );
    await openModal(user);

    // Quick mode shows only Name + Slug — no Kind selector, no Payload JSON.
    expect(screen.queryByLabelText(/kind/i)).not.toBeInTheDocument();
    expect(screen.queryByLabelText(/payload/i)).not.toBeInTheDocument();

    await user.type(screen.getByLabelText(/^slug$/i), "live-check");
    await user.type(screen.getByLabelText(/^name$/i), "Live check");
    await user.click(screen.getByRole("button", { name: /create preset/i }));

    await waitFor(() =>
      expect(screen.queryByRole("dialog")).not.toBeInTheDocument(),
    );
    // Kind comes from presetKind; payload is the panel-supplied object verbatim.
    expect(body).toMatchObject({
      slug: "live-check",
      name: "Live check",
      kind: "url",
      payload: { url: "https://example.com/a", headers: {} },
    });
  });

  it('"Use this template" pre-fills the full-mode form fields', async () => {
    const user = userEvent.setup();
    const example = TEST_PRESET_EXAMPLES[0]; // "Mobile paywall test" (rule)

    render(<TestPresetFormModal trigger={trigger} />, { wrapper });
    await openModal(user);

    // Open the collapsible "Start from an example" helper, then use the first.
    await user.click(
      screen.getByRole("button", { name: /start from an example/i }),
    );
    const cards = await screen.findAllByRole("button", {
      name: /use this template/i,
    });
    await user.click(cards[0]);

    // The form is now pre-filled from the example (slug/name/kind/payload).
    expect(screen.getByLabelText(/^slug$/i)).toHaveValue(example.slug);
    expect(screen.getByLabelText(/^name$/i)).toHaveValue(example.name);
    expect(screen.getByLabelText(/kind/i)).toHaveValue(example.kind);
    expect(screen.getByLabelText(/payload/i)).toHaveValue(
      JSON.stringify(example.payload, null, 2),
    );
  });
});
