// Manifest-driven, pure helpers shared by the node, tooltip, palette, and config
// form (spec Part E). They implement the LOCKED validation + display rules so
// the frontend mirrors the backend (rule_graph_service::validate) exactly, with
// ZERO per-node-type code. No React here.
import type { NodeDisplayConfig, NodeFieldSpec, NodeTypeSpec } from "@/lib/api/nodeTypes";
import type { ProcessorConfig } from "@/lib/canvas/types";

// Build a dropped-node default config from a spec: `type` = kind, then each
// field's `default` (missing => omitted, so the node is intentionally
// incomplete and fails `required` until edited).
export function defaultProcessor(spec: NodeTypeSpec): ProcessorConfig {
  const cfg: ProcessorConfig = { type: spec.kind };
  for (const field of spec.fields) {
    if (field.default !== undefined) cfg[field.name] = field.default;
  }
  return cfg;
}

// A field is required when `required` is true OR (`required_unless` is set AND
// the named sibling field's current value !== required_unless.value).
export function isFieldRequired(
  field: NodeFieldSpec,
  processor: ProcessorConfig,
): boolean {
  if (field.required) return true;
  if (field.required_unless) {
    return processor[field.required_unless.field] !== field.required_unless.value;
  }
  return false;
}

// "Non-empty": present AND not null AND (for strings) not empty after trim.
function isNonEmpty(value: unknown): boolean {
  if (value === undefined || value === null) return false;
  if (typeof value === "string") return value.trim().length > 0;
  return true;
}

const GENERIC_REQUIRED = "This field is required";

// Per-field validation errors keyed by field name. Implements the locked rules:
// `processor_field_required`, `processor_field_option`. Unknown extra fields are
// ignored. Returns {} when the processor is valid against the spec.
export function validateProcessor(
  spec: NodeTypeSpec,
  processor: ProcessorConfig,
): Record<string, string> {
  const errors: Record<string, string> = {};
  for (const field of spec.fields) {
    const value = processor[field.name];

    // REQUIRED (processor_field_required)
    if (isFieldRequired(field, processor) && !isNonEmpty(value)) {
      errors[field.name] = field.required_message ?? GENERIC_REQUIRED;
      continue;
    }

    // SELECT OPTIONS (processor_field_option) — only when non-empty.
    if (field.control === "select" && isNonEmpty(value)) {
      const allowed = (field.options ?? []).map((o) => o.value);
      if (!allowed.includes(value)) {
        errors[field.name] = `Choose one of the available ${field.label.toLowerCase()} options`;
      }
    }
  }
  return errors;
}

export function isProcessorValid(
  spec: NodeTypeSpec,
  processor: ProcessorConfig,
): boolean {
  return Object.keys(validateProcessor(spec, processor)).length === 0;
}

// Human display value for a field: select -> the matching option label;
// text/number -> the raw value (stringified). Empty/absent -> "—".
export function fieldDisplayValue(
  field: NodeFieldSpec,
  processor: ProcessorConfig,
): string {
  const value = processor[field.name];
  if (value === undefined || value === null || value === "") return "—";
  if (field.control === "select") {
    const opt = (field.options ?? []).find((o) => o.value === value);
    if (opt) return opt.label;
  }
  return String(value);
}

// The node's display title = manifest label, falling back to the raw kind when
// no spec is loaded/known.
export function nodeTitle(
  spec: NodeTypeSpec | undefined,
  processor: ProcessorConfig,
): string {
  return spec?.label ?? processor.type;
}

const DEFAULT_VALUE_MAX_CHARS = 10;

// One token for the canvas condition summary: a select with a server-defined
// symbol -> the symbol; a select without -> the option label; text/number ->
// the value truncated to the server's value_max_chars + "…". Empty -> null.
export function fieldSummaryToken(
  field: NodeFieldSpec,
  processor: ProcessorConfig,
  maxChars: number,
): string | null {
  const value = processor[field.name];
  if (value === undefined || value === null || value === "") return null;
  if (field.control === "select") {
    const opt = (field.options ?? []).find((o) => o.value === value);
    if (opt?.symbol) return opt.symbol;
    if (opt) return opt.label;
    return String(value);
  }
  const s = String(value);
  return s.length > maxChars ? s.slice(0, maxChars) + "…" : s;
}

// The node's one-line condition summary: non-empty field tokens in field
// order, joined by spaces (e.g. "paywall ⊃ true", "== mobile"). The symbols
// and truncation length are server-owned (manifest); this only renders them.
export function nodeSummary(
  spec: NodeTypeSpec,
  processor: ProcessorConfig,
  display: NodeDisplayConfig | undefined,
): string {
  const max = display?.value_max_chars ?? DEFAULT_VALUE_MAX_CHARS;
  return spec.fields
    .map((f) => fieldSummaryToken(f, processor, max))
    .filter((t): t is string => t !== null)
    .join(" ");
}
