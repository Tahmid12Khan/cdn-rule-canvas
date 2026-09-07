"use client";

// Live preview of a Component template (design §5.3 / §6). Renders the mustache
// `html` with the supplied `values` client-side via mustache.render, then loads
// the result into a srcdoc <iframe> so it renders the way it will on the real
// site: <script> tags execute, remote stylesheets load, and custom elements can
// attach their shadow roots. The actual request-time render happens in the
// proxy; this is an author-facing approximation using mustache's flat
// interpolation (no sections/partials), matching the flat variable model.
//
// Why an iframe and not <SafeHtml>: DOMPurify strips <script>/<link>/<meta>, so
// script-driven components sanitized down to nothing renderable — e.g. the DN
// sub-paywall templates ship an empty #sub-paywall-container plus a module
// bundle that fills it, which previewed as a blank box. This is an admin-only
// authoring surface and the HTML comes from operators, so the preview runs it
// as-is; the iframe is what keeps it out of the admin document (own document,
// own CSS, no top-level navigation) instead of neutering the markup.
//
// Reused in the editor (sample/placeholder values = each variable's title) and
// by the rule-node sub-form (entered values).
import { useEffect, useMemo, useRef, useState } from "react";
import Mustache from "mustache";

interface ComponentPreviewProps {
  html: string;
  // name → value. Missing keys render empty (mustache default).
  values: Record<string, string>;
  className?: string;
}

// Everything except allow-top-navigation: a preview that can navigate the admin
// tab away is a footgun, never a feature.
const SANDBOX = [
  "allow-scripts",
  "allow-same-origin",
  "allow-forms",
  "allow-modals",
  "allow-popups",
  "allow-popups-to-escape-sandbox",
  "allow-downloads",
].join(" ");

// The component's own CSS owns the look inside the frame; give it the white
// page it expects rather than the admin panel's dark background.
const DOC_HEAD =
  '<meta charset="utf-8"><base target="_blank">' +
  "<style>html,body{margin:0;background:#fff}</style>";

const MIN_HEIGHT = 160;

// Reloading a component's remote scripts on every keystroke in a variable field
// is wasteful; let typing settle first.
const RELOAD_DEBOUNCE_MS = 300;

function wrap(body: string): string {
  return `<!doctype html><html><head>${DOC_HEAD}</head><body>${body}</body></html>`;
}

export function ComponentPreview({
  html,
  values,
  className,
}: ComponentPreviewProps) {
  const next = useMemo(() => {
    try {
      // mustache escapes interpolated VALUES by default, which is what we want
      // for author-entered text landing inside markup.
      return wrap(Mustache.render(html, values));
    } catch {
      // A malformed template (e.g. unbalanced braces) throws — fall back to the
      // raw body so the preview never crashes the editor. Lint surfaces the
      // underlying issue separately.
      return wrap(html);
    }
  }, [html, values]);

  // Seeded so the first paint is immediate; only later edits are debounced.
  const [doc, setDoc] = useState(next);
  useEffect(() => {
    if (doc === next) return;
    const timer = setTimeout(() => setDoc(next), RELOAD_DEBOUNCE_MS);
    return () => clearTimeout(timer);
  }, [doc, next]);

  // Grow the frame to fit its content. srcdoc + allow-same-origin keeps the
  // inner document reachable, so we observe it directly rather than injecting a
  // postMessage bridge into someone else's markup.
  const frameRef = useRef<HTMLIFrameElement>(null);
  const [height, setHeight] = useState(MIN_HEIGHT);
  useEffect(() => {
    const frame = frameRef.current;
    if (!frame || typeof ResizeObserver === "undefined") return;
    let observer: ResizeObserver | undefined;

    function attach() {
      observer?.disconnect();
      const inner = frame?.contentDocument;
      if (!inner?.body) return;
      const measure = () =>
        setHeight(
          Math.max(
            MIN_HEIGHT,
            inner.body.scrollHeight,
            inner.documentElement.scrollHeight,
          ),
        );
      measure();
      observer = new ResizeObserver(measure);
      observer.observe(inner.body);
      observer.observe(inner.documentElement);
    }

    // `load` covers the reload each srcdoc change triggers; the direct call
    // covers a frame that already finished loading.
    frame.addEventListener("load", attach);
    attach();
    return () => {
      frame.removeEventListener("load", attach);
      observer?.disconnect();
    };
  }, [doc]);

  return (
    <iframe
      ref={frameRef}
      title="Component preview"
      srcDoc={doc}
      sandbox={SANDBOX}
      className={className}
      style={{ width: "100%", height, border: 0, display: "block" }}
    />
  );
}
