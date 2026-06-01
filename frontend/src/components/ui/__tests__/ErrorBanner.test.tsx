import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";

import { ErrorBanner } from "@/components/ui/ErrorBanner";
import type { UserError } from "@/lib/errors/userError";

describe("ErrorBanner", () => {
  it("renders a flat message (back-compat) with a Retry button", async () => {
    const user = userEvent.setup();
    const onRetry = vi.fn();
    render(<ErrorBanner message="Boom" onRetry={onRetry} />);

    expect(screen.getByRole("alert")).toHaveTextContent("Boom");
    await user.click(screen.getByRole("button", { name: /retry/i }));
    expect(onRetry).toHaveBeenCalledOnce();
  });

  it("renders a UserError's title, why, and how-to-fix lines", () => {
    const error: UserError = {
      title: "That slug is already taken",
      why: "Another item already uses this slug.",
      howToFix: "Choose a different, unique slug.",
      retryable: false,
    };
    render(<ErrorBanner error={error} />);

    const alert = screen.getByRole("alert");
    expect(alert).toHaveTextContent("That slug is already taken");
    expect(alert).toHaveTextContent("Another item already uses this slug.");
    expect(alert).toHaveTextContent(/how to fix/i);
    expect(alert).toHaveTextContent("Choose a different, unique slug.");
  });

  it("hides Retry for a non-retryable UserError even when onRetry is passed", () => {
    const error: UserError = {
      title: "Locked",
      why: "Published.",
      howToFix: "Save as new.",
      retryable: false,
    };
    render(<ErrorBanner error={error} onRetry={() => {}} />);
    expect(
      screen.queryByRole("button", { name: /retry/i }),
    ).not.toBeInTheDocument();
  });

  it("shows Retry for a retryable UserError with onRetry", async () => {
    const user = userEvent.setup();
    const onRetry = vi.fn();
    const error: UserError = {
      title: "Server error",
      why: "5xx.",
      howToFix: "Try again.",
      retryable: true,
    };
    render(<ErrorBanner error={error} onRetry={onRetry} />);
    await user.click(screen.getByRole("button", { name: /retry/i }));
    expect(onRetry).toHaveBeenCalledOnce();
  });
});
