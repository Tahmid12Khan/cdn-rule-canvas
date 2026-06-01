"use client";

// Client component: derives the breadcrumb trail from usePathname (browser
// routing API). Mounted once in the root layout (FRONTEND CONTRACT §1).
import { Fragment } from "react";
import Link from "next/link";
import { usePathname } from "next/navigation";

interface Crumb {
  label: string;
  href: string;
}

// Maps a URL path into a human-readable breadcrumb trail. Leaf pages may
// refine labels (feature/version names) from the Query cache; this default
// derivation keeps the trail usable everywhere.
function deriveCrumbs(pathname: string): Crumb[] {
  if (pathname === "/" || pathname === "") return [];

  const segments = pathname.split("/").filter(Boolean);
  const crumbs: Crumb[] = [];
  let href = "";

  for (let i = 0; i < segments.length; i += 1) {
    const segment = segments[i];
    href += `/${segment}`;

    // `products/features/{type}/{slug}/{vnum}/transformation/{outcomeId}`
    let label = segment;
    if (segment === "products") label = "Products";
    else if (segment === "features") label = "Features";
    else if (segment === "transformation") label = "Edit Outcome";
    else if (segments[i - 1] === "features" && i >= 2) label = segment; // type segment (html/json)

    crumbs.push({ label, href });
  }

  return crumbs;
}

export function Breadcrumbs() {
  const pathname = usePathname();
  const crumbs = deriveCrumbs(pathname);

  if (crumbs.length === 0) return null;

  return (
    <nav
      aria-label="Breadcrumb"
      className="border-b border-status-prevBg bg-bg-elevated px-6 py-2 text-sm text-status-prevFg"
    >
      <ol className="flex flex-wrap items-center gap-2">
        {crumbs.map((crumb, idx) => {
          const isLast = idx === crumbs.length - 1;
          return (
            <Fragment key={crumb.href}>
              <li>
                {isLast ? (
                  <span className="font-medium text-nav" aria-current="page">
                    {crumb.label}
                  </span>
                ) : (
                  <Link href={crumb.href} className="hover:text-brand-600">
                    {crumb.label}
                  </Link>
                )}
              </li>
              {!isLast && (
                <li aria-hidden className="text-status-prev">
                  /
                </li>
              )}
            </Fragment>
          );
        })}
      </ol>
    </nav>
  );
}
