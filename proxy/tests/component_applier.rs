//! Integration tests for the `apply_component` (HTML) and `apply_component_json`
//! (JSON) expression actions (Component Editor, design §4.3): the sync applier
//! branches that consume a PRE-RESOLVED `ResolvedComponentMap`.
//!
//! Covers: inject at a selector with placement, set-at-path, mustache escaping,
//! ammonia sanitization of the rendered output, idempotency (re-running == once),
//! and fail-open on a missing resolution / render error / bad selector.

use std::collections::HashMap;
use std::sync::Arc;

use rre_proxy::domain::applier::html_sanitizer::load_sanitizer;
use rre_proxy::domain::applier::json_apply::{
    apply_action_html, apply_action_json, ResolvedComponentMap,
};
use rre_proxy::infra::backend_client::{ResolvedComponent, VersionSelector};
use serde_json::{json, Value};
use uuid::Uuid;

fn sanitizer() -> ammonia::Builder<'static> {
    load_sanitizer("config/sanitizer.yaml").unwrap()
}

const CID: &str = "44444444-4444-4444-4444-444444444444";

fn cid() -> Uuid {
    Uuid::parse_str(CID).unwrap()
}

/// Build a one-entry resolved-component map keyed by `(cid, selector)`.
fn resolved_map(
    selector: VersionSelector,
    version_number: i32,
    html_body: &str,
) -> ResolvedComponentMap {
    let rc: ResolvedComponent = serde_json::from_value(json!({
        "version_number": version_number,
        "html_body": html_body,
        "variables": []
    }))
    .unwrap();
    let mut map: ResolvedComponentMap = HashMap::new();
    map.insert((cid(), selector), Arc::new(rc));
    map
}

// ---------------------------------------------------------------------------
// apply_component (HTML)
// ---------------------------------------------------------------------------

#[test]
fn apply_component_renders_and_injects() {
    let map = resolved_map(
        VersionSelector::Default,
        1,
        "<p class=\"promo\">{{headline}}</p>",
    );
    let action = json!({
        "type": "apply_component",
        "component_id": CID,
        "version": "default",
        "variables": { "headline": "Subscribe now" },
        "target_selector": "#article-body",
        "placement_mode": "append"
    });
    let html =
        r#"<html><body><div id="article-body"><p>article</p></div></body></html>"#.to_string();

    let (out, changed) = apply_action_html(html, &action, &[], &map, &sanitizer());
    assert!(changed);
    assert!(out.contains("Subscribe now"), "out: {out}");
    assert!(out.contains("class=\"promo\""), "out: {out}");
    // Injected inside the target's content (append), not before it.
    assert!(out.contains("<p>article</p>"), "original kept: {out}");
}

#[test]
fn apply_component_escapes_double_brace_and_sanitizes() {
    // The rendered `{{x}}` is HTML-escaped by mustache; even if a raw `{{{x}}}`
    // produced markup, ammonia strips disallowed tags (e.g. `<script>`).
    let map = resolved_map(VersionSelector::Default, 1, "<div>{{safe}}{{{raw}}}</div>");
    let action = json!({
        "type": "apply_component",
        "component_id": CID,
        "version": "default",
        "variables": {
            "safe": "<b>x</b>",
            "raw": "<script>alert(1)</script><p>ok</p>"
        },
        "target_selector": "#t",
        "placement_mode": "append"
    });
    let html = r#"<div id="t"></div>"#.to_string();

    let (out, _) = apply_action_html(html, &action, &[], &map, &sanitizer());
    // `{{safe}}` escaped: no live <b> from the variable.
    assert!(out.contains("&lt;b&gt;x&lt;/b&gt;"), "escaped: {out}");
    // `{{{raw}}}` is raw but ammonia removed <script>.
    assert!(!out.contains("<script>"), "script stripped: {out}");
    assert!(out.contains("<p>ok</p>"), "allowed tag kept: {out}");
}

#[test]
fn apply_component_is_idempotent() {
    let map = resolved_map(VersionSelector::Default, 1, "<p>{{x}}</p>");
    let action = json!({
        "type": "apply_component",
        "component_id": CID,
        "version": "default",
        "variables": { "x": "hi" },
        "target_selector": "#t",
        "placement_mode": "append"
    });
    let html = r#"<div id="t"></div>"#.to_string();

    let (once, changed1) = apply_action_html(html, &action, &[], &map, &sanitizer());
    assert!(changed1);
    let (twice, changed2) = apply_action_html(once.clone(), &action, &[], &map, &sanitizer());
    assert!(!changed2, "second pass is a no-op (idempotency marker)");
    assert_eq!(once, twice);
}

#[test]
fn apply_component_missing_resolution_fails_open() {
    // Map is EMPTY → the component is unresolved → body untouched, changed=false.
    let map: ResolvedComponentMap = HashMap::new();
    let action = json!({
        "type": "apply_component",
        "component_id": CID,
        "version": "default",
        "variables": {},
        "target_selector": "#t",
        "placement_mode": "append"
    });
    let html = r#"<div id="t">original</div>"#.to_string();

    let (out, changed) = apply_action_html(html.clone(), &action, &[], &map, &sanitizer());
    assert!(!changed);
    assert_eq!(out, html, "fail-open: served original");
}

#[test]
fn apply_component_render_error_fails_open() {
    // Unbalanced mustache → render error → skip, body untouched.
    let map = resolved_map(VersionSelector::Default, 1, "<p>{{unclosed</p>");
    let action = json!({
        "type": "apply_component",
        "component_id": CID,
        "version": "default",
        "variables": {},
        "target_selector": "#t",
        "placement_mode": "append"
    });
    let html = r#"<div id="t">original</div>"#.to_string();

    let (out, changed) = apply_action_html(html.clone(), &action, &[], &map, &sanitizer());
    assert!(!changed);
    assert_eq!(out, html, "fail-open on render error");
}

#[test]
fn apply_component_bad_selector_fails_open() {
    let map = resolved_map(VersionSelector::Default, 1, "<p>{{x}}</p>");
    // A selector with disallowed characters is rejected → skip.
    let action = json!({
        "type": "apply_component",
        "component_id": CID,
        "version": "default",
        "variables": { "x": "hi" },
        "target_selector": "div { color: red }",
        "placement_mode": "append"
    });
    let html = r#"<div id="t">original</div>"#.to_string();

    let (out, changed) = apply_action_html(html.clone(), &action, &[], &map, &sanitizer());
    assert!(!changed);
    assert_eq!(out, html);
}

#[test]
fn apply_component_pinned_version_resolves_by_selector() {
    // The map is keyed by VersionSelector::Version(2); the action references
    // version=2 — they must match for the lookup to succeed.
    let map = resolved_map(VersionSelector::Version(2), 2, "<p>{{x}}</p>");
    let action = json!({
        "type": "apply_component",
        "component_id": CID,
        "version": 2,
        "variables": { "x": "pinned" },
        "target_selector": "#t",
        "placement_mode": "append"
    });
    let html = r#"<div id="t"></div>"#.to_string();

    let (out, changed) = apply_action_html(html, &action, &[], &map, &sanitizer());
    assert!(changed);
    assert!(out.contains("pinned"), "out: {out}");
}

// ---------------------------------------------------------------------------
// apply_component_json (JSON)
// ---------------------------------------------------------------------------

#[test]
fn apply_component_json_sets_rendered_html_at_path() {
    let map = resolved_map(VersionSelector::Default, 1, "<p>{{headline}}</p>");
    let action = json!({
        "type": "apply_component_json",
        "component_id": CID,
        "version": "default",
        "variables": { "headline": "Hi" },
        "target_path": "$.content.html"
    });
    let mut body = json!({ "content": { "title": "t" } });

    let changed = apply_action_json(&mut body, &action, &[], &map, &sanitizer());
    assert!(changed);
    assert_eq!(body["content"]["html"], json!("<p>Hi</p>"));
    // Sibling untouched.
    assert_eq!(body["content"]["title"], json!("t"));
}

#[test]
fn apply_component_json_is_idempotent() {
    let map = resolved_map(VersionSelector::Default, 1, "<p>{{x}}</p>");
    let action = json!({
        "type": "apply_component_json",
        "component_id": CID,
        "version": "default",
        "variables": { "x": "v" },
        "target_path": "$.html"
    });
    let mut body = json!({});

    let first = apply_action_json(&mut body, &action, &[], &map, &sanitizer());
    assert!(first);
    let once = body.clone();
    let second = apply_action_json(&mut body, &action, &[], &map, &sanitizer());
    assert!(!second, "re-setting the same rendered string is a no-op");
    assert_eq!(body, once);
}

#[test]
fn apply_component_json_missing_resolution_fails_open() {
    let map: ResolvedComponentMap = HashMap::new();
    let action = json!({
        "type": "apply_component_json",
        "component_id": CID,
        "version": "default",
        "variables": {},
        "target_path": "$.html"
    });
    let mut body: Value = json!({ "keep": 1 });
    let before = body.clone();

    let changed = apply_action_json(&mut body, &action, &[], &map, &sanitizer());
    assert!(!changed);
    assert_eq!(body, before, "fail-open: body untouched");
}

#[test]
fn apply_component_on_json_body_is_noop() {
    // The HTML `apply_component` action on a JSON body is skipped (warn + no-op).
    let map = resolved_map(VersionSelector::Default, 1, "<p>{{x}}</p>");
    let action = json!({
        "type": "apply_component",
        "component_id": CID,
        "version": "default",
        "variables": { "x": "v" },
        "target_selector": "#t",
        "placement_mode": "append"
    });
    let mut body = json!({ "k": 1 });
    let before = body.clone();
    let changed = apply_action_json(&mut body, &action, &[], &map, &sanitizer());
    assert!(!changed);
    assert_eq!(body, before);
}

#[test]
fn apply_component_json_on_html_body_is_noop() {
    let map = resolved_map(VersionSelector::Default, 1, "<p>{{x}}</p>");
    let action = json!({
        "type": "apply_component_json",
        "component_id": CID,
        "version": "default",
        "variables": { "x": "v" },
        "target_path": "$.html"
    });
    let html = "<html><body>x</body></html>".to_string();
    let (out, changed) = apply_action_html(html.clone(), &action, &[], &map, &sanitizer());
    assert!(!changed);
    assert_eq!(out, html);
}
