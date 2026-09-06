use std::collections::HashMap;

use http::HeaderMap;
use rre_proxy::domain::context::EvaluationContextParts;
use rre_proxy::domain::identity::Identity;
use rre_proxy::domain::processors::{meta_tags::MetaTagsProcessor, Branch, CanvasProcessor};
use serde_json::json;

fn ctx(html: &str) -> rre_proxy::domain::context::EvaluationContext {
    EvaluationContextParts::from_request(
        &HeaderMap::new(),
        "/",
        &HashMap::new(),
        html.to_string(),
        false,
        Identity::default(),
    )
    .into_context()
}

#[test]
fn exists_operator() {
    let p = MetaTagsProcessor;
    let html = r#"<html><head><meta name="paywall" content="true"></head><body></body></html>"#;
    let cfg = json!({ "type": "meta_tags", "tag_name": "paywall", "operator": "exists" });
    let out = p.evaluate(&cfg, &ctx(html)).unwrap();
    assert_eq!(out.branch, Branch::Yes);

    let out = p.evaluate(&cfg, &ctx("<html></html>")).unwrap();
    assert_eq!(out.branch, Branch::No);
}

#[test]
fn equals_and_contains() {
    let p = MetaTagsProcessor;
    let html = r#"<meta name="paywall" content="hard-true">"#;

    let eq = json!({
        "type": "meta_tags", "tag_name": "paywall", "operator": "equals", "value": "hard-true"
    });
    assert_eq!(p.evaluate(&eq, &ctx(html)).unwrap().branch, Branch::Yes);

    let eq_no = json!({
        "type": "meta_tags", "tag_name": "paywall", "operator": "equals", "value": "true"
    });
    assert_eq!(p.evaluate(&eq_no, &ctx(html)).unwrap().branch, Branch::No);

    let contains = json!({
        "type": "meta_tags", "tag_name": "paywall", "operator": "contains", "value": "true"
    });
    assert_eq!(
        p.evaluate(&contains, &ctx(html)).unwrap().branch,
        Branch::Yes
    );
}

#[test]
fn rejects_oversized_or_illegal_selector() {
    let p = MetaTagsProcessor;
    let html = "<html></html>";
    let big = "a".repeat(300);
    let cfg = json!({ "type": "meta_tags", "tag_name": big, "operator": "exists" });
    assert!(p.evaluate(&cfg, &ctx(html)).is_err());

    let illegal = json!({ "type": "meta_tags", "tag_name": "pay)wall{", "operator": "exists" });
    assert!(p.evaluate(&illegal, &ctx(html)).is_err());
}

#[test]
fn unknown_operator_errors() {
    let p = MetaTagsProcessor;
    let cfg = json!({ "type": "meta_tags", "tag_name": "x", "operator": "startsWith" });
    assert!(p.evaluate(&cfg, &ctx("<html></html>")).is_err());
}

/// M5: when `needs_meta_tags` is false the HTML DOM is NOT parsed and `meta_tags`
/// stays empty even though the body carries `<meta>` tags — the gate the evaluator
/// uses to skip the parse for canvases without a meta_tags node.
#[test]
fn needs_meta_tags_false_skips_parse() {
    let html = r#"<html><head><meta name="paywall" content="true"></head><body></body></html>"#;
    let mut parts = EvaluationContextParts::from_request(
        &HeaderMap::new(),
        "/",
        &HashMap::new(),
        html.to_string(),
        false,
        Identity::default(),
    );
    parts.needs_meta_tags = false;
    let ctx = parts.into_context();
    assert!(
        ctx.meta_tags.is_empty(),
        "gated parse must leave meta_tags empty"
    );

    // The default (true) path still extracts them.
    let parts = EvaluationContextParts::from_request(
        &HeaderMap::new(),
        "/",
        &HashMap::new(),
        html.to_string(),
        false,
        Identity::default(),
    );
    let ctx = parts.into_context();
    assert_eq!(
        ctx.meta_tags.get("paywall").map(String::as_str),
        Some("true")
    );
}
