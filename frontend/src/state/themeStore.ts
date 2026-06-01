"use client";

// Light/dark theme state (Phase 1, Task A). A tiny Zustand store persisted to
// localStorage under a versioned key so the choice survives reloads. The setter
// imperatively flips the `.light` / `.dark` class on <html> so the whole app
// re-themes from the CSS-var semantic tokens (tailwind.config.ts) with zero
// per-component edits. A pre-paint inline script in layout.tsx reads the SAME
// localStorage shape before first paint to avoid FOUC / hydration mismatch.
import { create } from "zustand";
import { createJSONStorage, persist } from "zustand/middleware";

export type Theme = "light" | "dark";

interface ThemeState {
  theme: Theme;
  setTheme: (t: Theme) => void;
  toggle: () => void;
}

// Apply the chosen theme class to <html> (add it, remove the other). SSR-safe:
// no-op when document is unavailable.
function applyThemeClass(theme: Theme) {
  if (typeof document === "undefined") return;
  const root = document.documentElement;
  root.classList.add(theme);
  root.classList.remove(theme === "dark" ? "light" : "dark");
}

export const useThemeStore = create<ThemeState>()(
  persist(
    (set, get) => ({
      // Default mirrors the pre-paint script's final fallback. The persisted
      // value (if any) rehydrates over this; onRehydrateStorage re-applies the
      // class so store state and the DOM stay in sync after hydration.
      theme: "dark",
      setTheme: (t) => {
        applyThemeClass(t);
        set({ theme: t });
      },
      toggle: () => {
        const next: Theme = get().theme === "dark" ? "light" : "dark";
        applyThemeClass(next);
        set({ theme: next });
      },
    }),
    {
      name: "rre-theme-v1",
      storage: createJSONStorage(() => localStorage),
      onRehydrateStorage: () => (state) => {
        if (state) applyThemeClass(state.theme);
      },
    },
  ),
);
