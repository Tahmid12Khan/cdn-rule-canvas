"use client";

// Shared request-header rows editor for the rule-builder Test panels (synthetic
// "Test a rule" + "Test with a live URL"). Renders name/value rows with add/
// remove, per-row inline validation via the canonical headerEntryError rules,
// and a too-many-headers guard. Extracted from UrlTestPanel so both panels share
// the EXACT same UX + validation (CONTRACTS.md FRONTEND; @/lib/validation/headers).
import { headerEntryError, MAX_HEADERS } from "@/lib/validation/headers";

export interface HeaderRow {
  key: string;
  value: string;
}

interface HeaderRowsEditorProps {
  rows: HeaderRow[];
  onChange: (rows: HeaderRow[]) => void;
  // Unique-per-instance id fragment so two editors on one screen don't collide.
  idPrefix?: string;
  // aria-label prefix for each name/value input. Defaults to "Test header" so
  // the existing UrlTestPanel labels ("Test header name 1", …) are preserved.
  labelPrefix?: string;
}

// Collapse rows into a header map: drop blank-name rows (last write wins on a
// duplicate name). `valid` is false when any non-blank row fails the
// name/value rules OR more than MAX_HEADERS non-blank rows are present.
export function validateHeaderRows(rows: HeaderRow[]): {
  headers: Record<string, string>;
  valid: boolean;
} {
  const headers = rows.reduce<Record<string, string>>((acc, row) => {
    const k = row.key.trim();
    if (k) acc[k] = row.value;
    return acc;
  }, {});
  const nonBlankCount = rows.filter((r) => r.key.trim() !== "").length;
  const tooMany = nonBlankCount > MAX_HEADERS;
  const anyRowError = rows.some((r) => {
    const e = headerEntryError(r.key, r.value);
    return Boolean(e.name || e.value);
  });
  return { headers, valid: !tooMany && !anyRowError };
}

export function HeaderRowsEditor({
  rows,
  onChange,
  labelPrefix = "Test header",
}: HeaderRowsEditorProps) {
  const headerErrors = rows.map((r) => headerEntryError(r.key, r.value));
  const nonBlankCount = rows.filter((r) => r.key.trim() !== "").length;
  const tooManyHeaders = nonBlankCount > MAX_HEADERS;

  function updateRow(index: number, patch: Partial<HeaderRow>) {
    onChange(rows.map((r, i) => (i === index ? { ...r, ...patch } : r)));
  }
  function addRow() {
    onChange([...rows, { key: "", value: "" }]);
  }
  function removeRow(index: number) {
    onChange(rows.length === 1 ? rows : rows.filter((_, i) => i !== index));
  }

  return (
    <div className="mt-1 flex flex-col gap-2">
      {rows.map((row, i) => {
        const err = headerErrors[i];
        return (
          <div key={i} className="flex flex-col gap-1">
            <div className="flex items-center gap-2">
              <input
                type="text"
                value={row.key}
                onChange={(e) => updateRow(i, { key: e.target.value })}
                placeholder="Header-Name"
                aria-label={`${labelPrefix} name ${i + 1}`}
                aria-invalid={Boolean(err.name) || undefined}
                className="w-1/3 rounded-md border border-status-prevBg px-2 py-1.5 text-sm focus:border-brand-500 focus:outline-none aria-[invalid]:border-danger"
              />
              <input
                type="text"
                value={row.value}
                onChange={(e) => updateRow(i, { value: e.target.value })}
                placeholder="value"
                aria-label={`${labelPrefix} value ${i + 1}`}
                aria-invalid={Boolean(err.value) || undefined}
                className="flex-1 rounded-md border border-status-prevBg px-2 py-1.5 text-sm focus:border-brand-500 focus:outline-none aria-[invalid]:border-danger"
              />
              <button
                type="button"
                onClick={() => removeRow(i)}
                aria-label={`Remove ${labelPrefix.toLowerCase()} ${i + 1}`}
                className="rounded p-1 text-status-prev hover:bg-status-prevBg disabled:opacity-40"
                disabled={rows.length === 1}
              >
                ×
              </button>
            </div>
            {(err.name || err.value) && (
              <p role="alert" className="text-xs font-medium text-danger">
                {err.name ?? err.value}
              </p>
            )}
          </div>
        );
      })}
      <button
        type="button"
        onClick={addRow}
        className="w-fit text-xs font-medium text-action-600 hover:text-action-700"
      >
        + Add header
      </button>
      {tooManyHeaders && (
        <p role="alert" className="text-xs font-medium text-danger">
          At most {MAX_HEADERS} headers are allowed.
        </p>
      )}
    </div>
  );
}
