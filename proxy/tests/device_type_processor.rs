use std::collections::HashMap;

use http::HeaderMap;
use rre_proxy::domain::context::EvaluationContextParts;
use rre_proxy::domain::processors::{device_type::DeviceTypeProcessor, Branch, CanvasProcessor};
use serde_json::json;

fn ctx_with_ua(ua: &str) -> rre_proxy::domain::context::EvaluationContext {
    let mut headers = HeaderMap::new();
    headers.insert(http::header::USER_AGENT, ua.parse().unwrap());
    EvaluationContextParts::from_request(
        &headers,
        "/",
        &HashMap::new(),
        "<html></html>".to_string(),
    )
    .into_context()
}

#[test]
fn mobile_ua_matches_mobile() {
    let p = DeviceTypeProcessor;
    let ctx = ctx_with_ua("Mozilla/5.0 (iPhone; CPU iPhone OS 17_0 like Mac OS X) Mobile");
    let cfg = json!({ "type": "device_type", "operator": "equals", "value": "mobile" });
    assert_eq!(p.evaluate(&cfg, &ctx).unwrap().branch, Branch::Yes);
}

#[test]
fn desktop_ua_does_not_match_mobile() {
    let p = DeviceTypeProcessor;
    let ctx = ctx_with_ua("Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7)");
    let cfg = json!({ "type": "device_type", "operator": "equals", "value": "mobile" });
    assert_eq!(p.evaluate(&cfg, &ctx).unwrap().branch, Branch::No);
}

#[test]
fn tablet_takes_priority_over_mobile() {
    let p = DeviceTypeProcessor;
    let ctx = ctx_with_ua("Mozilla/5.0 (iPad; CPU OS 17_0 like Mac OS X)");
    let cfg = json!({ "type": "device_type", "operator": "equals", "value": "tablet" });
    assert_eq!(p.evaluate(&cfg, &ctx).unwrap().branch, Branch::Yes);
}

#[test]
fn unknown_operator_errors() {
    let p = DeviceTypeProcessor;
    let ctx = ctx_with_ua("x");
    let cfg = json!({ "type": "device_type", "operator": "regex", "value": "mobile" });
    assert!(p.evaluate(&cfg, &ctx).is_err());
}
