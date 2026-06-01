use rre_proxy::domain::applier::html_sanitizer::load_sanitizer;
use rre_proxy::domain::applier::placement_sticky_footer;
use rre_proxy::infra::backend_client::{ActiveComponent, Placement};
use serde_json::json;
use uuid::Uuid;

fn sanitizer() -> ammonia::Builder<'static> {
    load_sanitizer("config/sanitizer.yaml").unwrap()
}

fn component() -> ActiveComponent {
    ActiveComponent {
        id: Uuid::parse_str("66666666-6666-6666-6666-666666666666").unwrap(),
        slug: "sf".to_string(),
        r#type: "html_injection".to_string(),
        config: json!({ "html_body": "<p>Footer notice</p>" }),
        placement: Placement::StickyFooter,
        order_index: 0,
    }
}

#[test]
fn appends_footer_and_style_once() {
    let s = sanitizer();
    let html = "<html><head></head><body><main></main></body></html>";
    let out = placement_sticky_footer::render(html, &component(), &s).unwrap();
    assert!(out.contains("rre-sticky-footer"));
    assert!(out.contains("Footer notice"));
    assert!(out.contains(r#"data-rre="sticky-footer""#));
}

#[test]
fn is_idempotent() {
    let s = sanitizer();
    let html = "<html><head></head><body><main></main></body></html>";
    let once = placement_sticky_footer::render(html, &component(), &s).unwrap();
    let twice = placement_sticky_footer::render(&once, &component(), &s).unwrap();
    assert_eq!(once, twice);
    assert_eq!(once.matches("Footer notice").count(), 1);
}
