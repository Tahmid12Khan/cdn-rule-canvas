"use client";

// Client component: renders an error message with an optional retry action.
// Accepts EITHER a flat `message: string` (back-compat) OR a structured
// `error: UserError` (W4) which renders a bold title + why + how-to-fix lines.
import type { UserError } from "@/lib/errors/userError";

interface ErrorBannerMessageProps {
  message: string;
  error?: undefined;
  onRetry?: () => void;
}

interface ErrorBannerUserErrorProps {
  error: UserError;
  message?: undefined;
  onRetry?: () => void;
}

type ErrorBannerProps = ErrorBannerMessageProps | ErrorBannerUserErrorProps;

export function ErrorBanner({ message, error, onRetry }: ErrorBannerProps) {
  // Only show Retry for a flat message, or for a UserError flagged retryable.
  const showRetry = Boolean(onRetry) && (error ? error.retryable : true);

  return (
    <div
      role="alert"
      className="flex items-start justify-between gap-4 rounded-lg border border-danger bg-danger-bg px-4 py-3 text-sm text-danger"
    >
      {error ? (
        <div className="space-y-1">
          <p className="font-semibold">{error.title}</p>
          <p className="text-danger/90">{error.why}</p>
          <p className="text-danger/80">
            <span className="font-medium">How to fix:</span> {error.howToFix}
          </p>
        </div>
      ) : (
        <span>{message}</span>
      )}
      {showRetry && onRetry && (
        <button
          type="button"
          onClick={onRetry}
          className="shrink-0 rounded-md border border-danger px-3 py-1 font-medium hover:bg-danger-bg"
        >
          Retry
        </button>
      )}
    </div>
  );
}
