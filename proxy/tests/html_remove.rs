use rre_proxy::domain::applier::html_remove::HtmlRemoveRenderer;
use rre_proxy::domain::applier::ComponentRenderer;
use rre_proxy::infra::backend_client::{ActiveComponent, Placement};
use serde_json::json;
use uuid::Uuid;

fn sanitizer() -> ammonia::Builder<'static> {
    rre_core::default_sanitizer()
}

fn component(target_selector: &str, include_selector: bool) -> ActiveComponent {
    ActiveComponent {
        id: Uuid::parse_str("66666666-6666-6666-6666-666666666666").unwrap(),
        slug: "remove".to_string(),
        r#type: "html_remove".to_string(),
        config: json!({
            "type": "html_remove",
            "target_selector": target_selector,
            "include_selector": include_selector
        }),
        placement: Placement::Inline,
        order_index: 0,
    }
}

const PAGE: &str = r#"<html><head></head><body><div id="dn-content-ssr"><p>Paid words</p></div><footer>keep</footer></body></html>"#;

#[test]
fn empties_the_target_and_injects_nothing() {
    let out = HtmlRemoveRenderer
        .render(PAGE, &component("#dn-content-ssr", false), &sanitizer())
        .unwrap();

    assert!(out.contains(r#"<div id="dn-content-ssr"></div>"#));
    assert!(!out.contains("Paid words"));
    // Nothing is added: no marker wrapper, no injected element.
    assert!(!out.contains("data-rre-injected"));
    assert!(out.contains("<footer>keep</footer>"));
}

#[test]
fn include_selector_removes_the_element_too() {
    let out = HtmlRemoveRenderer
        .render(PAGE, &component("#dn-content-ssr", true), &sanitizer())
        .unwrap();

    assert!(!out.contains("dn-content-ssr"));
    assert!(!out.contains("Paid words"));
    assert!(out.contains("<footer>keep</footer>"));
}

#[test]
fn defaults_to_keeping_the_element_when_flag_absent() {
    let mut c = component("#dn-content-ssr", false);
    c.config = json!({ "type": "html_remove", "target_selector": "#dn-content-ssr" });

    let out = HtmlRemoveRenderer.render(PAGE, &c, &sanitizer()).unwrap();

    assert!(out.contains(r#"<div id="dn-content-ssr"></div>"#));
}

#[test]
fn is_idempotent() {
    let s = sanitizer();
    for include_selector in [false, true] {
        let c = component("#dn-content-ssr", include_selector);
        let once = HtmlRemoveRenderer.render(PAGE, &c, &s).unwrap();
        let twice = HtmlRemoveRenderer.render(&once, &c, &s).unwrap();
        assert_eq!(once, twice);
    }
}

#[test]
fn no_match_leaves_the_page_untouched() {
    let out = HtmlRemoveRenderer
        .render(PAGE, &component("#nope", true), &sanitizer())
        .unwrap();
    assert!(out.contains("Paid words"));
}

#[test]
fn rejects_an_injection_style_selector() {
    let err = HtmlRemoveRenderer.render(PAGE, &component("#x{}</style>", true), &sanitizer());
    assert!(err.is_err());
}

#[test]
fn missing_target_selector_is_an_error() {
    let mut c = component("#dn-content-ssr", false);
    c.config = json!({ "type": "html_remove", "include_selector": true });
    assert!(HtmlRemoveRenderer.render(PAGE, &c, &sanitizer()).is_err());
}
