use rre_proxy::domain::applier::html_injection::HtmlInjectionRenderer;
use rre_proxy::domain::applier::ComponentRenderer;
use rre_proxy::infra::backend_client::{ActiveComponent, Placement};
use serde_json::json;
use uuid::Uuid;

fn sanitizer() -> ammonia::Builder<'static> {
    rre_core::default_sanitizer()
}

fn component(mode: &str) -> ActiveComponent {
    ActiveComponent {
        id: Uuid::parse_str("44444444-4444-4444-4444-444444444444").unwrap(),
        slug: "inj".to_string(),
        r#type: "html_injection".to_string(),
        config: json!({
            "type": "html_injection",
            "target_selector": "#article-body",
            "placement_mode": mode,
            "html_body": "<p class=\"promo\">Subscribe now</p>"
        }),
        placement: Placement::Inline,
        order_index: 0,
    }
}

#[test]
fn appends_sanitized_html() {
    let r = HtmlInjectionRenderer;
    let s = sanitizer();
    let html = r#"<html><body><div id="article-body"><p>Body</p></div></body></html>"#;
    let out = r.render(html, &component("append"), &s).unwrap();
    assert!(out.contains("Subscribe now"));
    assert!(out.contains("data-rre-injected"));
}

#[test]
fn injection_is_idempotent() {
    let r = HtmlInjectionRenderer;
    let s = sanitizer();
    let html = r#"<html><body><div id="article-body"><p>Body</p></div></body></html>"#;
    let once = r.render(html, &component("append"), &s).unwrap();
    let twice = r.render(&once, &component("append"), &s).unwrap();
    assert_eq!(once, twice, "running twice must equal running once");
    assert_eq!(once.matches("Subscribe now").count(), 1);
}

#[test]
fn rejects_illegal_selector() {
    let r = HtmlInjectionRenderer;
    let s = sanitizer();
    let mut c = component("append");
    c.config = json!({
        "type": "html_injection",
        "target_selector": "#bad)selector{",
        "placement_mode": "append",
        "html_body": "<p>x</p>"
    });
    assert!(r.render("<html></html>", &c, &s).is_err());
}
