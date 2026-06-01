use std::collections::HashMap;

use http::HeaderMap;
use rre_proxy::domain::context::EvaluationContextParts;
use rre_proxy::domain::processors::{meta_tags::MetaTagsProcessor, Branch, CanvasProcessor};
use serde_json::json;

fn ctx(html: &str) -> rre_proxy::domain::context::EvaluationContext {
    EvaluationContextParts::from_request(
        &HeaderMap::new(),
        "/",
        &HashMap::new(),
        html.to_string(),
        false,
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
