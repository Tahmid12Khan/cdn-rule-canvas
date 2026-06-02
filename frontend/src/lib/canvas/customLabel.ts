// Expression-node custom name validation (spec §v2.3). IDENTICAL rule to the
// backend (rule_graph_service): an empty/absent value is VALID (default = no
// custom name); a non-empty value MUST be snake_case matching
// ^[a-z0-9]+(_[a-z0-9]+)*$ (lowercase alphanumerics in single-underscore
// segments; no leading/trailing/double underscore, no uppercase, no spaces).
import { z } from "zod";

// LOCKED regex — keep byte-identical to the backend rule.
export const CUSTOM_LABEL_RE = /^[a-z0-9]+(_[a-z0-9]+)*$/;

// LOCKED inline message — shown on the form when validation fails.
export const CUSTOM_LABEL_MESSAGE =
  "Custom name must be snake_case (lowercase letters, digits, single underscores) or left empty.";

// Empty string is allowed; a non-empty value must match the snake_case regex.
export const CustomLabel = z
  .string()
  .refine((v) => v === "" || CUSTOM_LABEL_RE.test(v), CUSTOM_LABEL_MESSAGE);

// Returns the inline error message, or null when the value is valid
// (empty/absent counts as valid).
export function validateCustomLabel(value: string): string | null {
  const result = CustomLabel.safeParse(value);
  return result.success ? null : CUSTOM_LABEL_MESSAGE;
}
