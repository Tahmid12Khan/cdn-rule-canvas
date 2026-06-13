"use client";

// Live preview of a Component template (design §5.3 / §6). Renders the mustache
// `html` with the supplied `values` client-side via mustache.render, then pipes
// the result through <SafeHtml> — the ONLY sanctioned dangerouslySetInnerHTML
// sink (FRONTEND CONTRACT §0/§8). The actual request-time render happens in the
// proxy; this is an author-facing approximation using mustache's flat
// interpolation (no sections/partials), matching the flat variable model.
//
// Reused in the editor (sample/placeholder values = each variable's title) and
// later by the rule-node sub-form (entered values).
import { useMemo } from "react";
import Mustache from "mustache";

import { SafeHtml } from "@/components/ui/SafeHtml";

interface ComponentPreviewProps {
  html: string;
  // name → value. Missing keys render empty (mustache default).
  values: Record<string, string>;
  className?: string;
}

export function ComponentPreview({
  html,
  values,
  className,
}: ComponentPreviewProps) {
  const rendered = useMemo(() => {
    try {
      // Disable HTML-escaping of the template tags themselves is NOT needed;
      // mustache escapes interpolated VALUES by default which is desirable for
      // untrusted sample text, and SafeHtml sanitizes the final markup anyway.
      return Mustache.render(html, values);
    } catch {
      // A malformed template (e.g. unbalanced braces) throws — fall back to the
      // raw body so the preview never crashes the editor. Lint surfaces the
      // underlying issue separately.
      return html;
    }
  }, [html, values]);

  return <SafeHtml html={rendered} className={className} />;
}
