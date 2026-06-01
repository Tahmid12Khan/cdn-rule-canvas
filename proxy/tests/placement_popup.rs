use rre_proxy::domain::applier::html_sanitizer::load_sanitizer;
use rre_proxy::domain::applier::placement_popup;
use rre_proxy::infra::backend_client::{ActiveComponent, Placement};
use serde_json::json;
use uuid::Uuid;

fn sanitizer() -> ammonia::Builder<'static> {
    load_sanitizer("config/sanitizer.yaml").unwrap()
}

fn component() -> ActiveComponent {
    ActiveComponent {
        id: Uuid::parse_str("77777777-7777-7777-7777-777777777777").unwrap(),
        slug: "pp".to_string(),
        r#type: "html_injection".to_string(),
        config: json!({ "html_body": "<p>Modal text</p>" }),
        placement: Placement::Popup,
        order_index: 0,
    }
}

#[test]
fn appends_dialog_and_style_once() {
    let s = sanitizer();
    let html = "<html><head></head><body></body></html>";
    let out = placement_popup::render(html, &component(), &s).unwrap();
    assert!(out.contains(r#"role="dialog""#));
    assert!(out.contains("Modal text"));
    assert!(out.contains(r#"data-rre="popup""#));
}

#[test]
fn is_idempotent() {
    let s = sanitizer();
    let html = "<html><head></head><body></body></html>";
    let once = placement_popup::render(html, &component(), &s).unwrap();
    let twice = placement_popup::render(&once, &component(), &s).unwrap();
    assert_eq!(once, twice);
    assert_eq!(once.matches("Modal text").count(), 1);
}
