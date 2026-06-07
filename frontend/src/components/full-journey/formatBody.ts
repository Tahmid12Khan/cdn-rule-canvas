// Pretty-print a journey step's body_after for diffing. Mirrors
// TransformationJourney's local (non-exported) helper: JSON features stringify
// the value with 2-space indent; HTML features show the raw string as-is
// (stringifying a non-string fallback defensively). Kept identical so the
// full-journey diffs read the same as the single-canvas Transformation Journey
// diffs. NOTE: this is a small intentional duplicate of that helper — if either
// changes, keep them in sync (or promote to a shared lib/canvas module).
export function formatBody(value: unknown, isJson: boolean): string {
  if (isJson) {
    try {
      return JSON.stringify(value, null, 2);
    } catch {
      return String(value);
    }
  }
  return typeof value === "string" ? value : JSON.stringify(value, null, 2);
}
