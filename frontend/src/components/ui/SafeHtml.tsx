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

// Force rel="noopener noreferrer" on any anchor that opens a new context, so a
// target (especially target="_blank") can't reverse-tabnab the opener window.
// Registered once at module load; DOMPurify hooks are global.
DOMPurify.addHook("afterSanitizeAttributes", (node) => {
  if (node.tagName === "A" && node.hasAttribute("target")) {
    node.setAttribute("rel", "noopener noreferrer");
  }
});

// Explicit, hardened config: forbid scriptable sinks and keep the default
// (non-widened) allow-list. URI schemes are restricted so javascript:/data:
// hrefs can't slip through. `target` is the ONLY allow-list addition — needed
// so legitimate new-tab links survive, but the afterSanitizeAttributes hook
// above pins rel="noopener noreferrer" on them to block reverse tabnabbing.
const PURIFY_CONFIG = {
  ADD_ATTR: ["target"],
  FORBID_TAGS: ["style"],
  FORBID_ATTR: ["style", "srcset"],
  ALLOW_DATA_ATTR: false,
  ALLOWED_URI_REGEXP:
    /^(?:(?:https?|mailto|tel):|[^a-z]|[a-z+.-]+(?:[^a-z+.\-:]|$))/i,
};

export function SafeHtml({ html, className }: SafeHtmlProps) {
  const clean = useMemo(
    () => DOMPurify.sanitize(html, PURIFY_CONFIG),
    [html],
  );
  return (
    <div
      className={className}
      // Sanitized by DOMPurify above; this is the sole sanctioned HTML sink.
      dangerouslySetInnerHTML={{ __html: clean }}
    />
  );
}
