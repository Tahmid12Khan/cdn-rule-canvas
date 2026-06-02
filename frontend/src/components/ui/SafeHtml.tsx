"use client";

// SECURITY REVIEW REQUIRED.
// This is the ONLY component permitted to call dangerouslySetInnerHTML. All
// untrusted HTML (e.g. component `html_body` previews) MUST flow through here
// so it is sanitized by DOMPurify first. Never call dangerouslySetInnerHTML
// directly elsewhere (FRONTEND CONTRACT §0 / §8).
import { useMemo } from "react";
import DOMPurify from "dompurify";

interface SafeHtmlProps {
  html: string;
  className?: string;
}

export function SafeHtml({ html, className }: SafeHtmlProps) {
  const clean = useMemo(() => DOMPurify.sanitize(html), [html]);
  return (
    <div
      className={className}
      // Sanitized by DOMPurify above; this is the sole sanctioned HTML sink.
      dangerouslySetInnerHTML={{ __html: clean }}
    />
  );
}
