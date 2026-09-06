"use client";

// CodeMirror 6 HTML editor for the Component Editor (design §5.3 left pane).
// Imported via next/dynamic with `ssr: false` from the editor page because
// CodeMirror touches `document`/`window` at module init and would break SSR.
//
// The lint gutter is fed by the pure `htmlLintEngine` (advisory only — never
// blocks Save, design item 8): each diagnostic is mapped to a CodeMirror
// Diagnostic with the engine's character offsets.
import { useMemo } from "react";
import { html as htmlLang } from "@codemirror/lang-html";
import { linter, lintGutter, type Diagnostic } from "@codemirror/lint";
import { EditorView } from "@codemirror/view";
import CodeMirror from "@uiw/react-codemirror";

import { htmlLintEngine } from "@/lib/canvas/htmlLintEngine";

interface HtmlCodeEditorProps {
  value: string;
  onChange: (value: string) => void;
  readOnly?: boolean;
}

export function HtmlCodeEditor({
  value,
  onChange,
  readOnly = false,
}: HtmlCodeEditorProps) {
  const extensions = useMemo(
    () => [
      htmlLang(),
      lintGutter(),
      linter((view): Diagnostic[] => {
        const doc = view.state.doc.toString();
        return htmlLintEngine(doc).map((d) => ({
          from: Math.min(d.from, doc.length),
          to: Math.min(d.to, doc.length),
          severity: d.severity,
          message: d.message,
        }));
      }),
      EditorView.lineWrapping,
    ],
    [],
  );

  return (
    <CodeMirror
      value={value}
      onChange={onChange}
      extensions={extensions}
      readOnly={readOnly}
      height="100%"
      basicSetup={{ lineNumbers: true, foldGutter: false }}
      className="h-full overflow-auto rounded-md border border-border text-sm"
    />
  );
}
