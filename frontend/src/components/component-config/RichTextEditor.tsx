"use client";

// RichTextEditor (FRONTEND CONTRACT §2.5, Task 16).
//
// The contract calls for a Visual (TipTap) ⇄ HTML (CodeMirror6) editor. Neither
// TipTap nor CodeMirror is declared in package.json, so per the task brief this
// is the sanctioned "textarea + code tab" fallback:
//   - "Visual" tab: an editable textarea plus a live, sanitized preview.
//   - "HTML" tab: a monospace code textarea editing the same raw HTML string.
// Both tabs are bound to the same controlled `value`, so switching tabs never
// loses content. The preview goes through <SafeHtml/> (DOMPurify) — the only
// sanctioned raw-HTML sink. FLAGGED FOR SECURITY REVIEW.
import { useId, useState } from "react";
import clsx from "clsx";

import { SafeHtml } from "@/components/ui/SafeHtml";

type EditorTab = "visual" | "html";

interface RichTextEditorProps {
  value: string;
  onChange: (next: string) => void;
  id?: string;
  describedBy?: string;
  invalid?: boolean;
}

const TABS: { key: EditorTab; label: string }[] = [
  { key: "visual", label: "Visual" },
  { key: "html", label: "HTML" },
];

export function RichTextEditor({
  value,
  onChange,
  id,
  describedBy,
  invalid,
}: RichTextEditorProps) {
  const [tab, setTab] = useState<EditorTab>("visual");
  const generatedId = useId();
  const fieldId = id ?? generatedId;

  return (
    <div
      className={clsx(
        "overflow-hidden rounded-lg border",
        invalid ? "border-danger" : "border-border",
      )}
    >
      <div
        role="tablist"
        aria-label="HTML editor mode"
        className="flex border-b border-border bg-bg"
      >
        {TABS.map((t) => {
          const selected = tab === t.key;
          return (
            <button
              key={t.key}
              type="button"
              role="tab"
              aria-selected={selected}
              onClick={() => setTab(t.key)}
              className={clsx(
                "px-4 py-2 text-sm font-medium transition-colors",
                selected
                  ? "border-b-2 border-brand-500 text-brand-700"
                  : "border-b-2 border-transparent text-fg-muted hover:text-fg",
              )}
            >
              {t.label}
            </button>
          );
        })}
      </div>

      {tab === "visual" ? (
        <div className="space-y-2 p-3">
          <textarea
            id={fieldId}
            aria-describedby={describedBy}
            aria-invalid={invalid || undefined}
            value={value}
            onChange={(e) => onChange(e.target.value)}
            rows={5}
            placeholder="Write the HTML content to inject…"
            className="w-full resize-y rounded-md border border-border p-2 text-sm focus:border-brand-500 focus:outline-none"
          />
          <div>
            <p className="mb-1 text-xs font-medium uppercase tracking-wide text-fg-subtle">
              Preview
            </p>
            <SafeHtml
              html={value}
              className="min-h-[2.5rem] rounded-md border border-dashed border-border bg-bg-elevated p-2 text-sm"
            />
          </div>
        </div>
      ) : (
        <textarea
          id={fieldId}
          aria-describedby={describedBy}
          aria-invalid={invalid || undefined}
          value={value}
          onChange={(e) => onChange(e.target.value)}
          rows={8}
          spellCheck={false}
          placeholder="<div>…</div>"
          className="w-full resize-y bg-nav p-3 font-mono text-sm text-nav-fg focus:outline-none"
        />
      )}
    </div>
  );
}
