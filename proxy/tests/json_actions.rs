//! Unit tests for the expression-node JSON action primitives (spec §4):
//! `trim_json`, `add_attribute`, and the `apply_action_json` / `apply_action_html`
//! dispatchers. Covers match, no-op, and idempotency per the spec.

use rre_proxy::domain::applier::json_apply::{
    add_attribute, apply_action_html, apply_action_json, trim_json,
};
use rre_proxy::infra::backend_client::ActiveOutcome;
use serde_json::{json, Value};

// ---------------------------------------------------------------------------
// trim_json
// ---------------------------------------------------------------------------

#[test]
fn trim_json_truncates_to_length() {
    let mut body = json!({ "body": [1, 2, 3, 4, 5] });
    let changed = trim_json(&mut body, "$.body", 2).unwrap();
    assert!(changed);
    assert_eq!(body, json!({ "body": [1, 2] }));
}

#[test]
fn trim_json_length_zero_empties_array() {
    let mut body = json!({ "body": [1, 2, 3] });
    let changed = trim_json(&mut body, "$.body", 0).unwrap();
    assert!(changed);
    assert_eq!(body, json!({ "body": [] }));
}

#[test]
fn trim_json_negative_length_treated_as_zero() {
    let mut body = json!({ "body": [1, 2, 3] });
    let changed = trim_json(&mut body, "$.body", -5).unwrap();
    assert!(changed);
    assert_eq!(body, json!({ "body": [] }));
}

#[test]
fn trim_json_length_ge_actual_is_noop() {
    let mut body = json!({ "body": [1, 2] });
    // min(actual=2, length=10) = 2 -> no change.
    let changed = trim_json(&mut body, "$.body", 10).unwrap();
    assert!(!changed);
    assert_eq!(body, json!({ "body": [1, 2] }));
}

#[test]
fn trim_json_non_array_is_noop() {
    let mut body = json!({ "body": "scalar" });
    let changed = trim_json(&mut body, "$.body", 0).unwrap();
    assert!(!changed);
    assert_eq!(body, json!({ "body": "scalar" }));
}

#[test]
fn trim_json_missing_path_is_noop() {
    let mut body = json!({ "other": [1, 2, 3] });
    let changed = trim_json(&mut body, "$.body", 0).unwrap();
    assert!(!changed);
    assert_eq!(body, json!({ "other": [1, 2, 3] }));
}

#[test]
fn trim_json_is_idempotent() {
    let mut body = json!({ "body": [1, 2, 3, 4] });
    let first = trim_json(&mut body, "$.body", 1).unwrap();
    assert!(first);
    // Second run with the same length: already short enough -> no change.
    let second = trim_json(&mut body, "$.body", 1).unwrap();
    assert!(!second);
    assert_eq!(body, json!({ "body": [1] }));
}

// ---------------------------------------------------------------------------
// add_attribute
// ---------------------------------------------------------------------------

#[test]
fn add_attribute_sets_top_level_value() {
    let mut body = json!({ "a": 1 });
    let changed = add_attribute(&mut body, "$.paywall_show", json!("<html>x</html>")).unwrap();
    assert!(changed);
    assert_eq!(body, json!({ "a": 1, "paywall_show": "<html>x</html>" }));
}

#[test]
fn add_attribute_creates_missing_parents() {
    let mut body = json!({});
    let changed = add_attribute(&mut body, "$.a.b.c", json!(true)).unwrap();
    assert!(changed);
    assert_eq!(body, json!({ "a": { "b": { "c": true } } }));
}

#[test]
fn add_attribute_overwrites_existing() {
    let mut body = json!({ "flag": false });
    let changed = add_attribute(&mut body, "$.flag", json!(true)).unwrap();
    assert!(changed);
    assert_eq!(body, json!({ "flag": true }));
}

#[test]
fn add_attribute_is_idempotent() {
    let mut body = json!({});
    let first = add_attribute(&mut body, "$.x", json!("v")).unwrap();
    assert!(first);
    // Re-setting the same value -> no change.
    let second = add_attribute(&mut body, "$.x", json!("v")).unwrap();
    assert!(!second);
    assert_eq!(body, json!({ "x": "v" }));
}

// ---------------------------------------------------------------------------
// apply_action_json dispatcher
// ---------------------------------------------------------------------------

#[test]
fn apply_action_json_dispatches_trim_and_add() {
    // trim_json action.
    let mut body = json!({ "body": [1, 2, 3], "api": "dn-article" });
    let trim = json!({ "type": "trim_json", "json_path": "$.body", "length": 0 });
    assert!(apply_action_json(&mut body, &trim, &[]));
    assert_eq!(body["body"], json!([]));

    // add_attribute action.
    let add = json!({ "type": "add_attribute", "json_path": "$.paywall_show", "value": "<html>p</html>" });
    assert!(apply_action_json(&mut body, &add, &[]));
    assert_eq!(body["paywall_show"], json!("<html>p</html>"));
}

#[test]
fn apply_action_json_length_as_string() {
    let mut body = json!({ "body": [1, 2, 3] });
    // `length` arriving as a numeric string (manifest text control) is parsed.
    let trim = json!({ "type": "trim_json", "json_path": "$.body", "length": "1" });
    assert!(apply_action_json(&mut body, &trim, &[]));
    assert_eq!(body["body"], json!([1]));
}

#[test]
fn apply_action_json_unknown_type_is_noop() {
    let mut body = json!({ "x": 1 });
    let action = json!({ "type": "mystery", "json_path": "$.x" });
    assert!(!apply_action_json(&mut body, &action, &[]));
    assert_eq!(body, json!({ "x": 1 }));
}

#[test]
fn apply_action_json_missing_type_is_noop() {
    let mut body = json!({ "x": 1 });
    let action = json!({ "json_path": "$.x", "length": 0 });
    assert!(!apply_action_json(&mut body, &action, &[]));
    assert_eq!(body, json!({ "x": 1 }));
}

#[test]
fn apply_action_json_apply_outcome_runs_components() {
    let outcomes: Vec<ActiveOutcome> = serde_json::from_value(json!([
        {
            "id": "22222222-2222-2222-2222-222222222222",
            "title": "Lock", "is_builtin": false, "order_index": 0,
            "components": [
                { "id": "44444444-4444-4444-4444-444444444444", "slug": "lock",
                  "type": "json_set",
                  "config": { "type": "json_set", "target_path": "$.locked", "value": true },
                  "placement": "inline", "order_index": 0 }
            ]
        }
    ]))
    .unwrap();

    let mut body = json!({ "locked": false });
    let action =
        json!({ "type": "apply_outcome", "outcome_id": "22222222-2222-2222-2222-222222222222" });
    assert!(apply_action_json(&mut body, &action, &outcomes));
    assert_eq!(body["locked"], json!(true));
}

#[test]
fn apply_action_json_apply_outcome_missing_outcome_is_noop() {
    let mut body = json!({ "locked": false });
    let action =
        json!({ "type": "apply_outcome", "outcome_id": "99999999-9999-9999-9999-999999999999" });
    assert!(!apply_action_json(&mut body, &action, &[]));
    assert_eq!(body, json!({ "locked": false }));
}

// ---------------------------------------------------------------------------
// apply_action_html dispatcher (trim/add are JSON-only no-ops)
// ---------------------------------------------------------------------------

#[test]
fn apply_action_html_trim_and_add_are_noops() {
    let sanitizer =
        rre_proxy::domain::applier::html_sanitizer::load_sanitizer("config/sanitizer.yaml")
            .unwrap();
    let html = "<html><body>x</body></html>".to_string();

    let trim = json!({ "type": "trim_json", "json_path": "$.body", "length": 0 });
    let (out, changed) = apply_action_html(html.clone(), &trim, &[], &sanitizer);
    assert!(!changed);
    assert_eq!(out, html);

    let add = json!({ "type": "add_attribute", "json_path": "$.x", "value": "v" });
    let (out, changed) = apply_action_html(html.clone(), &add, &[], &sanitizer);
    assert!(!changed);
    assert_eq!(out, html);
}

#[test]
fn apply_action_html_apply_outcome_injects() {
    let sanitizer =
        rre_proxy::domain::applier::html_sanitizer::load_sanitizer("config/sanitizer.yaml")
            .unwrap();
    let outcomes: Vec<ActiveOutcome> = serde_json::from_value(json!([
        {
            "id": "22222222-2222-2222-2222-222222222222",
            "title": "Paywall", "is_builtin": false, "order_index": 0,
            "components": [
                { "id": "44444444-4444-4444-4444-444444444444", "slug": "wall",
                  "type": "html_injection",
                  "config": { "type": "html_injection", "target_selector": "#body",
                              "placement_mode": "append", "html_body": "<div>Subscribe</div>" },
                  "placement": "inline", "order_index": 0 }
            ]
        }
    ]))
    .unwrap();

    let html = r#"<html><body><div id="body"><p>article</p></div></body></html>"#.to_string();
    let action =
        json!({ "type": "apply_outcome", "outcome_id": "22222222-2222-2222-2222-222222222222" });
    let (out, changed) = apply_action_html(html, &action, &outcomes, &sanitizer);
    assert!(changed);
    assert!(out.contains("Subscribe"), "out: {out}");
}

/// Folding the §7 worked-example actions over a body equals running once
/// (idempotent transform). `apply_action_json` mutates in place; re-applying the
/// same chain to the already-mutated body is a no-op.
#[test]
fn folding_chain_is_idempotent() {
    let trim = json!({ "type": "trim_json", "json_path": "$.body", "length": 0 });
    let add = json!({ "type": "add_attribute", "json_path": "$.paywall_show", "value": "<html>p</html>" });

    let mut body: Value = json!({ "api": "dn-article", "body": [1, 2, 3] });
    let mut applied = false;
    applied |= apply_action_json(&mut body, &trim, &[]);
    applied |= apply_action_json(&mut body, &add, &[]);
    assert!(applied);
    let once = body.clone();

    // Second pass over the same body: no further change.
    let mut twice = false;
    twice |= apply_action_json(&mut body, &trim, &[]);
    twice |= apply_action_json(&mut body, &add, &[]);
    assert!(!twice);
    assert_eq!(body, once);
}
