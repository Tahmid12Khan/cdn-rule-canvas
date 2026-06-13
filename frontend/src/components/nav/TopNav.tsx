import Link from "next/link";

import { ThemeToggle } from "@/components/nav/ThemeToggle";

// Server component: static top navigation bar + theme toggle + tenant pill.
// ThemeToggle is the only client island; the bar itself stays server-rendered.
export function TopNav() {
  return (
    <nav className="flex h-14 items-center justify-between border-b border-nav-border bg-nav-bg px-6">
      <div className="flex items-center gap-3">
        <Link
          href="/"
          className="flex items-center gap-2 text-nav-fg hover:text-fg"
        >
          <span className="h-2.5 w-2.5 rounded-sm bg-brand-500" aria-hidden />
          <span className="text-sm font-semibold tracking-tight">RRE</span>
        </Link>
        <span className="text-nav-border" aria-hidden>
          /
        </span>
        <span className="text-sm text-nav-muted">Response Rule Engine</span>
        <span className="text-nav-border" aria-hidden>
          /
        </span>
        <Link
          href="/products/features"
          className="text-sm font-medium text-nav-muted hover:text-fg"
        >
          Features
        </Link>
        <Link
          href="/products/sites"
          className="text-sm font-medium text-nav-muted hover:text-fg"
        >
          Sites
        </Link>
        <Link
          href="/products/components"
          className="text-sm font-medium text-nav-muted hover:text-fg"
        >
          Components
        </Link>
        <Link
          href="/products/test-presets"
          className="text-sm font-medium text-nav-muted hover:text-fg"
        >
          Test Presets
        </Link>
        <Link
          href="/products/test-full-journey"
          className="text-sm font-medium text-nav-muted hover:text-fg"
        >
          Test Full Journey
        </Link>
      </div>
      <div className="flex items-center gap-3">
        <ThemeToggle />
        <span className="rounded-full border border-nav-border px-3 py-1 text-xs font-medium text-nav-fg">
          Demo Tenant
        </span>
      </div>
    </nav>
  );
}
