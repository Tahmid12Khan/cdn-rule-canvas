//! Unit tests for the `json_expression` processor (kind = "json_expression").
//! Covers every operator, the no-body -> No path, and the bad-path fail-open.

use std::collections::HashMap;

use http::HeaderMap;
use rre_proxy::domain::context::{EvaluationContext, EvaluationContextParts};
use rre_proxy::domain::processors::{
    json_expression::JsonExpressionProcessor, Branch, CanvasProcessor,
};
use serde_json::json;

/// Build a JSON-mode context from a body string.
fn ctx_json(body: &str) -> EvaluationContext {
    EvaluationContextParts::from_request(
        &HeaderMap::new(),
        "/",
        &HashMap::new(),
        body.to_string(),
        true,
    )
    .into_context()
}

/// Build an HTML-mode context (no response_json).
fn ctx_html() -> EvaluationContext {
    EvaluationContextParts::from_request(
        &HeaderMap::new(),
        "/",
        &HashMap::new(),
        "<html></html>".to_string(),
        false,
    )
    .into_context()
}

#[test]
fn equals_matches_string_value() {
    let p = JsonExpressionProcessor;
    let ctx = ctx_json(r#"{"type":"premium"}"#);
    let cfg = json!({ "json_path": "$.type", "operator": "equals", "value": "premium" });
    assert_eq!(p.evaluate(&cfg, &ctx).unwrap().branch, Branch::Yes);

    let miss = json!({ "json_path": "$.type", "operator": "equals", "value": "free" });
    assert_eq!(p.evaluate(&miss, &ctx).unwrap().branch, Branch::No);
}

#[test]
fn equals_matches_non_string_node_via_to_string() {
    let p = JsonExpressionProcessor;
    let ctx = ctx_json(r#"{"count":42,"flag":true}"#);
    let num = json!({ "json_path": "$.count", "operator": "equals", "value": "42" });
    assert_eq!(p.evaluate(&num, &ctx).unwrap().branch, Branch::Yes);
    let boolean = json!({ "json_path": "$.flag", "operator": "equals", "value": "true" });
    assert_eq!(p.evaluate(&boolean, &ctx).unwrap().branch, Branch::Yes);
}

#[test]
fn contains_starts_ends_with() {
    let p = JsonExpressionProcessor;
    let ctx = ctx_json(r#"{"title":"Breaking News Today"}"#);
    let c = json!({ "json_path": "$.title", "operator": "contains", "value": "News" });
    assert_eq!(p.evaluate(&c, &ctx).unwrap().branch, Branch::Yes);
    let s = json!({ "json_path": "$.title", "operator": "starts_with", "value": "Breaking" });
    assert_eq!(p.evaluate(&s, &ctx).unwrap().branch, Branch::Yes);
    let e = json!({ "json_path": "$.title", "operator": "ends_with", "value": "Today" });
    assert_eq!(p.evaluate(&e, &ctx).unwrap().branch, Branch::Yes);
    let no = json!({ "json_path": "$.title", "operator": "starts_with", "value": "News" });
    assert_eq!(p.evaluate(&no, &ctx).unwrap().branch, Branch::No);
}

#[test]
fn is_one_of_comma_separated() {
    let p = JsonExpressionProcessor;
    let ctx = ctx_json(r#"{"tier":"gold"}"#);
    let cfg =
        json!({ "json_path": "$.tier", "operator": "is_one_of", "value": "silver, gold, bronze" });
    assert_eq!(p.evaluate(&cfg, &ctx).unwrap().branch, Branch::Yes);
    let miss = json!({ "json_path": "$.tier", "operator": "is_one_of", "value": "silver, bronze" });
    assert_eq!(p.evaluate(&miss, &ctx).unwrap().branch, Branch::No);
}

#[test]
fn exists_operator() {
    let p = JsonExpressionProcessor;
    let ctx = ctx_json(r#"{"meta":{"locked":true}}"#);
    let present = json!({ "json_path": "$.meta.locked", "operator": "exists" });
    assert_eq!(p.evaluate(&present, &ctx).unwrap().branch, Branch::Yes);
    let absent = json!({ "json_path": "$.meta.missing", "operator": "exists" });
    assert_eq!(p.evaluate(&absent, &ctx).unwrap().branch, Branch::No);
}

#[test]
fn exists_matches_any_of_multiple_nodes() {
    let p = JsonExpressionProcessor;
    let ctx = ctx_json(r#"{"items":[{"id":1},{"id":2}]}"#);
    let cfg = json!({ "json_path": "$.items[*].id", "operator": "exists" });
    assert_eq!(p.evaluate(&cfg, &ctx).unwrap().branch, Branch::Yes);
    // equals across the matched set: any node matches -> Yes.
    let eq = json!({ "json_path": "$.items[*].id", "operator": "equals", "value": "2" });
    assert_eq!(p.evaluate(&eq, &ctx).unwrap().branch, Branch::Yes);
}

#[test]
fn no_body_returns_no_not_error() {
    let p = JsonExpressionProcessor;
    let ctx = ctx_html();
    let cfg = json!({ "json_path": "$.type", "operator": "exists" });
    // HTML response: no response_json -> No (never an error).
    assert_eq!(p.evaluate(&cfg, &ctx).unwrap().branch, Branch::No);
}

#[test]
fn bad_path_is_config_error_fail_open() {
    let p = JsonExpressionProcessor;
    let ctx = ctx_json(r#"{"type":"premium"}"#);
    let cfg = json!({ "json_path": "$.[bad(", "operator": "exists" });
    assert!(p.evaluate(&cfg, &ctx).is_err());
}

#[test]
fn missing_required_fields_error() {
    let p = JsonExpressionProcessor;
    let ctx = ctx_json(r#"{"type":"premium"}"#);
    let no_path = json!({ "operator": "exists" });
    assert!(p.evaluate(&no_path, &ctx).is_err());
    let no_op = json!({ "json_path": "$.type" });
    assert!(p.evaluate(&no_op, &ctx).is_err());
}

#[test]
fn unknown_operator_errors() {
    let p = JsonExpressionProcessor;
    let ctx = ctx_json(r#"{"type":"premium"}"#);
    let cfg = json!({ "json_path": "$.type", "operator": "regex", "value": "x" });
    assert!(p.evaluate(&cfg, &ctx).is_err());
}
