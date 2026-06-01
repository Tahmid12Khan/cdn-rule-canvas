"use client";

// First-run onboarding state (WS5). A tiny Zustand store persisted to
// localStorage so the dismissible "Getting started" checklist survives reloads.
// Steps auto-advance off real mutation-success signals (no manual tracking).
// The localStorage key is versioned so flow changes don't strand old users.
import { create } from "zustand";
import { createJSONStorage, persist } from "zustand/middleware";

export type OnboardingStep =
  | "feature"
  | "version"
  | "edit"
  | "outcome"
  | "save";

export const ONBOARDING_STEPS: OnboardingStep[] = [
  "feature",
  "version",
  "edit",
  "outcome",
  "save",
];

interface OnboardingState {
  dismissed: boolean;
  completedSteps: OnboardingStep[];
  complete: (step: OnboardingStep) => void;
  dismiss: () => void;
  reset: () => void;
}

export const useOnboardingStore = create<OnboardingState>()(
  persist(
    (set, get) => ({
      dismissed: false,
      completedSteps: [],
      complete: (step) => {
        if (get().completedSteps.includes(step)) return;
        set((s) => ({ completedSteps: [...s.completedSteps, step] }));
      },
      dismiss: () => set({ dismissed: true }),
      reset: () => set({ dismissed: false, completedSteps: [] }),
    }),
    {
      name: "rre-onboarding-v1",
      storage: createJSONStorage(() => localStorage),
    },
  ),
);
