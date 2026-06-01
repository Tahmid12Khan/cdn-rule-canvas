"use client";

// First-run onboarding checklist (WS5). A small dismissible card pinned
// bottom-right that teaches the happy path. Steps tick automatically off real
// mutation-success signals stored in the onboarding store. Rendered only after
// a mounted flag so the localStorage-backed store doesn't cause an SSR
// hydration mismatch. Hidden once dismissed or all steps complete.
import { useEffect, useState } from "react";
import Link from "next/link";

import {
  ONBOARDING_STEPS,
  useOnboardingStore,
  type OnboardingStep,
} from "@/state/onboardingStore";

interface StepCopy {
  step: OnboardingStep;
  label: string;
}

const STEP_COPY: StepCopy[] = [
  { step: "feature", label: "Create a feature" },
  { step: "version", label: "Add a draft version" },
  { step: "edit", label: "Enter edit mode on the canvas" },
  { step: "outcome", label: "Configure an outcome" },
  { step: "save", label: "Save your rules" },
];

export function OnboardingChecklist() {
  const [mounted, setMounted] = useState(false);
  useEffect(() => setMounted(true), []);

  const dismissed = useOnboardingStore((s) => s.dismissed);
  const completedSteps = useOnboardingStore((s) => s.completedSteps);
  const dismiss = useOnboardingStore((s) => s.dismiss);

  if (!mounted) return null;
  const allDone = ONBOARDING_STEPS.every((s) => completedSteps.includes(s));
  if (dismissed || allDone) return null;

  const done = (step: OnboardingStep) => completedSteps.includes(step);
  const currentStep = STEP_COPY.find((s) => !done(s.step));

  return (
    <aside
      aria-label="Getting started"
      className="fixed bottom-4 right-4 z-40 w-72 rounded-lg border border-status-prevBg bg-bg-elevated p-4 shadow-lg"
    >
      <div className="flex items-start justify-between">
        <div>
          <p className="text-sm font-semibold text-nav">Getting started</p>
          <p className="text-xs text-status-prevFg">
            {completedSteps.length} of {ONBOARDING_STEPS.length} steps done
          </p>
        </div>
        <button
          type="button"
          aria-label="Dismiss getting started"
          onClick={dismiss}
          className="rounded p-1 text-status-prev hover:bg-status-prevBg"
        >
          <span aria-hidden className="text-lg leading-none">
            ×
          </span>
        </button>
      </div>

      <ol className="mt-3 space-y-2">
        {STEP_COPY.map(({ step, label }) => {
          const isDone = done(step);
          const isCurrent = currentStep?.step === step;
          return (
            <li
              key={step}
              className={[
                "flex items-center gap-2 text-sm",
                isDone
                  ? "text-status-prev line-through"
                  : isCurrent
                    ? "font-medium text-nav"
                    : "text-status-prevFg",
              ].join(" ")}
            >
              <span
                aria-hidden
                className={[
                  "flex h-4 w-4 shrink-0 items-center justify-center rounded-full border text-[10px]",
                  isDone
                    ? "border-brand-500 bg-brand-500 text-white"
                    : "border-status-prevBg",
                ].join(" ")}
              >
                {isDone ? "✓" : ""}
              </span>
              {label}
            </li>
          );
        })}
      </ol>

      {currentStep?.step === "feature" && (
        <Link
          href="/products/features"
          className="mt-3 inline-flex w-full items-center justify-center rounded-md bg-action-600 px-3 py-1.5 text-sm font-medium text-white hover:bg-action-700"
        >
          Go to Features
        </Link>
      )}
    </aside>
  );
}
