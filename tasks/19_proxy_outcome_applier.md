# Task 19 — Proxy Outcome Applier (Response Modification)

## Goal
Apply the outcome identified in Task 18 to the upstream HTML response. The `outcome_id` is produced by the zen `DecisionEngine` evaluation in Task 18 (parsed from `DecisionGraphResponse.result`'s `{ outcomeId }` payload); this task simply maps that id to its component config and renders. Implement the two MVP component types: **HTML Injection** and **Content Truncation**, plus the pass-through `ShowContent` builtin.

## Dependencies
Task 16, 18.

## Acceptance Criteria
- Applier runs ordered components for the selected outcome (`order_index` ascending). Inline components first, then placement-aware (sticky footer / popup) appended to `<body>`.
- **HTML Injection component:**
  - Read `target_selector`, `placement_mode` (`replace`/`append`/`prepend`/`before`/`after`), `html_body` (sanitized via `ammonia` with a configurable allow-list; configurable means the allow-list lives in `proxy/config/sanitizer.yaml`).
  - Rewrites the HTML via `lol_html` element handlers, then emits the modified document.
- **Content Truncation component:**
  - Read `target_selector`, `word_count` (int), `fade_out` (bool).
  - Truncates the target element's text content (preserves inline tags up to the word budget — implement as a token-by-token walker, not naive split).
  - If `fade_out`, wraps the truncated remainder area in a `<div class="rre-fade-out">` and appends a `<style>` block once with the gradient CSS.
- **Sticky Footer placement:** wraps the component HTML in `<div class="rre-sticky-footer">…</div>` appended to `<body>`. Inline CSS injected once.
- **Popup placement:** wraps in `<div class="rre-popup" role="dialog">…</div>` + backdrop + close button. CSS injected once.
- `ShowContent` outcome short-circuits — no modifications.
- Response Content-Length recalculated; preserve gzip handling (decompress → modify → recompress via `flate2`) when applicable. Other encodings (br/deflate): pass-through unchanged with a log warning.
- Errors during outcome application caught and logged; the original response is returned unmodified rather than failing the request. A header `X-RRE-Apply-Status: ok|skipped|error` is set for debuggability.
- Unit tests for each component renderer + applier orchestration; integration test against the demo upstream.

## Implementation Steps
1. `proxy/src/domain/applier/mod.rs` — `ComponentRenderer` trait.
2. `applier/html_injection.rs`, `applier/content_truncation.rs`, `applier/placement_sticky_footer.rs`, `applier/placement_popup.rs`.
3. `applier/orchestrator.rs::apply_outcome(html, outcome) -> ModificationResult`.
4. `applier/html_sanitizer.rs` wrapping `ammonia`.
5. `forwarder.rs` extended: after `evaluator::evaluate` returns an outcome id (from the zen `DecisionEngine` evaluation in Task 18 — the applier is decoupled from how the id was computed), look up its config from the cached payload, invoke the orchestrator, write back the modified HTML.
6. Gzip helper `proxy/src/infra/encoding.rs` (via `flate2`).
7. Tests:
   - `html_injection.rs` — each placement mode produces correct DOM.
   - `content_truncation.rs` — word counting respects inline tags, no broken markup.
   - `placement_sticky_footer.rs` / `placement_popup.rs` — injected once even if applied twice; correct class names.
   - `orchestrator.rs` — ordering, ShowContent no-op, error caught → header `X-RRE-Apply-Status: error`.
   - `proxy_e2e_html.rs` using `wiremock` returning the seeded article — full pipeline.

## Files
- Proxy applier modules as above
- `proxy/config/sanitizer.yaml`
- Tests as above

## Tests
- Minimum 15 distinct tests across applier modules.
- E2E pipeline test: rule graph + outcome with both component types → modified HTML matches snapshot.

## Verify
1. In dashboard, attach outcome "Show Paywall" with: (a) Content Truncation `#article-body / 20 words / fade out` and (b) HTML Injection `#article-body / append / <div>SUBSCRIBE NOW</div>`. Publish to LIVE.
2. `curl localhost:9000/article.html` → response body is truncated, fade applied, paywall HTML appended.
3. `curl -H "Cookie: rre_user_type=customer" localhost:9000/article.html` → unmodified (customer canvas empty).
4. `curl -i localhost:9000/article.html` → `X-RRE-Apply-Status: ok`.

## Done When
PR merged with side-by-side curl outputs (unmodified vs modified).
