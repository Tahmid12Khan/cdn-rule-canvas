// Banner shown above the canvas for a non-draft version (Task 14 / Phase 2 W1).
// Two variants:
//   - "readonly" (default): the version is published; edits aren't possible
//     until the user clicks Edit (which enters a LOCAL edit mode).
//   - "local": the user is editing a published version locally — changes are
//     NOT saved to the server and only "Save as New Version" persists them.
//     Rendered in an unmistakable amber/warning tone.
// Presentational (server-safe).

interface ReadOnlyBannerProps {
  variant?: "readonly" | "local";
}

export function ReadOnlyBanner({ variant = "readonly" }: ReadOnlyBannerProps) {
  if (variant === "local") {
    return (
      <div
        role="status"
        className="flex items-start gap-2 rounded-md border border-status-stagingFg bg-status-stagingBg px-4 py-2.5 text-sm font-medium text-status-stagingFg"
      >
        <svg
          aria-hidden
          viewBox="0 0 24 24"
          fill="none"
          stroke="currentColor"
          strokeWidth="2"
          strokeLinecap="round"
          strokeLinejoin="round"
          className="mt-0.5 h-4 w-4 shrink-0"
        >
          <path d="M10.29 3.86 1.82 18a2 2 0 0 0 1.71 3h16.94a2 2 0 0 0 1.71-3L13.71 3.86a2 2 0 0 0-3.42 0Z" />
          <path d="M12 9v4" />
          <path d="M12 17h.01" />
        </svg>
        <span>
          Editing locally — changes are{" "}
          <span className="font-bold underline">NOT saved</span> to the server.
          Use &ldquo;Save as New Version&rdquo; to persist.
        </span>
      </div>
    );
  }

  return (
    <div
      role="status"
      className="flex items-center gap-2 rounded-md border border-status-stagingBg bg-status-stagingBg/40 px-4 py-2 text-sm font-medium text-status-stagingFg"
    >
      <svg
        aria-hidden
        viewBox="0 0 24 24"
        fill="none"
        stroke="currentColor"
        strokeWidth="2"
        className="h-4 w-4"
      >
        <rect x="5" y="11" width="14" height="9" rx="2" />
        <path d="M8 11V7a4 4 0 1 1 8 0v4" strokeLinecap="round" />
      </svg>
      Read-only — click Edit to make local changes, then Save as New Version.
    </div>
  );
}
