import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it } from "vitest";

import { ThemeToggle } from "@/components/nav/ThemeToggle";
import { useThemeStore } from "@/state/themeStore";

beforeEach(() => {
  document.documentElement.classList.remove("light", "dark");
  useThemeStore.setState({ theme: "dark" });
});

describe("ThemeToggle", () => {
  it("labels itself for switching to light when in dark mode", async () => {
    render(<ThemeToggle />);
    expect(
      await screen.findByRole("button", { name: "Switch to light theme" }),
    ).toBeInTheDocument();
  });

  it("toggles the theme and updates its aria-label on click", async () => {
    const user = userEvent.setup();
    render(<ThemeToggle />);
    const btn = await screen.findByRole("button", {
      name: "Switch to light theme",
    });
    await user.click(btn);
    expect(useThemeStore.getState().theme).toBe("light");
    expect(
      screen.getByRole("button", { name: "Switch to dark theme" }),
    ).toBeInTheDocument();
  });
});
