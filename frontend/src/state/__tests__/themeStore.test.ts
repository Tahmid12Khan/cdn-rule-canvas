import { beforeEach, describe, expect, it } from "vitest";

import { useThemeStore } from "@/state/themeStore";

beforeEach(() => {
  // Reset to the default + clear any class the previous test applied.
  document.documentElement.classList.remove("light", "dark");
  useThemeStore.setState({ theme: "dark" });
});

describe("themeStore", () => {
  it("setTheme updates state and applies the matching class on <html>", () => {
    useThemeStore.getState().setTheme("light");
    expect(useThemeStore.getState().theme).toBe("light");
    const root = document.documentElement;
    expect(root.classList.contains("light")).toBe(true);
    expect(root.classList.contains("dark")).toBe(false);
  });

  it("toggle flips dark -> light -> dark and swaps the class each time", () => {
    const { toggle } = useThemeStore.getState();
    toggle();
    expect(useThemeStore.getState().theme).toBe("light");
    expect(document.documentElement.classList.contains("light")).toBe(true);

    toggle();
    expect(useThemeStore.getState().theme).toBe("dark");
    expect(document.documentElement.classList.contains("dark")).toBe(true);
    expect(document.documentElement.classList.contains("light")).toBe(false);
  });
});
