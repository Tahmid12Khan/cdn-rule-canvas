"use client";

// Shared button primitive (WS6). Collapses the app's ad-hoc inline button
// styles into three token-driven variants so there is ONE accent (teal) plus a
// neutral secondary — the Zed flat look. Forwards all native button props.
import { forwardRef, type ButtonHTMLAttributes } from "react";
import clsx from "clsx";

type Variant = "primary" | "secondary" | "ghost";

interface ButtonProps extends ButtonHTMLAttributes<HTMLButtonElement> {
  variant?: Variant;
}

const VARIANTS: Record<Variant, string> = {
  primary: "bg-accent text-accent-fg hover:opacity-90",
  secondary:
    "border border-border bg-transparent text-fg hover:bg-bg-overlay",
  ghost: "bg-transparent text-fg-muted hover:bg-bg-overlay hover:text-fg",
};

export const Button = forwardRef<HTMLButtonElement, ButtonProps>(
  function Button({ variant = "primary", className, type, ...rest }, ref) {
    return (
      <button
        ref={ref}
        type={type ?? "button"}
        className={clsx(
          "rounded-md px-4 py-2 text-sm font-semibold transition-colors disabled:cursor-not-allowed disabled:opacity-50",
          VARIANTS[variant],
          className,
        )}
        {...rest}
      />
    );
  },
);
