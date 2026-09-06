use rre_proxy::domain::applier::html_sanitizer::load_sanitizer;
use rre_proxy::domain::applier::json_apply::ResolvedComponentMap;
use rre_proxy::domain::applier::orchestrator::apply_outcome;
use rre_proxy::infra::backend_client::{ActiveComponent, ActiveOutcome, Placement};
use serde_json::json;
use uuid::Uuid;

fn sanitizer() -> ammonia::Builder<'static> {
    load_sanitizer("config/sanitizer.yaml").unwrap()
}

/// Empty pre-resolved map for tests that exercise the built-in component types
/// (no `component_ref` to resolve).
fn no_components() -> ResolvedComponentMap {
    ResolvedComponentMap::new()
}

fn inline(id: &str, order: i32) -> ActiveComponent {
    ActiveComponent {
        id: Uuid::parse_str(id).unwrap(),
        slug: "c".to_string(),
        r#type: "html_injection".to_string(),
        config: json!({
            "type": "html_injection",
            "target_selector": "#article-body",
            "placement_mode": "append",
            "html_body": format!("<p>frag-{order}</p>")
        }),
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

#[test]
fn applies_components_in_order() {
    let s = sanitizer();
    let html = r#"<html><head></head><body><div id="article-body"></div></body></html>"#;
    let o = outcome(vec![
        inline("99999999-9999-9999-9999-999999999999", 1),
        inline("aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa", 0),
    ]);
    let result = apply_outcome(html.to_string(), &o, &no_components(), &s).unwrap();
    assert!(result.applied);
    let pos0 = result.html.find("frag-0").unwrap();
    let pos1 = result.html.find("frag-1").unwrap();
    assert!(pos0 < pos1, "order_index 0 should be applied before 1");
}

#[test]
fn empty_outcome_is_noop() {
    let s = sanitizer();
    let html = "<html><body></body></html>";
    let o = outcome(vec![]);
    let result = apply_outcome(html.to_string(), &o, &no_components(), &s).unwrap();
    assert!(!result.applied);
    assert_eq!(result.html, html);
}

#[test]
fn full_outcome_is_idempotent() {
    let s = sanitizer();
    let html = r#"<html><head></head><body><div id="article-body"></div></body></html>"#;
    let o = outcome(vec![inline("99999999-9999-9999-9999-999999999999", 0)]);
    let once = apply_outcome(html.to_string(), &o, &no_components(), &s).unwrap();
    let twice = apply_outcome(once.html.clone(), &o, &no_components(), &s).unwrap();
    assert_eq!(once.html, twice.html);
}
