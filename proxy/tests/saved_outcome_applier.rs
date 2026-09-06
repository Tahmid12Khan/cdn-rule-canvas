//! Integration tests for the `apply_saved_outcome` (HTML) and
//! `apply_saved_outcome_json` (JSON) expression actions (Outcomes Library):
//! the sync applier branches that consume a PRE-RESOLVED
//! `ResolvedSavedOutcomeMap`. No mustache rendering happens here — the backend
//! `resolve` route already rendered the saved outcome's `html_body`.
//!
//! Covers: inject at a selector with placement, set-at-path, ammonia
//! sanitization of the resolved output, idempotency (re-running == once), and
//! fail-open on a missing resolution.

use std::collections::HashMap;
use std::sync::Arc;

use rre_proxy::domain::applier::html_sanitizer::load_sanitizer;
use rre_proxy::domain::applier::json_apply::{
    apply_action_html, apply_action_json, ResolvedComponentMap, ResolvedSavedOutcomeMap,
};
use rre_proxy::infra::saved_outcome_cache::ResolvedSavedOutcome;
use serde_json::json;
use uuid::Uuid;

fn sanitizer() -> ammonia::Builder<'static> {
    load_sanitizer("config/sanitizer.yaml").unwrap()
}

/// Empty pre-resolved component map: these tests exercise Saved Outcome
/// actions, so no Component template is ever resolved.
fn no_components() -> ResolvedComponentMap {
    ResolvedComponentMap::new()
}

const SOID: &str = "55555555-5555-5555-5555-555555555555";

fn soid() -> Uuid {
    Uuid::parse_str(SOID).unwrap()
}

/// Build a one-entry resolved-saved-outcome map keyed by `soid()`.
fn resolved_map(html_body: &str) -> ResolvedSavedOutcomeMap {
    let mut map: ResolvedSavedOutcomeMap = HashMap::new();
    map.insert(
        soid(),
        Arc::new(ResolvedSavedOutcome {
            html_body: html_body.to_string(),
        }),
    );
    map
}

// ---------------------------------------------------------------------------
// apply_saved_outcome (HTML)
// ---------------------------------------------------------------------------

#[test]
fn apply_saved_outcome_injects_at_selector() {
    let map = resolved_map("<p class=\"promo\">Subscribe now</p>");
    let action = json!({
        "type": "apply_saved_outcome",
        "saved_outcome_id": SOID,
        "target_selector": "#article-body",
        "placement_mode": "append"
    });
    let html =
        r#"<html><body><div id="article-body"><p>article</p></div></body></html>"#.to_string();

    let (out, changed) =
        apply_action_html(html, &action, &[], &no_components(), &map, &sanitizer());
    assert!(changed);
    assert!(out.contains("Subscribe now"), "out: {out}");
    assert!(out.contains("<p>article</p>"), "original kept: {out}");
}

#[test]
fn apply_saved_outcome_sanitizes_resolved_html() {
    // The resolve endpoint already rendered the mustache; the proxy still
    // ammonia-sanitizes it before injection (defense in depth).
    let map = resolved_map("<script>alert(1)</script><p>ok</p>");
    let action = json!({
        "type": "apply_saved_outcome",
        "saved_outcome_id": SOID,
        "target_selector": "#t",
        "placement_mode": "append"
    });
    let html = r#"<div id="t"></div>"#.to_string();

    let (out, _) = apply_action_html(html, &action, &[], &no_components(), &map, &sanitizer());
    assert!(!out.contains("<script>"), "script stripped: {out}");
    assert!(out.contains("<p>ok</p>"), "allowed tag kept: {out}");
}

#[test]
fn apply_saved_outcome_is_idempotent() {
    let map = resolved_map("<p>hi</p>");
    let action = json!({
        "type": "apply_saved_outcome",
        "saved_outcome_id": SOID,
        "target_selector": "#t",
        "placement_mode": "append"
    });
    let html = r#"<div id="t"></div>"#.to_string();

    let (once, changed1) =
        apply_action_html(html, &action, &[], &no_components(), &map, &sanitizer());
    assert!(changed1);
    let (twice, changed2) = apply_action_html(
        once.clone(),
        &action,
        &[],
        &no_components(),
        &map,
        &sanitizer(),
    );
    assert!(!changed2, "second pass is a no-op (idempotency marker)");
    assert_eq!(once, twice);
}

#[test]
fn apply_saved_outcome_missing_resolution_fails_open() {
    // Map is EMPTY -> the saved outcome is unresolved -> body untouched.
    let map: ResolvedSavedOutcomeMap = HashMap::new();
    let action = json!({
        "type": "apply_saved_outcome",
        "saved_outcome_id": SOID,
        "target_selector": "#t",
        "placement_mode": "append"
    });
    let html = r#"<div id="t">original</div>"#.to_string();

    let (out, changed) = apply_action_html(
        html.clone(),
        &action,
        &[],
        &no_components(),
        &map,
        &sanitizer(),
    );
    assert!(!changed);
    assert_eq!(out, html, "fail-open: served original");
}

#[test]
fn apply_saved_outcome_json_on_html_body_is_noop() {
    let map = resolved_map("<p>hi</p>");
    let action = json!({
        "type": "apply_saved_outcome_json",
        "saved_outcome_id": SOID,
        "target_path": "$.html"
    });
    let html = "<html><body>x</body></html>".to_string();
    let (out, changed) = apply_action_html(
        html.clone(),
        &action,
        &[],
        &no_components(),
        &map,
        &sanitizer(),
    );
    assert!(!changed);
    assert_eq!(out, html);
}

// ---------------------------------------------------------------------------
// apply_saved_outcome_json (JSON)
// ---------------------------------------------------------------------------

#[test]
fn apply_saved_outcome_json_sets_resolved_html_at_path() {
    let map = resolved_map("<p>Hi</p>");
    let action = json!({
        "type": "apply_saved_outcome_json",
        "saved_outcome_id": SOID,
        "target_path": "$.content.html"
    });
    let mut body = json!({ "content": { "title": "t" } });

    let changed = apply_action_json(
        &mut body,
        &action,
        &[],
        &no_components(),
        &map,
        &sanitizer(),
    );
    assert!(changed);
    assert_eq!(body["content"]["html"], json!("<p>Hi</p>"));
    assert_eq!(body["content"]["title"], json!("t"));
}

#[test]
fn apply_saved_outcome_json_is_idempotent() {
    let map = resolved_map("<p>v</p>");
    let action = json!({
        "type": "apply_saved_outcome_json",
        "saved_outcome_id": SOID,
        "target_path": "$.html"
    });
    let mut body = json!({});

    let first = apply_action_json(
        &mut body,
        &action,
        &[],
        &no_components(),
        &map,
        &sanitizer(),
    );
    assert!(first);
    let once = body.clone();
    let second = apply_action_json(
        &mut body,
        &action,
        &[],
        &no_components(),
        &map,
        &sanitizer(),
    );
    assert!(!second, "re-setting the same resolved string is a no-op");
    assert_eq!(body, once);
}

#[test]
fn apply_saved_outcome_json_missing_resolution_fails_open() {
    let map: ResolvedSavedOutcomeMap = HashMap::new();
    let action = json!({
        "type": "apply_saved_outcome_json",
        "saved_outcome_id": SOID,
        "target_path": "$.html"
    });
    let mut body = json!({ "keep": 1 });
    let before = body.clone();

    let changed = apply_action_json(
        &mut body,
        &action,
        &[],
        &no_components(),
        &map,
        &sanitizer(),
    );
    assert!(!changed);
    assert_eq!(body, before, "fail-open: body untouched");
}

#[test]
fn apply_saved_outcome_on_json_body_is_noop() {
    let map = resolved_map("<p>v</p>");
    let action = json!({
        "type": "apply_saved_outcome",
        "saved_outcome_id": SOID,
        "target_selector": "#t",
        "placement_mode": "append"
    });
    let mut body = json!({ "k": 1 });
    let before = body.clone();
    let changed = apply_action_json(
        &mut body,
        &action,
        &[],
        &no_components(),
        &map,
        &sanitizer(),
    );
    assert!(!changed);
    assert_eq!(body, before);
}
