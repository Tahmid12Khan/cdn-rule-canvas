//! Unit tests for the JSON applier: the simple-path parser (`applier::json_path`)
//! and the mutation orchestrator (`applier::json_apply::apply_outcome_json`)
//! covering remove / set (upsert + intermediate creation) / replace (exists-only),
//! per-component fail-open, and idempotency.

use rre_proxy::domain::applier::json_apply::apply_outcome_json;
use rre_proxy::domain::applier::json_path::{self, ParsePathError, Seg};
use rre_proxy::infra::backend_client::ActiveOutcome;
use serde_json::{json, Value};

// ---------------------------------------------------------------------------
// json_path parser
// ---------------------------------------------------------------------------

#[test]
fn parses_dollar_dot_key() {
    assert_eq!(
        json_path::parse("$.user.premium").unwrap(),
        vec![Seg::Key("user".into()), Seg::Key("premium".into())]
    );
}

#[test]
fn parses_index_and_bracket_key() {
    assert_eq!(
        json_path::parse("$.items[0].price").unwrap(),
        vec![
            Seg::Key("items".into()),
            Seg::Index(0),
            Seg::Key("price".into())
        ]
    );
    assert_eq!(
        json_path::parse(r#"$["a-b"][2]"#).unwrap(),
        vec![Seg::Key("a-b".into()), Seg::Index(2)]
    );
}

#[test]
fn parses_bare_leading_key() {
    assert_eq!(
        json_path::parse("user.name").unwrap(),
        vec![Seg::Key("user".into()), Seg::Key("name".into())]
    );
}

#[test]
fn rejects_filter_wildcard_recursive() {
    assert_eq!(
        json_path::parse("$.items[*]"),
        Err(ParsePathError::Unsupported)
    );
    assert_eq!(
        json_path::parse("$..price"),
        Err(ParsePathError::Unsupported)
    );
    assert_eq!(
        json_path::parse("$.items[?(@.id==1)]"),
        Err(ParsePathError::Unsupported)
    );
    assert_eq!(
        json_path::parse("$.items[1:3]"),
        Err(ParsePathError::Unsupported)
    );
}

#[test]
fn rejects_empty_and_overlong() {
    assert_eq!(json_path::parse(""), Err(ParsePathError::Empty));
    assert_eq!(json_path::parse("   "), Err(ParsePathError::Empty));
    let long = format!("$.{}", "a".repeat(600));
    assert_eq!(json_path::parse(&long), Err(ParsePathError::TooLong));
}

// ---------------------------------------------------------------------------
// json_apply: build outcomes from JSON for robustness against field changes.
// ---------------------------------------------------------------------------

fn outcome(components: Value) -> ActiveOutcome {
    let v = json!({
        "id": "00000000-0000-0000-0000-000000000001",
        "title": "JSON outcome",
        "is_builtin": false,
        "order_index": 0,
        "components": components,
    });
    serde_json::from_value(v).expect("valid ActiveOutcome")
}

fn component(slug: &str, kind: &str, order: i32, config: Value) -> Value {
    json!({
        "id": format!("00000000-0000-0000-0000-0000000000{order:02}"),
        "slug": slug,
        "type": kind,
        "config": config,
        "placement": "inline",
        "order_index": order,
    })
}

#[test]
fn json_remove_deletes_existing_key() {
    let o = outcome(json!([component(
        "rm",
        "json_remove",
        0,
        json!({ "target_path": "$.user.premium" })
    )]));
    let body = json!({ "user": { "premium": true, "name": "a" } });
    let res = apply_outcome_json(body, &o).unwrap();
    assert!(res.applied);
    assert_eq!(res.json, json!({ "user": { "name": "a" } }));
}

#[test]
fn json_remove_missing_key_is_noop() {
    let o = outcome(json!([component(
        "rm",
        "json_remove",
        0,
        json!({ "target_path": "$.user.absent" })
    )]));
    let body = json!({ "user": { "name": "a" } });
    let res = apply_outcome_json(body.clone(), &o).unwrap();
    assert!(!res.applied);
    assert_eq!(res.json, body);
}

#[test]
fn json_remove_array_index() {
    let o = outcome(json!([component(
        "rm",
        "json_remove",
        0,
        json!({ "target_path": "$.items[1]" })
    )]));
    let body = json!({ "items": [10, 20, 30] });
    let res = apply_outcome_json(body, &o).unwrap();
    assert!(res.applied);
    assert_eq!(res.json, json!({ "items": [10, 30] }));
}

#[test]
fn json_set_overwrites_existing() {
    let o = outcome(json!([component(
        "set",
        "json_set",
        0,
        json!({ "target_path": "$.user.premium", "value": false })
    )]));
    let body = json!({ "user": { "premium": true } });
    let res = apply_outcome_json(body, &o).unwrap();
    assert!(res.applied);
    assert_eq!(res.json, json!({ "user": { "premium": false } }));
}

#[test]
fn json_set_creates_intermediate_objects() {
    let o = outcome(json!([component(
        "set",
        "json_set",
        0,
        json!({ "target_path": "$.a.b.c", "value": 1 })
    )]));
    let body = json!({});
    let res = apply_outcome_json(body, &o).unwrap();
    assert!(res.applied);
    assert_eq!(res.json, json!({ "a": { "b": { "c": 1 } } }));
}

#[test]
fn json_set_is_idempotent() {
    let o = outcome(json!([component(
        "set",
        "json_set",
        0,
        json!({ "target_path": "$.x", "value": "v" })
    )]));
    let body = json!({ "x": "v" });
    let res = apply_outcome_json(body.clone(), &o).unwrap();
    // Same value already present -> not "applied" (no change).
    assert!(!res.applied);
    assert_eq!(res.json, body);
}

#[test]
fn json_replace_only_when_exists() {
    let present = outcome(json!([component(
        "rep",
        "json_replace",
        0,
        json!({ "target_path": "$.x", "value": "new" })
    )]));
    let res = apply_outcome_json(json!({ "x": "old" }), &present).unwrap();
    assert!(res.applied);
    assert_eq!(res.json, json!({ "x": "new" }));

    let absent = outcome(json!([component(
        "rep",
        "json_replace",
        0,
        json!({ "target_path": "$.y", "value": "new" })
    )]));
    let res = apply_outcome_json(json!({ "x": "old" }), &absent).unwrap();
    // Path missing -> replace is a no-op (does NOT create).
    assert!(!res.applied);
    assert_eq!(res.json, json!({ "x": "old" }));
}

#[test]
fn json_set_array_element_overwrites() {
    let o = outcome(json!([component(
        "set",
        "json_set",
        0,
        json!({ "target_path": "$.items[0]", "value": 99 })
    )]));
    let body = json!({ "items": [10, 20] });
    let res = apply_outcome_json(body, &o).unwrap();
    assert!(res.applied);
    assert_eq!(res.json, json!({ "items": [99, 20] }));
}

#[test]
fn json_set_array_element_out_of_range_is_noop() {
    let o = outcome(json!([component(
        "set",
        "json_set",
        0,
        json!({ "target_path": "$.items[5]", "value": 99 })
    )]));
    let body = json!({ "items": [10, 20] });
    let res = apply_outcome_json(body.clone(), &o).unwrap();
    // Index past the end -> fail-open no-op.
    assert!(!res.applied);
    assert_eq!(res.json, body);
}

#[test]
fn json_replace_array_element_out_of_range_is_noop() {
    let o = outcome(json!([component(
        "rep",
        "json_replace",
        0,
        json!({ "target_path": "$.items[5]", "value": 99 })
    )]));
    let body = json!({ "items": [10, 20] });
    let res = apply_outcome_json(body.clone(), &o).unwrap();
    // Path does not resolve (index past the end) -> replace is a no-op.
    assert!(!res.applied);
    assert_eq!(res.json, body);
}

#[test]
fn json_set_array_element_is_idempotent() {
    let o = outcome(json!([component(
        "set",
        "json_set",
        0,
        json!({ "target_path": "$.items[0]", "value": 99 })
    )]));
    let first = apply_outcome_json(json!({ "items": [10, 20] }), &o).unwrap();
    assert!(first.applied);
    // Re-setting the same value at the same index -> no change.
    let second = apply_outcome_json(first.json.clone(), &o).unwrap();
    assert!(!second.applied);
    assert_eq!(second.json, first.json);
}

#[test]
fn json_replace_array_element_overwrites_when_present() {
    let present = outcome(json!([component(
        "rep",
        "json_replace",
        0,
        json!({ "target_path": "$.items[1]", "value": "new" })
    )]));
    let res = apply_outcome_json(json!({ "items": ["a", "b"] }), &present).unwrap();
    assert!(res.applied);
    assert_eq!(res.json, json!({ "items": ["a", "new"] }));

    // Array entirely absent -> replace does not create anything.
    let absent = outcome(json!([component(
        "rep",
        "json_replace",
        0,
        json!({ "target_path": "$.items[0]", "value": "new" })
    )]));
    let res = apply_outcome_json(json!({ "other": 1 }), &absent).unwrap();
    assert!(!res.applied);
    assert_eq!(res.json, json!({ "other": 1 }));
}

#[test]
fn json_set_does_not_clobber_existing_scalar_intermediate() {
    let o = outcome(json!([component(
        "set",
        "json_set",
        0,
        json!({ "target_path": "$.a.b", "value": 1 })
    )]));
    // `$.a` is a scalar; descending into it would destroy it -> fail-open skip.
    let body = json!({ "a": "scalar" });
    let res = apply_outcome_json(body.clone(), &o).unwrap();
    assert!(!res.applied);
    assert_eq!(res.json, body);
}

#[test]
fn json_set_upserts_when_intermediate_missing_or_object() {
    // Missing intermediate `$.a` -> created as an object, then `b` set.
    let o = outcome(json!([component(
        "set",
        "json_set",
        0,
        json!({ "target_path": "$.a.b", "value": 1 })
    )]));
    let res = apply_outcome_json(json!({}), &o).unwrap();
    assert!(res.applied);
    assert_eq!(res.json, json!({ "a": { "b": 1 } }));

    // Present object intermediate `$.a` -> preserved, `b` added alongside.
    let res = apply_outcome_json(json!({ "a": { "existing": true } }), &o).unwrap();
    assert!(res.applied);
    assert_eq!(res.json, json!({ "a": { "existing": true, "b": 1 } }));
}

#[test]
fn html_and_unknown_component_types_are_noops() {
    let o = outcome(json!([
        component(
            "html",
            "html_injection",
            0,
            json!({ "target_selector": "#x" })
        ),
        component("unk", "mystery_type", 1, json!({})),
    ]));
    let body = json!({ "x": 1 });
    let res = apply_outcome_json(body.clone(), &o).unwrap();
    assert!(!res.applied);
    assert_eq!(res.json, body);
}

#[test]
fn bad_target_path_fails_open_per_component() {
    let o = outcome(json!([
        component(
            "bad",
            "json_set",
            0,
            json!({ "target_path": "$.items[*]", "value": 1 })
        ),
        component(
            "good",
            "json_set",
            1,
            json!({ "target_path": "$.ok", "value": 2 })
        ),
    ]));
    let body = json!({});
    let res = apply_outcome_json(body, &o).unwrap();
    // The wildcard path is rejected (skipped); the valid one still applies.
    assert!(res.applied);
    assert_eq!(res.json, json!({ "ok": 2 }));
}

#[test]
fn full_outcome_runs_components_in_order() {
    let o = outcome(json!([
        component("rm", "json_remove", 1, json!({ "target_path": "$.secret" })),
        component(
            "set",
            "json_set",
            0,
            json!({ "target_path": "$.locked", "value": true })
        ),
    ]));
    let body = json!({ "secret": "s", "locked": false });
    let res = apply_outcome_json(body, &o).unwrap();
    assert!(res.applied);
    assert_eq!(res.json, json!({ "locked": true }));
}
