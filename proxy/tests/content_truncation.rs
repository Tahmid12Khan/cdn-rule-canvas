use rre_proxy::domain::applier::content_truncation::ContentTruncationRenderer;
use rre_proxy::domain::applier::html_sanitizer::load_sanitizer;
use rre_proxy::domain::applier::ComponentRenderer;
use rre_proxy::infra::backend_client::{ActiveComponent, Placement};
use serde_json::json;
use uuid::Uuid;

fn sanitizer() -> ammonia::Builder<'static> {
    load_sanitizer("config/sanitizer.yaml").unwrap()
}

fn component(word_count: u32, fade_out: bool) -> ActiveComponent {
    ActiveComponent {
        id: Uuid::parse_str("55555555-5555-5555-5555-555555555555").unwrap(),
        slug: "trunc".to_string(),
        r#type: "content_truncation".to_string(),
        config: json!({
            "type": "content_truncation",
            "target_selector": "#article-body",
            "word_count": word_count,
            "fade_out": fade_out
        }),
        placement: Placement::Inline,
        order_index: 0,
    }
}

#[test]
fn truncates_to_word_budget() {
    let r = ContentTruncationRenderer;
    let s = sanitizer();
    let html = r#"<html><head></head><body><div id="article-body">one two three four five six</div></body></html>"#;
    let out = r.render(html, &component(3, false), &s).unwrap();
    assert!(out.contains("one two three"));
    assert!(!out.contains("four five six"));
    assert!(out.contains("data-rre-truncated"));
}

#[test]
fn fade_out_injects_style_once() {
    let r = ContentTruncationRenderer;
    let s = sanitizer();
    let html = r#"<html><head></head><body><div id="article-body">a b c d e</div></body></html>"#;
    let once = r.render(html, &component(2, true), &s).unwrap();
    assert!(once.contains("rre-fade-out"));
    assert_eq!(once.matches(r#"data-rre="content-truncation""#).count(), 1);

    let twice = r.render(&once, &component(2, true), &s).unwrap();
    assert_eq!(once, twice, "truncation must be idempotent");
}
