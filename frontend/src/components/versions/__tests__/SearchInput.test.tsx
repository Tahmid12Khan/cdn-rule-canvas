import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";

import { SearchInput } from "@/components/versions/SearchInput";

describe("SearchInput", () => {
  it("debounces and reports the trimmed term", async () => {
    const user = userEvent.setup();
    const onSearch = vi.fn();
    render(<SearchInput onSearch={onSearch} />);

    // Initial mount fires once with the empty term.
    await waitFor(() => expect(onSearch).toHaveBeenCalledWith(""));
    onSearch.mockClear();

    await user.type(screen.getByLabelText(/search versions/i), "  Testing  ");

    await waitFor(() => expect(onSearch).toHaveBeenCalledWith("Testing"));
  });
});
