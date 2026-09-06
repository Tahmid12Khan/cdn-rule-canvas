// Extract the ordered, unique mustache variable names from a template body
// (component-editor design §5.3). The regex matches both escaped `{{name}}` and
// raw `{{{name}}}` / `{{&name}}`-style triple braces; names may contain word
// characters and dots (e.g. `user.name`). This drives the Variables panel:
// names found here are the canonical set of declared variables.
//
// Kept dependency-free + pure so it is trivially unit-testable and reusable on
// both the editor and (later) the rule-node sub-form.
export const VARIABLE_RE = /\{\{\{?\s*([\w.]+)\s*\}?\}\}/g;

export function extractVariables(html: string): string[] {
  const seen = new Set<string>();
  const out: string[] = [];
  // A fresh RegExp avoids shared-lastIndex bugs from the module-level constant.
  const re = new RegExp(VARIABLE_RE.source, "g");
  let match: RegExpExecArray | null;
  while ((match = re.exec(html)) !== null) {
    const name = match[1];
    if (!seen.has(name)) {
      seen.add(name);
      out.push(name);
    }
    // Guard against zero-width matches looping forever.
    if (match.index === re.lastIndex) re.lastIndex += 1;
  }
  return out;
}
