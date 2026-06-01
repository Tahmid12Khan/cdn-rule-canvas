import { render, screen, waitFor } from "@testing-library/react";
import type { ReactNode } from "react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import { OnboardingChecklist } from "@/components/onboarding/OnboardingChecklist";
import {
  ONBOARDING_STEPS,
  useOnboardingStore,
} from "@/state/onboardingStore";

vi.mock("next/link", () => ({
  default: ({ children, href }: { children: ReactNode; href: string }) => (
    <a href={href}>{children}</a>
  ),
}));

beforeEach(() => {
  useOnboardingStore.setState({ dismissed: false, completedSteps: [] });
});

describe("onboardingStore", () => {
  it("complete() is idempotent per step", () => {
    const s = useOnboardingStore.getState();
    s.complete("feature");
    s.complete("feature");
    expect(useOnboardingStore.getState().completedSteps).toEqual(["feature"]);
  });

  it("dismiss() flips the dismissed flag", () => {
    useOnboardingStore.getState().dismiss();
    expect(useOnboardingStore.getState().dismissed).toBe(true);
  });
});

describe("OnboardingChecklist", () => {
  it("renders the steps and ticks completed ones", async () => {
    useOnboardingStore.setState({
      dismissed: false,
      completedSteps: ["feature"],
    });
    render(<OnboardingChecklist />);
    await waitFor(() =>
      expect(screen.getByText("Getting started")).toBeInTheDocument(),
    );
    expect(screen.getByText("1 of 5 steps done")).toBeInTheDocument();
    expect(screen.getByText("Create a feature")).toBeInTheDocument();
  });

  it("hides when dismissed", async () => {
    useOnboardingStore.setState({ dismissed: true, completedSteps: [] });
    render(<OnboardingChecklist />);
    await waitFor(() => {
      expect(screen.queryByText("Getting started")).not.toBeInTheDocument();
    });
  });

  it("hides when all steps are complete", async () => {
    useOnboardingStore.setState({
      dismissed: false,
      completedSteps: [...ONBOARDING_STEPS],
    });
    render(<OnboardingChecklist />);
    await waitFor(() => {
      expect(screen.queryByText("Getting started")).not.toBeInTheDocument();
    });
  });
});
