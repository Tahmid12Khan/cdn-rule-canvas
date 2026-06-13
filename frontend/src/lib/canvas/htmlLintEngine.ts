// Pure HTML + mustache lint engine for the Component Editor (design §5.3 item
// 8). It produces ADVISORY diagnostics only — it NEVER blocks Save. Two classes
// of checks:
//   (a) HTML parse errors via DOMParser('text/html'): the browser's tolerant
//       parser flags malformed markup through an injected <parsererror> node
//       (most reliable in XML mode, but text/html surfaces some via the error
//       sink; we additionally run an unbalanced-tag heuristic for the common
//       case the lenient parser silently repairs).
//   (b) Unbalanced / empty mustache braces.
//
// `from`/`to` are absolute character offsets into the source so the caller can
// map them onto CodeMirror diagnostics.

export type LintSeverity = "warning" | "info";

export interface LintDiagnostic {
  from: number;
  to: number;
  severity: LintSeverity;
  message: string;
}

// Void elements never need a closing tag (HTML spec); excluded from the
// balance heuristic.
const VOID_TAGS = new Set([
  "area",
  "base",
  "br",
  "col",
  "embed",
  "hr",
  "img",
  "input",
  "link",
  "meta",
  "param",
  "source",
  "track",
  "wbr",
]);

// ── mustache brace checks ─────────────────────────────────────────────────────
function lintMustache(html: string): LintDiagnostic[] {
  const out: LintDiagnostic[] = [];
  // Walk `{{ ... }}` pairs. Unbalanced opens (no closing `}}`) and empty pairs
  // (`{{}}` / `{{   }}`) are flagged.
  let i = 0;
  while (i < html.length) {
    const open = html.indexOf("{{", i);
    if (open === -1) break;
    const close = html.indexOf("}}", open + 2);
    if (close === -1) {
      out.push({
        from: open,
        to: html.length,
        severity: "warning",
        message: "Unbalanced mustache braces: missing closing }}",
      });
      break;
    }
    // Strip an optional leading `{` (triple-brace raw form) and a leading `&`
    // (unescaped form) before checking emptiness.
    const inner = html
      .slice(open + 2, close)
      .replace(/^\{/, "")
      .replace(/^&/, "")
      .trim();
    if (inner === "") {
      out.push({
        from: open,
        to: close + 2,
        severity: "warning",
        message: "Empty mustache expression {{ }}",
      });
    }
    i = close + 2;
  }
  return out;
}

// ── unbalanced-tag heuristic ─────────────────────────────────────────────────
// The browser's text/html parser silently repairs most malformed markup, so a
// clean parse is not proof of well-formedness. This lightweight stack check
// catches the common authoring mistake of a missing/mismatched closing tag.
function lintTagBalance(html: string): LintDiagnostic[] {
  const out: LintDiagnostic[] = [];
  const tagRe = /<\/?([a-zA-Z][\w-]*)\b[^>]*?(\/?)>/g;
  const stack: { tag: string; from: number }[] = [];
  let m: RegExpExecArray | null;
  while ((m = tagRe.exec(html)) !== null) {
    const full = m[0];
    const tag = m[1].toLowerCase();
    const selfClosing = m[2] === "/" || VOID_TAGS.has(tag);
    const isClose = full.startsWith("</");
    if (isClose) {
      // Find the matching open on the stack (pop intervening unclosed tags).
      const idx = stack.map((s) => s.tag).lastIndexOf(tag);
      if (idx === -1) {
        out.push({
          from: m.index,
          to: m.index + full.length,
          severity: "warning",
          message: `Stray closing tag </${tag}> with no matching opener`,
        });
      } else {
        stack.length = idx;
      }
    } else if (!selfClosing) {
      stack.push({ tag, from: m.index });
    }
  }
  for (const open of stack) {
    out.push({
      from: open.from,
      to: open.from + open.tag.length + 1,
      severity: "warning",
      message: `Unclosed <${open.tag}> tag`,
    });
  }
  return out;
}

// ── DOMParser parse errors ────────────────────────────────────────────────────
// Best-effort: only runs where a DOMParser exists (browser / jsdom). Returns
// nothing on the server. The unbalanced-tag heuristic carries the common cases.
function lintParseErrors(html: string): LintDiagnostic[] {
  if (typeof DOMParser === "undefined") return [];
  try {
    const doc = new DOMParser().parseFromString(html, "text/html");
    const err = doc.querySelector("parsererror");
    if (err) {
      return [
        {
          from: 0,
          to: Math.min(html.length, 1),
          severity: "warning",
          message: `HTML parse error: ${err.textContent?.trim() ?? "malformed markup"}`,
        },
      ];
    }
  } catch {
    // Ignore parser exceptions — lint is advisory.
  }
  return [];
}

export function htmlLintEngine(html: string): LintDiagnostic[] {
  if (html.trim() === "") return [];
  return [
    ...lintParseErrors(html),
    ...lintTagBalance(html),
    ...lintMustache(html),
  ];
}
