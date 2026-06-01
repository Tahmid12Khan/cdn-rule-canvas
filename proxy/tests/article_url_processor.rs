use std::collections::HashMap;

use http::HeaderMap;
use rre_proxy::domain::context::EvaluationContextParts;
use rre_proxy::domain::processors::{article_url::ArticleUrlProcessor, Branch, CanvasProcessor};
use serde_json::json;

fn ctx_with_path(path: &str) -> rre_proxy::domain::context::EvaluationContext {
    EvaluationContextParts::from_request(
        &HeaderMap::new(),
        path,
        &HashMap::new(),
        "<html></html>".to_string(),
    )
    .into_context()
}

#[test]
fn equals_matches_exact_path() {
    let p = ArticleUrlProcessor;
    let ctx = ctx_with_path("/news/article.html");
    let cfg = json!({ "type": "article_url", "operator": "equals", "value": "/news/article.html" });
    assert_eq!(p.evaluate(&cfg, &ctx).unwrap().branch, Branch::Yes);
}

#[test]
fn equals_miss_is_no() {
    let p = ArticleUrlProcessor;
    let ctx = ctx_with_path("/news/article.html");
    let cfg = json!({ "type": "article_url", "operator": "equals", "value": "/other.html" });
    assert_eq!(p.evaluate(&cfg, &ctx).unwrap().branch, Branch::No);
}

#[test]
fn contains_matches_substring() {
    let p = ArticleUrlProcessor;
    let ctx = ctx_with_path("/news/article.html");
    let cfg = json!({ "type": "article_url", "operator": "contains", "value": "article" });
    assert_eq!(p.evaluate(&cfg, &ctx).unwrap().branch, Branch::Yes);
}

#[test]
fn starts_with_matches_prefix() {
    let p = ArticleUrlProcessor;
    let ctx = ctx_with_path("/news/article.html");
    let cfg = json!({ "type": "article_url", "operator": "starts_with", "value": "/news/" });
    assert_eq!(p.evaluate(&cfg, &ctx).unwrap().branch, Branch::Yes);
}

#[test]
fn starts_with_miss_is_no() {
    let p = ArticleUrlProcessor;
    let ctx = ctx_with_path("/news/article.html");
    let cfg = json!({ "type": "article_url", "operator": "starts_with", "value": "/blog/" });
    assert_eq!(p.evaluate(&cfg, &ctx).unwrap().branch, Branch::No);
}

#[test]
fn matches_regex_hit() {
    let p = ArticleUrlProcessor;
    let ctx = ctx_with_path("/news/article.html");
    let cfg = json!({ "type": "article_url", "operator": "matches", "value": r"\.html$" });
    assert_eq!(p.evaluate(&cfg, &ctx).unwrap().branch, Branch::Yes);

    let no = json!({ "type": "article_url", "operator": "matches", "value": "^/blog/" });
    assert_eq!(p.evaluate(&no, &ctx).unwrap().branch, Branch::No);
}

#[test]
fn invalid_regex_errors() {
    let p = ArticleUrlProcessor;
    let ctx = ctx_with_path("/news/article.html");
    let cfg = json!({ "type": "article_url", "operator": "matches", "value": "(" });
    assert!(p.evaluate(&cfg, &ctx).is_err());
}

#[test]
fn unknown_operator_errors() {
    let p = ArticleUrlProcessor;
    let ctx = ctx_with_path("/news/article.html");
    let cfg = json!({ "type": "article_url", "operator": "regex", "value": "/news" });
    assert!(p.evaluate(&cfg, &ctx).is_err());
}
