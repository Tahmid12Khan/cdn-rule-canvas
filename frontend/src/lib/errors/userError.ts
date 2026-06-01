// UserError system (Phase 2, W4). Turns raw API / validation failures into a
// human-readable { title, why, howToFix } triple so the UI never surfaces a
// raw backend string or a bare generic fallback. Mapping is keyed on the
// ApiError.code / HTTP status, with a sensible generic fallthrough for unknown
// codes and a dedicated branch for network/TypeError (fetch couldn't reach the
// server).
import type { ZodError } from "zod";

import { ApiError } from "@/lib/api/client";

export interface UserError {
  /** Short, bold headline (what went wrong). */
  title: string;
  /** One line explaining WHY it happened. */
  why: string;
  /** One line telling the user HOW to fix / what to do next. */
  howToFix: string;
  /** Optional form field this error attaches to (for inline field surfacing). */
  fieldPath?: string;
  /** True when a Retry button is worth offering. */
  retryable?: boolean;
}

/** The surface the error occurred on — lets us tailor copy a little. */
export type ErrorSurface =
  | "load"
  | "save"
  | "create"
  | "eval"
  | "delete"
  | "publish";

interface ToUserErrorCtx {
  surface?: ErrorSurface;
}

// A fetch that never reached the server throws a TypeError ("Failed to fetch")
// rather than an ApiError. Detect it so we can tell the user to check the
// backend/proxy is running rather than showing "Failed to fetch".
function isNetworkError(err: unknown): boolean {
  if (err instanceof ApiError) return false;
  if (err instanceof TypeError) return true;
  if (err instanceof Error) {
    return /failed to fetch|network|load failed|fetch/i.test(err.message);
  }
  return false;
}

function surfaceVerb(surface: ErrorSurface | undefined): string {
  switch (surface) {
    case "load":
      return "load this";
    case "save":
      return "save your changes";
    case "create":
      return "create this";
    case "eval":
      return "run the test";
    case "delete":
      return "delete this";
    case "publish":
      return "publish this version";
    default:
      return "complete that action";
  }
}

export function toUserError(err: unknown, ctx?: ToUserErrorCtx): UserError {
  const surface = ctx?.surface;

  // 1. Network / unreachable server (no HTTP response at all).
  if (isNetworkError(err)) {
    if (surface === "eval") {
      return {
        title: "Could not reach the evaluator",
        why: "The request to the proxy didn't get a response.",
        howToFix: "Make sure the proxy service is running, then run the test again.",
        retryable: true,
      };
    }
    return {
      title: "Can't reach the server",
      why:
        "The request didn't get a response, so the app couldn't " +
        `${surfaceVerb(surface)}.`,
      howToFix:
        "Check that the backend is running, then retry.",
      retryable: true,
    };
  }

  if (err instanceof ApiError) {
    // 2. Validation errors (422 or VALIDATION_ERROR).
    if (err.isValidation) {
      const fields = (err.details ?? [])
        .map((d) => d.loc)
        .filter(Boolean);
      return {
        title: "Some fields need attention",
        why:
          fields.length > 0
            ? "The server rejected the request because some values are invalid."
            : "The server couldn't accept the request as submitted.",
        howToFix:
          "Fix the highlighted fields and try again.",
        retryable: false,
      };
    }

    switch (err.code) {
      case "SLUG_CONFLICT":
        return {
          title: "That slug is already taken",
          why: "Another item already uses this slug, and slugs must be unique.",
          howToFix:
            "Choose a different, unique slug (lowercase kebab-case).",
          fieldPath: "id",
          retryable: false,
        };
      case "VERSION_EDIT_LOCKED":
        return {
          title: "This version is locked",
          why: "Published versions can't be edited directly to protect what's live.",
          howToFix:
            "Use 'Save as New Version' to capture your changes in an editable draft.",
          retryable: false,
        };
      default:
        break;
    }

    // 3. 409 conflict (not a known code) — usually an invalid status transition.
    if (err.status === 409) {
      if (surface === "publish") {
        return {
          title: "This version is already live",
          why: "You can only publish a version that isn't already the live one.",
          howToFix:
            "Pick a different version to publish, or unpublish the current live version first.",
          retryable: false,
        };
      }
      return {
        title: "This version is locked",
        why: "The version's status doesn't allow this change.",
        howToFix:
          "Use 'Save as New Version' to edit, or refresh to see the latest status.",
        retryable: false,
      };
    }

    // 4. Server errors (5xx / INTERNAL_ERROR).
    if (err.status >= 500 || err.code === "INTERNAL_ERROR") {
      return {
        title: "Something went wrong on the server",
        why: `The server hit an error while trying to ${surfaceVerb(surface)}.`,
        howToFix:
          "Try again in a moment. If it keeps happening, contact support.",
        retryable: true,
      };
    }

    // 5. Other 4xx with a message — show it as the why, but still scaffold it.
    return {
      title: "We couldn't complete that",
      why: err.message || `The server declined the request to ${surfaceVerb(surface)}.`,
      howToFix: "Review your input and try again.",
      retryable: err.status === 408 || err.status === 429,
    };
  }

  // 6. Unknown — never leak a stack string.
  return {
    title: "Something went wrong",
    why: `The app couldn't ${surfaceVerb(surface)}.`,
    howToFix: "Try again. If the problem persists, contact support.",
    retryable: true,
  };
}

/**
 * Maps a ZodError to per-field messages keyed by the field path, each enriched
 * with an optional `howToFix` remediation line. The base message is the zod
 * `issue.message` (we author friendly, actionable messages in the schemas).
 */
export function zodToUserErrors(
  error: ZodError,
): Record<string, { message: string; howToFix?: string }> {
  const out: Record<string, { message: string; howToFix?: string }> = {};
  for (const issue of error.issues) {
    const key = issue.path[0];
    if (key === undefined) continue;
    const field = String(key);
    if (out[field]) continue; // keep the first issue per field
    out[field] = { message: issue.message };
  }
  return out;
}
