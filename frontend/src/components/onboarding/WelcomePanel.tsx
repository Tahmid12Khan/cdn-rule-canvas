"use client";

// First-visit welcome panel on the landing page (WS5). Shown only on a fresh
// visit (onboarding not dismissed and no steps completed) to point new users at
// the 5-step happy path. Deferred behind a mounted flag to avoid an SSR
// hydration mismatch from the localStorage-backed store.
import { useEffect, useState } from "react";
import Link from "next/link";

import { useOnboardingStore } from "@/state/onboardingStore";

export function WelcomePanel() {
  const [mounted, setMounted] = useState(false);
  useEffect(() => setMounted(true), []);

  const dismissed = useOnboardingStore((s) => s.dismissed);
  const completedSteps = useOnboardingStore((s) => s.completedSteps);
  const dismiss = useOnboardingStore((s) => s.dismiss);

  if (!mounted) return null;
  if (dismissed || completedSteps.length > 0) return null;

  return (
    <section className="rounded-lg border border-brand-400 bg-brand-50 p-5">
      <h2 className="text-base font-semibold text-nav">
        Create your first rule in 5 steps
      </h2>
      <p className="mt-1 text-sm text-status-prevFg">
        Feature → Version → Edit canvas → Configure outcome → Save. Follow the
        checklist in the corner as you go.
      </p>
      <div className="mt-4 flex items-center gap-3">
        <Link
          href="/products/features"
          className="inline-flex items-center rounded-md bg-action-600 px-4 py-2 text-sm font-medium text-white hover:bg-action-700"
        >
          Start
        </Link>
        <button
          type="button"
          onClick={dismiss}
          className="text-sm font-medium text-status-prevFg hover:text-nav"
        >
          Skip tour
        </button>
      </div>
    </section>
  );
}
