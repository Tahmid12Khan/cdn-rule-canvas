import { screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { http, HttpResponse } from "msw";
import { describe, expect, it, vi } from "vitest";

import { SavedOutcomeSelectControl } from "@/components/canvas/config/SavedOutcomeSelectControl";
import { API_BASE } from "@/lib/api/client";
import type { SavedOutcomeRead } from "@/lib/api/savedOutcomes";
import { renderWithQuery } from "@/test/renderWithQuery";
import { server } from "@/test/mocks/server";

const OUTCOMES_URL = `${API_BASE}/api/v1/saved-outcomes`;

const outcome: SavedOutcomeRead = {
  id: "11111111-1111-1111-1111-111111111111",
  slug: "promo-banner-default",
  name: "Promo Banner (Default)",
  component_id: "22222222-2222-2222-2222-222222222222",
  component_name: "Promo Banner",
  version_number: null,
  variables: { headline: "Sale" },
  created_at: "2026-06-06T00:00:00Z",
  updated_at: "2026-06-06T00:00:00Z",
};

describe("SavedOutcomeSelectControl", () => {
  it("opens on focus, searches, and stores the selected id", async () => {
    const user = userEvent.setup();
    const onChange = vi.fn();
    server.use(
      http.get(OUTCOMES_URL, () =>
        HttpResponse.json({ items: [outcome], page: 1, page_size: 10, total: 1 }),
      ),
    );

    renderWithQuery(
      <SavedOutcomeSelectControl id="field-outcome" value="" onChange={onChange} />,
    );

    await user.click(screen.getByRole("combobox"));
    await waitFor(() =>
      expect(screen.getByText("Promo Banner (Default)")).toBeInTheDocument(),
    );

    // Each option is a <li role="option"> wrapping a <button>; the click
    // handler lives on the button.
    await user.click(screen.getByRole("button", { name: /promo banner/i }));
    expect(onChange).toHaveBeenCalledWith(outcome.id);
  });

  it("resolves the selected id to 'name (component_name)' and clears it", async () => {
    const user = userEvent.setup();
    const onChange = vi.fn();
    server.use(
      http.get(`${OUTCOMES_URL}/:id`, () => HttpResponse.json(outcome)),
    );

    renderWithQuery(
      <SavedOutcomeSelectControl
        id="field-outcome"
        value={outcome.id}
        onChange={onChange}
      />,
    );

    await waitFor(() =>
      expect(
        screen.getByText("Promo Banner (Default) (Promo Banner)"),
      ).toBeInTheDocument(),
    );
    await user.click(screen.getByRole("button", { name: /clear/i }));
    expect(onChange).toHaveBeenCalledWith("");
  });

  it("falls back to the bare id while the outcome is unresolved", () => {
    const onChange = vi.fn();
    server.use(
      http.get(`${OUTCOMES_URL}/:id`, () =>
        HttpResponse.json(
          { error: { code: "NOT_FOUND", message: "gone" } },
          { status: 404 },
        ),
      ),
    );

    renderWithQuery(
      <SavedOutcomeSelectControl
        id="field-outcome"
        value="ghost-id"
        onChange={onChange}
      />,
    );

    expect(screen.getByText("ghost-id")).toBeInTheDocument();
  });
});
