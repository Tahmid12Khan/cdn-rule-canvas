# Task 16 — Component Configuration Modal

## Goal
Build the component config modal (§4.7) supporting two MVP component types: **HTML Injection** and **Content Truncation**. Includes the Visual/HTML editor toggle for the rich-text HTML field.

## Dependencies
Task 15.

## Acceptance Criteria
- Modal opens when a component row "Edit" is clicked, or right after a new component is added via the drawer.
- Modal header shows the component slug (auto-generated from type + counter, editable). `✕` closes (with dirty confirm). `< Select a Component` link returns to the type picker.
- Fields per type:
  - **HTML Injection (`type=html_injection`):**
    - `Target Selector` — text input (CSS selector, validated to be parseable).
    - `Placement Mode` — select: `replace`, `append`, `prepend`, `before`, `after`.
    - `HTML Body` — rich-text editor with **Visual** and **HTML** tabs.
      - HTML tab: CodeMirror 6 with HTML syntax highlighting.
      - Visual tab: TipTap-based WYSIWYG (`StarterKit` + Link).
      - Content syncs both ways via TipTap's `editor.getHTML()` and `editor.commands.setContent(html)`.
    - `Theme` — text input free-form for now ("dark", "light", etc.).
  - **Content Truncation (`type=content_truncation`):**
    - `Target Selector` — text input.
    - `Word Count` — number input (1–10000).
    - `Fade Out` — toggle switch.
- Footer: Cancel | Save. Save persists via `PATCH /api/v1/components/{id}` (or POST when new).
- On row in Outcome editor: tag badges reflect config (e.g., `20 Words | Fade Out` for truncation; `Replace · main article` for HTML injection).
- Vitest tests for each field's validation; integration-style test that switching Visual ↔ HTML preserves content; jsdom may not run CodeMirror cleanly — wrap CM in a thin component and test in isolation with a mock or under Playwright.

## Implementation Steps
1. Install `@tiptap/react`, `@tiptap/starter-kit`, `@tiptap/extension-link`, `@codemirror/lang-html`, `@uiw/react-codemirror`.
2. `src/components/component-config/ComponentConfigModal.tsx` — Radix `Dialog`.
3. `src/components/component-config/HtmlInjectionForm.tsx`.
4. `src/components/component-config/ContentTruncationForm.tsx`.
5. `src/components/component-config/RichTextEditor.tsx` — TipTap + CM6 tabs.
6. `src/lib/canvas/componentBadges.ts` — derives badge string from config.
7. Schemas in `src/lib/schemas/components.ts` mirrored from backend (kept in sync; consider generating from OpenAPI).
8. Tests as described above.

## Files
- Components above
- `frontend/src/components/component-config/__tests__/*.test.tsx`
- (Backend) extend the `ComponentConfig` serde enum to discriminate by `type` (`#[serde(tag = "type")]`) so it validates server-side too — add `0005_component_config_validation.{up,down}.sql` migration only if column changes (it doesn't; JSONB already exists).

## Tests
- HtmlInjectionForm: invalid selector blocks Save.
- ContentTruncationForm: Word Count below 1 invalid.
- RichTextEditor: switching tabs preserves content (rendered with both editors mounted but only one visible).
- Component row badge text updates after Save.

## Verify
1. From outcome editor, add HTML Injection component, target `.article-paywall`, replace mode, paste sample HTML → Save → reload → field values present.
2. Toggle Visual ↔ HTML tabs in rich editor and confirm content is identical.
3. Add Content Truncation, `20 words / Fade Out true` → row shows `20 Words | Fade Out` badge.

## Done When
PR merged with screenshots: Modal in HTML mode, Modal in Visual mode, truncation modal, outcome editor with both component badges visible.
