"use client";

// Warns the user before leaving the page with unsaved canvas edits (Task 14).
// Covers the browser `beforeunload` path. (Next.js App Router does not expose a
// stable router-intercept API for client navigations in 14.x, so we guard the
// hard-navigation/reload/close path here; in-app navigation is rare from the
// builder and the SaveBar status keeps dirty state visible.)
import { useEffect } from "react";

import { useRuleBuilderStore } from "@/state/ruleBuilderStore";

export function UnsavedChangesGuard() {
  const dirty = useRuleBuilderStore((s) => s.dirty);

  useEffect(() => {
    if (!dirty) return undefined;
    const handler = (e: BeforeUnloadEvent) => {
      e.preventDefault();
      e.returnValue = "";
    };
    window.addEventListener("beforeunload", handler);
    return () => window.removeEventListener("beforeunload", handler);
  }, [dirty]);

  return null;
}
