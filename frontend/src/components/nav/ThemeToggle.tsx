"use client";

// Light/dark theme toggle (Phase 1, Task A). A two-state icon button living in
// the TopNav right-hand cluster. Reads/writes themeStore (which also flips the
// `.light`/`.dark` class on <html>). Renders a sun in dark mode (click → light)
// and a moon in light mode (click → dark). Inline SVG — no new dependencies.
//
// Hydration: the persisted theme rehydrates after mount, but the pre-paint
// script in layout.tsx already set the <html> class, so there is no visual
// flash. We render the dark-mode icon on the server/first client paint and let
// the effect sync the real value to avoid a hydration mismatch on the button.
import { useEffect, useState } from "react";

import { useThemeStore } from "@/state/themeStore";

export function ThemeToggle() {
  const theme = useThemeStore((s) => s.theme);
  const toggle = useThemeStore((s) => s.toggle);

  // Until mounted, render a stable placeholder icon to match SSR output (the
  // persisted store value is not available on the server).
  const [mounted, setMounted] = useState(false);
  useEffect(() => setMounted(true), []);

  const isDark = !mounted || theme === "dark";
  const label = isDark ? "Switch to light theme" : "Switch to dark theme";

  return (
    <button
      type="button"
      onClick={toggle}
      aria-label={label}
      title={label}
      className="flex h-8 w-8 items-center justify-center rounded-md text-fg-muted transition-colors hover:bg-bg-overlay hover:text-fg focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-accent"
    >
      {isDark ? (
        // Sun
        <svg
          width="18"
          height="18"
          viewBox="0 0 24 24"
          fill="none"
          stroke="currentColor"
          strokeWidth="2"
          strokeLinecap="round"
          strokeLinejoin="round"
          aria-hidden="true"
        >
          <circle cx="12" cy="12" r="4" />
          <path d="M12 2v2M12 20v2M4.93 4.93l1.41 1.41M17.66 17.66l1.41 1.41M2 12h2M20 12h2M6.34 17.66l-1.41 1.41M19.07 4.93l-1.41 1.41" />
        </svg>
      ) : (
        // Moon
        <svg
          width="18"
          height="18"
          viewBox="0 0 24 24"
          fill="none"
          stroke="currentColor"
          strokeWidth="2"
          strokeLinecap="round"
          strokeLinejoin="round"
          aria-hidden="true"
        >
          <path d="M21 12.79A9 9 0 1 1 11.21 3 7 7 0 0 0 21 12.79z" />
        </svg>
      )}
    </button>
  );
}
