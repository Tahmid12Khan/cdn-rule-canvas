"use client";

// Client component: renders an error message with an optional retry action.
// Accepts EITHER a flat `message: string` (back-compat) OR a structured
// `error: UserError` (W4) which renders a bold title + why + how-to-fix lines.
//
// FULL SERVER RESPONSE (req 3): when `rawResponse` is provided (the raw 422/error
// body from ApiError.rawBody) the banner renders a collapsed <details> accordion
// below the message showing the body truncated to the first ~1000 words (also
// char-capped so a huge minified JSON line can't jank the DOM). A "Show full"
// toggle swaps to the untruncated body. The raw text is rendered inside a <pre>
// as TEXT (never dangerouslySetInnerHTML) — no XSS surface, no sanitizer needed.
import { useState } from "react";

import type { UserError } from "@/lib/errors/userError";

interface ErrorBannerMessageProps {
  message: string;
  error?: undefined;
  onRetry?: () => void;
  rawResponse?: string;
}

interface ErrorBannerUserErrorProps {
  error: UserError;
  message?: undefined;
  onRetry?: () => void;
  rawResponse?: string;
}

type ErrorBannerProps = ErrorBannerMessageProps | ErrorBannerUserErrorProps;

const MAX_WORDS = 1000;
const MAX_CHARS = 8000;

// Truncate to the first ~1000 words, then "…". A char cap guards against a huge
// minified JSON line (one giant "word") rendering a multi-MB string.
function truncate(raw: string): { text: string; truncated: boolean } {
  const words = raw.split(/\s+/);
  let text = raw;
  let truncated = false;
  if (words.length > MAX_WORDS) {
    text = words.slice(0, MAX_WORDS).join(" ");
    truncated = true;
  }
  if (text.length > MAX_CHARS) {
    text = text.slice(0, MAX_CHARS);
    truncated = true;
  }
  return { text: truncated ? `${text} …` : text, truncated };
}

export function ErrorBanner({
  message,
  error,
  onRetry,
  rawResponse,
}: ErrorBannerProps) {
  // Only show Retry for a flat message, or for a UserError flagged retryable.
  const showRetry = Boolean(onRetry) && (error ? error.retryable : true);
  const [showFull, setShowFull] = useState(false);

  const trimmedRaw = rawResponse?.trim();
  const hasRaw = Boolean(trimmedRaw);
  const { text: shown, truncated } = trimmedRaw
    ? truncate(trimmedRaw)
    : { text: "", truncated: false };

  return (
    <div
      role="alert"
      className="flex flex-col gap-3 rounded-lg border border-danger bg-danger-bg px-4 py-3 text-sm text-danger"
    >
      <div className="flex items-start justify-between gap-4">
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

      {hasRaw && (
        <details className="rounded-md border border-danger/40 bg-bg-elevated/40">
          <summary className="cursor-pointer select-none px-3 py-2 text-xs font-medium text-danger/90">
            Show full server response
          </summary>
          <div className="border-t border-danger/30 px-3 py-2">
            <pre className="max-h-64 overflow-auto whitespace-pre-wrap break-words text-xs text-danger/90">
              {showFull && trimmedRaw ? trimmedRaw : shown}
            </pre>
            {truncated && (
              <button
                type="button"
                onClick={() => setShowFull((v) => !v)}
                className="mt-2 text-xs font-medium text-danger underline hover:no-underline"
              >
                {showFull ? "Show less" : "Show full"}
              </button>
            )}
          </div>
        </details>
      )}
    </div>
  );
}
