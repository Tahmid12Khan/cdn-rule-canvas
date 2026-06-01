import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";

import { Pagination } from "@/components/ui/Pagination";

describe("Pagination", () => {
  it("renders the 'Results X–Y of Z' summary", () => {
    render(
      <Pagination page={2} pageSize={20} total={45} onPageChange={vi.fn()} />,
    );
    expect(screen.getByText("Results 21–40 of 45")).toBeInTheDocument();
  });

  it("emits the next page when Next is clicked", async () => {
    const user = userEvent.setup();
    const onPageChange = vi.fn();
    render(
      <Pagination
        page={1}
        pageSize={20}
        total={45}
        onPageChange={onPageChange}
      />,
    );

    await user.click(screen.getByRole("button", { name: /next/i }));
    expect(onPageChange).toHaveBeenCalledWith(2);
  });

  it("emits the previous page when Previous is clicked", async () => {
    const user = userEvent.setup();
    const onPageChange = vi.fn();
    render(
      <Pagination
        page={2}
        pageSize={20}
        total={45}
        onPageChange={onPageChange}
      />,
    );

    await user.click(screen.getByRole("button", { name: /previous/i }));
    expect(onPageChange).toHaveBeenCalledWith(1);
  });

  it("disables Previous on the first page and Next on the last page", () => {
    const { rerender } = render(
      <Pagination page={1} pageSize={20} total={45} onPageChange={vi.fn()} />,
    );
    expect(screen.getByRole("button", { name: /previous/i })).toBeDisabled();

    rerender(
      <Pagination page={3} pageSize={20} total={45} onPageChange={vi.fn()} />,
    );
    expect(screen.getByRole("button", { name: /next/i })).toBeDisabled();
  });
});
