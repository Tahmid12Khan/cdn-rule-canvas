//! Integration tests for the `component_ref` (HTML) / `component_ref_json` (JSON)
//! COMPONENTS inside an outcome (Component Editor, design §4) — applied through the
//! outcome appliers `orchestrator::apply_outcome` / `json_apply::apply_outcome_json`
//! that consume a PRE-RESOLVED `ResolvedComponentMap`. (Distinct from the
//! `apply_component` / `apply_component_json` ACTIONS in `component_applier.rs`.)
//!
//! Covers: render + inject at a selector (inline placement); render + set-at-path
//! (JSON); sticky_footer / popup row placement honored; mustache escaping + ammonia
//! sanitize of the rendered output; idempotency (re-running == once); fail-open on a
//! missing resolution / render error.

use std::collections::HashMap;
use std::sync::Arc;

use rre_proxy::domain::applier::html_sanitizer::load_sanitizer;
use rre_proxy::domain::applier::json_apply::{apply_outcome_json, ResolvedComponentMap};
use rre_proxy::domain::applier::orchestrator::apply_outcome;
use rre_proxy::infra::backend_client::{
    ActiveComponent, ActiveOutcome, Placement, ResolvedComponent, VersionSelector,
};
use serde_json::{json, Value};
use uuid::Uuid;

fn sanitizer() -> ammonia::Builder<'static> {
    load_sanitizer("config/sanitizer.yaml").unwrap()
}

const CID: &str = "55555555-5555-5555-5555-555555555555";

fn cid() -> Uuid {
    Uuid::parse_str(CID).unwrap()
}

/// One-entry pre-resolved map keyed by `(cid, selector)`.
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

/// A `component_ref` (HTML) component with the given placement + config.
fn component_ref(placement: Placement, order: i32, config: Value) -> ActiveComponent {
    ActiveComponent {
        id: Uuid::parse_str("66666666-6666-6666-6666-666666666666").unwrap(),
        slug: "ref".to_string(),
        r#type: "component_ref".to_string(),
        config,
        placement,
        order_index: order,
    }
}

/// A `component_ref_json` (JSON) component (placement is irrelevant for JSON).
fn component_ref_json(order: i32, config: Value) -> ActiveComponent {
    ActiveComponent {
        id: Uuid::parse_str("77777777-7777-7777-7777-777777777777").unwrap(),
        slug: "refjson".to_string(),
        r#type: "component_ref_json".to_string(),
        config,
        placement: Placement::Inline,
        order_index: order,
    }
}

fn outcome(components: Vec<ActiveComponent>) -> ActiveOutcome {
    ActiveOutcome {
        id: Uuid::parse_str("88888888-8888-8888-8888-888888888888").unwrap(),
        title: "Paywall".to_string(),
        is_builtin: false,
        order_index: 0,
        components,
    }
}

// ---------------------------------------------------------------------------
// component_ref (HTML) — inside an outcome (apply_outcome path)
// ---------------------------------------------------------------------------

#[test]
fn component_ref_renders_and_injects_inline() {
    let map = resolved_map(
        VersionSelector::Default,
        1,
        "<p class=\"promo\">{{headline}}</p>",
    );
    let o = outcome(vec![component_ref(
        Placement::Inline,
        0,
        json!({
            "type": "component_ref",
            "component_id": CID,
            "version": "default",
            "variables": { "headline": "Subscribe now" },
            "target_selector": "#article-body",
            "placement_mode": "append"
        }),
    )]);
    let html =
        r#"<html><body><div id="article-body"><p>article</p></div></body></html>"#.to_string();

    let res = apply_outcome(html, &o, &map, &sanitizer()).unwrap();
    assert!(res.applied);
    assert!(res.html.contains("Subscribe now"), "out: {}", res.html);
    assert!(res.html.contains("class=\"promo\""), "out: {}", res.html);
    assert!(
        res.html.contains("<p>article</p>"),
        "original kept: {}",
        res.html
    );
}

#[test]
fn component_ref_escapes_and_sanitizes_rendered_output() {
    // `{{safe}}` is HTML-escaped; `{{{raw}}}` is raw but ammonia strips <script>.
    let map = resolved_map(VersionSelector::Default, 1, "<div>{{safe}}{{{raw}}}</div>");
    let o = outcome(vec![component_ref(
        Placement::Inline,
        0,
        json!({
            "type": "component_ref",
            "component_id": CID,
            "version": "default",
            "variables": {
                "safe": "<b>x</b>",
                "raw": "<script>alert(1)</script><p>ok</p>"
            },
            "target_selector": "#t",
            "placement_mode": "append"
        }),
    )]);
    let html = r#"<div id="t"></div>"#.to_string();

    let res = apply_outcome(html, &o, &map, &sanitizer()).unwrap();
    assert!(
        res.html.contains("&lt;b&gt;x&lt;/b&gt;"),
        "escaped: {}",
        res.html
    );
    assert!(
        !res.html.contains("<script>"),
        "script stripped: {}",
        res.html
    );
    assert!(
        res.html.contains("<p>ok</p>"),
        "allowed tag kept: {}",
        res.html
    );
}

#[test]
fn component_ref_inline_is_idempotent() {
    let map = resolved_map(VersionSelector::Default, 1, "<p>{{x}}</p>");
    let o = outcome(vec![component_ref(
        Placement::Inline,
        0,
        json!({
            "type": "component_ref",
            "component_id": CID,
            "version": "default",
            "variables": { "x": "hi" },
            "target_selector": "#t",
            "placement_mode": "append"
        }),
    )]);
    let html = r#"<div id="t"></div>"#.to_string();

    let once = apply_outcome(html, &o, &map, &sanitizer()).unwrap();
    assert!(once.applied);
    let twice = apply_outcome(once.html.clone(), &o, &map, &sanitizer()).unwrap();
    assert!(
        !twice.applied,
        "second pass is a no-op (idempotency marker)"
    );
    assert_eq!(once.html, twice.html);
}

#[test]
fn component_ref_sticky_footer_placement_honored() {
    let map = resolved_map(VersionSelector::Default, 1, "<p>{{cta}}</p>");
    let o = outcome(vec![component_ref(
        Placement::StickyFooter,
        0,
        json!({
            "type": "component_ref",
            "component_id": CID,
            "version": "default",
            "variables": { "cta": "Sign up" }
        }),
    )]);
    let html = r#"<html><head></head><body><p>body</p></body></html>"#.to_string();

    let res = apply_outcome(html, &o, &map, &sanitizer()).unwrap();
    assert!(res.applied);
    assert!(
        res.html.contains("rre-sticky-footer"),
        "wrapper: {}",
        res.html
    );
    assert!(res.html.contains("Sign up"), "rendered: {}", res.html);
    // Idempotent: a second pass with the same marker is a no-op.
    let twice = apply_outcome(res.html.clone(), &o, &map, &sanitizer()).unwrap();
    assert_eq!(res.html, twice.html);
}

#[test]
fn component_ref_popup_placement_honored() {
    let map = resolved_map(VersionSelector::Default, 1, "<p>{{msg}}</p>");
    let o = outcome(vec![component_ref(
        Placement::Popup,
        0,
        json!({
            "type": "component_ref",
            "component_id": CID,
            "version": "default",
            "variables": { "msg": "Hello" }
        }),
    )]);
    let html = r#"<html><head></head><body><p>body</p></body></html>"#.to_string();

    let res = apply_outcome(html, &o, &map, &sanitizer()).unwrap();
    assert!(res.applied);
    assert!(res.html.contains("rre-popup"), "wrapper: {}", res.html);
    assert!(res.html.contains("Hello"), "rendered: {}", res.html);
}

#[test]
fn component_ref_missing_resolution_fails_open() {
    // Empty map -> unresolved -> body untouched, not applied.
    let map: ResolvedComponentMap = HashMap::new();
    let o = outcome(vec![component_ref(
        Placement::Inline,
        0,
        json!({
            "type": "component_ref",
            "component_id": CID,
            "version": "default",
            "variables": {},
            "target_selector": "#t",
            "placement_mode": "append"
        }),
    )]);
    let html = r#"<div id="t">original</div>"#.to_string();

    let res = apply_outcome(html.clone(), &o, &map, &sanitizer()).unwrap();
    assert!(!res.applied);
    assert_eq!(res.html, html, "fail-open: served original");
}

#[test]
fn component_ref_render_error_fails_open() {
    // Unbalanced mustache -> render error -> skip; body untouched.
    let map = resolved_map(VersionSelector::Default, 1, "<p>{{unclosed</p>");
    let o = outcome(vec![component_ref(
        Placement::Inline,
        0,
        json!({
            "type": "component_ref",
            "component_id": CID,
            "version": "default",
            "variables": {},
            "target_selector": "#t",
            "placement_mode": "append"
        }),
    )]);
    let html = r#"<div id="t">original</div>"#.to_string();

    let res = apply_outcome(html.clone(), &o, &map, &sanitizer()).unwrap();
    assert!(!res.applied);
    assert_eq!(res.html, html, "fail-open on render error");
}

#[test]
fn component_ref_pinned_version_resolves_by_selector() {
    // The map is keyed by Version(2); the component config pins version=2.
    let map = resolved_map(VersionSelector::Version(2), 2, "<p>{{x}}</p>");
    let o = outcome(vec![component_ref(
        Placement::Inline,
        0,
        json!({
            "type": "component_ref",
            "component_id": CID,
            "version": 2,
            "variables": { "x": "pinned" },
            "target_selector": "#t",
            "placement_mode": "append"
        }),
    )]);
    let html = r#"<div id="t"></div>"#.to_string();

    let res = apply_outcome(html, &o, &map, &sanitizer()).unwrap();
    assert!(res.applied);
    assert!(res.html.contains("pinned"), "out: {}", res.html);
}

// ---------------------------------------------------------------------------
// component_ref_json (JSON) — inside an outcome (apply_outcome path)
// ---------------------------------------------------------------------------

#[test]
fn component_ref_json_sets_rendered_html_at_path() {
    let map = resolved_map(VersionSelector::Default, 1, "<p>{{headline}}</p>");
    let o = outcome(vec![component_ref_json(
        0,
        json!({
            "type": "component_ref_json",
            "component_id": CID,
            "version": "default",
            "variables": { "headline": "Hi" },
            "target_path": "$.content.html"
        }),
    )]);
    let body = json!({ "content": { "title": "t" } });

    let res = apply_outcome_json(body, &o, &map, &sanitizer()).unwrap();
    assert!(res.applied);
    assert_eq!(res.json["content"]["html"], json!("<p>Hi</p>"));
    assert_eq!(
        res.json["content"]["title"],
        json!("t"),
        "sibling untouched"
    );
}

#[test]
fn component_ref_json_creates_intermediate_objects() {
    let map = resolved_map(VersionSelector::Default, 1, "<p>{{x}}</p>");
    let o = outcome(vec![component_ref_json(
        0,
        json!({
            "type": "component_ref_json",
            "component_id": CID,
            "version": "default",
            "variables": { "x": "v" },
            "target_path": "$.a.b.html"
        }),
    )]);
    let res = apply_outcome_json(json!({}), &o, &map, &sanitizer()).unwrap();
    assert!(res.applied);
    assert_eq!(res.json, json!({ "a": { "b": { "html": "<p>v</p>" } } }));
}

#[test]
fn component_ref_json_is_idempotent() {
    let map = resolved_map(VersionSelector::Default, 1, "<p>{{x}}</p>");
    let o = outcome(vec![component_ref_json(
        0,
        json!({
            "type": "component_ref_json",
            "component_id": CID,
            "version": "default",
            "variables": { "x": "v" },
            "target_path": "$.html"
        }),
    )]);
    let first = apply_outcome_json(json!({}), &o, &map, &sanitizer()).unwrap();
    assert!(first.applied);
    let second = apply_outcome_json(first.json.clone(), &o, &map, &sanitizer()).unwrap();
    assert!(
        !second.applied,
        "re-setting the same rendered string is a no-op"
    );
    assert_eq!(first.json, second.json);
}

#[test]
fn component_ref_json_sanitizes_rendered_output() {
    let map = resolved_map(VersionSelector::Default, 1, "<div>{{{raw}}}</div>");
    let o = outcome(vec![component_ref_json(
        0,
        json!({
            "type": "component_ref_json",
            "component_id": CID,
            "version": "default",
            "variables": { "raw": "<script>x</script><p>ok</p>" },
            "target_path": "$.html"
        }),
    )]);
    let res = apply_outcome_json(json!({}), &o, &map, &sanitizer()).unwrap();
    assert!(res.applied);
    let html = res.json["html"].as_str().unwrap();
    assert!(!html.contains("<script>"), "script stripped: {html}");
    assert!(html.contains("<p>ok</p>"), "allowed tag kept: {html}");
}

#[test]
fn component_ref_json_missing_resolution_fails_open() {
    let map: ResolvedComponentMap = HashMap::new();
    let o = outcome(vec![component_ref_json(
        0,
        json!({
            "type": "component_ref_json",
            "component_id": CID,
            "version": "default",
            "variables": {},
            "target_path": "$.html"
        }),
    )]);
    let body: Value = json!({ "keep": 1 });

    let res = apply_outcome_json(body.clone(), &o, &map, &sanitizer()).unwrap();
    assert!(!res.applied);
    assert_eq!(res.json, body, "fail-open: body untouched");
}

#[test]
fn component_ref_json_render_error_fails_open() {
    let map = resolved_map(VersionSelector::Default, 1, "<p>{{unclosed</p>");
    let o = outcome(vec![component_ref_json(
        0,
        json!({
            "type": "component_ref_json",
            "component_id": CID,
            "version": "default",
            "variables": {},
            "target_path": "$.html"
        }),
    )]);
    let body: Value = json!({ "keep": 1 });

    let res = apply_outcome_json(body.clone(), &o, &map, &sanitizer()).unwrap();
    assert!(!res.applied);
    assert_eq!(res.json, body, "fail-open on render error");
}
