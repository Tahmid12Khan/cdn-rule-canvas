import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { http, HttpResponse } from "msw";
import type { ReactNode } from "react";
import { describe, expect, it, vi } from "vitest";

import { TestPresetBar } from "@/components/canvas/TestPresetBar";
import { API_BASE } from "@/lib/api/client";
import type { TestPresetRead } from "@/lib/api/test-presets";
import { server } from "@/test/mocks/server";

const PRESETS_URL = `${API_BASE}/api/v1/test-presets`;

function wrapper({ children }: { children: ReactNode }) {
  const client = new QueryClient({
    defaultOptions: { queries: { retry: false }, mutations: { retry: false } },
  });
  return <QueryClientProvider client={client}>{children}</QueryClientProvider>;
}

const sample: TestPresetRead = {
  slug: "mobile-paywall",
  name: "Mobile paywall",
  kind: "rule",
  payload: { feature_type: "html", device_type: "mobile" },
  created_at: "2026-06-06T00:00:00Z",
  updated_at: "2026-06-06T00:00:00Z",
};

describe("TestPresetBar", () => {
  it("loads a selected preset's payload via onLoad", async () => {
    const user = userEvent.setup();
    const onLoad = vi.fn();
    server.use(
      http.get(PRESETS_URL, () =>
        HttpResponse.json({
          items: [sample],
          page: 1,
          page_size: 100,
          total: 1,
        }),
      ),
    );

    render(
      <TestPresetBar
        kind="rule"
        currentPayload={{ feature_type: "html" }}
        payloadValid
        onLoad={onLoad}
      />,
      { wrapper },
    );

    // The select option appears once the list resolves.
    await waitFor(() =>
      expect(
        screen.getByRole("option", { name: "Mobile paywall" }),
      ).toBeInTheDocument(),
    );

    await user.selectOptions(
      screen.getByLabelText(/saved tests/i),
      "mobile-paywall",
    );
    expect(onLoad).toHaveBeenCalledWith(sample.payload);
  });

  it("saves the current payload as a new preset (quick mode: no Kind / Payload JSON)", async () => {
    const user = userEvent.setup();
    let posted: { slug?: string; kind?: string; payload?: unknown } | null =
      null;
    server.use(
      http.get(PRESETS_URL, () =>
        HttpResponse.json({ items: [], page: 1, page_size: 100, total: 0 }),
      ),
      http.post(PRESETS_URL, async ({ request }) => {
        posted = (await request.json()) as typeof posted;
        return HttpResponse.json(sample, { status: 201 });
      }),
    );

    render(
      <TestPresetBar
        kind="rule"
        currentPayload={{ feature_type: "html", device_type: "mobile" }}
        payloadValid
        onLoad={vi.fn()}
      />,
      { wrapper },
    );

    await user.click(screen.getByRole("button", { name: /^save$/i }));
    await screen.findByRole("dialog");

    // Quick mode hides the Kind selector + the raw Payload JSON textarea so a
    // non-technical user never edits JSON.
    expect(screen.queryByLabelText(/kind/i)).not.toBeInTheDocument();
    expect(screen.queryByLabelText(/payload/i)).not.toBeInTheDocument();

    await user.type(screen.getByLabelText(/^name$/i), "Mobile paywall");
    await user.type(screen.getByLabelText(/^slug$/i), "mobile-paywall");
    await user.click(screen.getByRole("button", { name: /create preset/i }));

    await waitFor(() => expect(posted).not.toBeNull());
    expect(posted!.slug).toBe("mobile-paywall");
    // Kind is auto-derived from the calling panel; payload is the panel's
    // current inputs verbatim.
    expect(posted!.kind).toBe("rule");
    expect(posted!.payload).toEqual({
      feature_type: "html",
      device_type: "mobile",
    });
  });

  it("disables Save when the payload is invalid", async () => {
    server.use(
      http.get(PRESETS_URL, () =>
        HttpResponse.json({ items: [], page: 1, page_size: 100, total: 0 }),
      ),
    );

    render(
      <TestPresetBar
        kind="url"
        currentPayload={{ url: "", headers: {} }}
        payloadValid={false}
        onLoad={vi.fn()}
      />,
      { wrapper },
    );

    expect(screen.getByRole("button", { name: /^save$/i })).toBeDisabled();
  });
});
