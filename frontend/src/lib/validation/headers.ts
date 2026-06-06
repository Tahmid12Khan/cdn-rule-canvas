// Canonical HTTP request-header validation rules, shared by the Sites admin form
// and the rule-builder test panels (synthetic "Test a rule" + "Test with a live
// URL"). Mirrors the backend rules in backend/src/schemas/site.rs: a header NAME
// is an RFC 7230 token (1..=128 chars, no spaces/separators) and a header VALUE
// is at most 2048 visible-ASCII chars (no CR/LF/control characters). At most 32
// headers. Keeping these in one module means the proxy's `headers_to_map`
// defensive drop is a backstop, never the user's first line of feedback.

export const HEADER_NAME_RE = /^[!#$%&'*+\-.^_`|~0-9A-Za-z]+$/;
export const HEADER_VALUE_CONTROL_RE = /[\x00-\x1f\x7f]/;
export const MAX_HEADERS = 32;
export const MAX_HEADER_NAME_LEN = 128;
export const MAX_HEADER_VALUE_LEN = 2048;

export interface HeaderEntryError {
  name?: string;
  value?: string;
}

// Validate a single header name/value pair. Returns per-field messages; an empty
// object means valid. A blank name is treated as valid (an unused/placeholder
// row), matching how the panels drop blank rows before sending.
export function headerEntryError(name: string, value: string): HeaderEntryError {
  const err: HeaderEntryError = {};
  const trimmed = name.trim();
  if (trimmed === "") return err; // blank row — ignored on send

  if (trimmed.length > MAX_HEADER_NAME_LEN) {
    err.name = `Header name must be at most ${MAX_HEADER_NAME_LEN} characters`;
  } else if (!HEADER_NAME_RE.test(trimmed)) {
    err.name = "Invalid header name — use a valid HTTP token (no spaces)";
  }

  if (value.length > MAX_HEADER_VALUE_LEN) {
    err.value = `Header value must be at most ${MAX_HEADER_VALUE_LEN} characters`;
  } else if (HEADER_VALUE_CONTROL_RE.test(value)) {
    err.value = "Header value must not contain control characters";
  }

  return err;
}
