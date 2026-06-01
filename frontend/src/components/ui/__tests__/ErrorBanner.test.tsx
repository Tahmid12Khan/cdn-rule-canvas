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

  it("does not render the full-response accordion without rawResponse", () => {
    render(<ErrorBanner message="Boom" />);
    expect(
      screen.queryByText(/show full server response/i),
    ).not.toBeInTheDocument();
  });

  it("renders the raw server response in a collapsible accordion", () => {
    render(
      <ErrorBanner
        message="Boom"
        rawResponse={'{"error":{"code":"INTERNAL_ERROR"}}'}
      />,
    );
    expect(
      screen.getByText(/show full server response/i),
    ).toBeInTheDocument();
    expect(screen.getByText(/INTERNAL_ERROR/)).toBeInTheDocument();
  });

  it("truncates a long body to ~1000 words then shows a Show full toggle", async () => {
    const user = userEvent.setup();
    // 1500 distinct words so the first 1000 are kept, the rest dropped.
    const words = Array.from({ length: 1500 }, (_, i) => `w${i}`);
    const raw = words.join(" ");
    render(<ErrorBanner message="Boom" rawResponse={raw} />);

    const pre = screen.getByText(/^w0 /);
    // Word 999 is the last kept; word 1000+ is truncated away.
    expect(pre.textContent).toContain("w999");
    expect(pre.textContent).not.toContain("w1000");
    expect(pre.textContent).toContain("…");

    // The "Show full" toggle expands to the untruncated body.
    await user.click(screen.getByRole("button", { name: /show full/i }));
    expect(screen.getByText(/w1000/)).toBeInTheDocument();
  });
});
