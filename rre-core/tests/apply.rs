//! End-to-end tests for `rre_core::apply`, the shared feature loop.
//!
//! These build bundles as typed values rather than JSON so a rename breaks the
//! test at compile time. The wire format itself is covered by `golden.rs`,
//! which parses committed JSON.

use std::collections::HashMap;

use rre_core::bundle::Applicability;
use rre_core::edge::{BodyKind, EdgeBundle, EdgeFeature, EdgeSite, RequestFacts, SkipReason};
use rre_core::graph::{Branch, CanvasGraph, Edge, Node, Position, ProcessorRef, RuleGraph};

fn pos() -> Position {
    Position { x: 0.0, y: 0.0 }
}

fn expression(id: &str, action: serde_json::Value) -> Node {
    let kind = action
        .get("type")
        .and_then(|v| v.as_str())
        .expect("action needs a type")
        .to_string();
    Node::Expression {
        id: id.to_string(),
        action: ProcessorRef {
            kind,
            config: action,
        },
        custom_label: None,
        position: pos(),
    }
}

fn edge(id: &str, from: &str, to: &str, branch: Branch) -> Edge {
    Edge {
        id: id.to_string(),
        source_node_id: from.to_string(),
        target_node_id: to.to_string(),
        branch,
    }
}

/// start -> expression -> end.
fn linear_canvas(action: serde_json::Value) -> CanvasGraph {
    CanvasGraph {
        nodes: vec![
            Node::Start {
                id: "s".into(),
                position: pos(),
            },
            expression("e", action),
            Node::End {
                id: "n".into(),
                position: pos(),
            },
        ],
        edges: vec![
            edge("x1", "s", "e", Branch::Yes),
            edge("x2", "e", "n", Branch::Yes),
        ],
        root_node_id: Some("s".into()),
    }
}

/// start -> decision -> (yes) expression -> end. The `no` branch dead-ends.
fn decision_canvas(processor: ProcessorRef, action: serde_json::Value) -> CanvasGraph {
    CanvasGraph {
        nodes: vec![
            Node::Start {
                id: "s".into(),
                position: pos(),
            },
            Node::Decision {
                id: "d".into(),
                processor,
                position: pos(),
            },
            expression("e", action),
            Node::End {
                id: "n".into(),
                position: pos(),
            },
        ],
        edges: vec![
            edge("x1", "s", "d", Branch::Yes),
            edge("x2", "d", "e", Branch::Yes),
            edge("x3", "e", "n", Branch::Yes),
        ],
        root_node_id: Some("s".into()),
    }
}

fn feature(id: &str, kind: &str, canvas: CanvasGraph) -> EdgeFeature {
    EdgeFeature {
        id: id.to_string(),
        r#type: kind.to_string(),
        execution_order: 1,
        version_number: 1,
        applicability: Applicability::default(),
        rule_graph: RuleGraph { canvas },
        outcomes: Vec::new(),
        resolved_components: HashMap::new(),
        saved_outcomes: HashMap::new(),
    }
}

fn bundle(features: Vec<EdgeFeature>) -> EdgeBundle {
    EdgeBundle {
        schema_version: rre_core::edge::SCHEMA_VERSION,
        site: EdgeSite {
            slug: "intrafish-com".into(),
            source_host: "test.intrafish.com".into(),
        },
        environment: "live".into(),
        generated_at: "2026-09-07T00:00:00Z".into(),
        features,
    }
}

fn facts() -> RequestFacts {
    RequestFacts {
        headers: http::HeaderMap::new(),
        path: "/proxy/global/v2/content/2-1-1742531".into(),
        cookies: HashMap::new(),
        site: Some("intrafish-com".into()),
        identity: rre_core::identity::Identity::default(),
    }
}

fn run(
    b: &EdgeBundle,
    f: &RequestFacts,
    kind: BodyKind,
    body: &str,
) -> rre_core::edge::ApplyOutcome {
    rre_core::apply(b, f, kind, body.to_string(), &rre_core::default_sanitizer())
}

/// THE risk this whole design rests on: zen's `evaluate` is async and
/// zen-engine depends on tokio, but `rre-core` drives it with
/// `futures::executor::block_on` and NO runtime installed, because a Fastly
/// Compute guest has neither tokio nor threads. If this test panics with "no
/// reactor running", edge evaluation is impossible as designed.
#[test]
fn evaluates_with_no_tokio_runtime_installed() {
    let b = bundle(vec![feature(
        "set-access",
        "json",
        linear_canvas(
            serde_json::json!({"type": "add_attribute", "json_path": "$.access", "value": "granted"}),
        ),
    )]);

    let out = run(&b, &facts(), BodyKind::Json, r#"{"id":"a1"}"#);

    assert!(out.changed, "the action must change the body");
    let v: serde_json::Value = serde_json::from_str(&out.body).unwrap();
    assert_eq!(v["access"], serde_json::json!("granted"));
    assert_eq!(out.features.len(), 1);
    assert!(out.features[0].matched);
}

/// Features of the other content kind are not run and not reported: an html
/// feature must never touch an article JSON response.
#[test]
fn skips_features_of_the_other_kind() {
    let b = bundle(vec![feature(
        "inject",
        "html",
        linear_canvas(serde_json::json!({"type": "add_attribute", "json_path": "$.x", "value": 1})),
    )]);

    let out = run(&b, &facts(), BodyKind::Json, r#"{"id":"a1"}"#);

    assert!(!out.changed);
    assert!(out.features.is_empty());
}

/// A malformed body must come back byte-identical rather than panicking. This
/// crate runs where a panic is a reader-visible 500.
#[test]
fn never_panics_on_a_malformed_body() {
    let b = bundle(vec![feature(
        "set-access",
        "json",
        linear_canvas(
            serde_json::json!({"type": "add_attribute", "json_path": "$.access", "value": "granted"}),
        ),
    )]);

    for body in [
        "",
        "{",
        "null",
        "[]",
        "\u{feff}not json at all",
        "\u{0}\u{1}",
    ] {
        let out = run(&b, &facts(), BodyKind::Json, body);
        assert_eq!(out.body, body, "malformed body must pass through unchanged");
        assert!(!out.changed);
    }
}

/// Two features chain: the second sees the first's output. Bundle order is the
/// execution order and is decided by the exporter, not re-derived here.
#[test]
fn chains_features_in_bundle_order() {
    let mut first = feature(
        "first",
        "json",
        linear_canvas(serde_json::json!({"type": "add_attribute", "json_path": "$.a", "value": 1})),
    );
    first.execution_order = 1;
    let mut second = feature(
        "second",
        "json",
        linear_canvas(serde_json::json!({"type": "add_attribute", "json_path": "$.b", "value": 2})),
    );
    second.execution_order = 2;

    let out = run(&bundle(vec![first, second]), &facts(), BodyKind::Json, "{}");

    let v: serde_json::Value = serde_json::from_str(&out.body).unwrap();
    assert_eq!(v["a"], serde_json::json!(1));
    assert_eq!(v["b"], serde_json::json!(2));
    assert_eq!(out.features.len(), 2);
    assert!(out.features.iter().all(|f| f.matched));
}

/// The applicability gate stops a feature before evaluation and says so.
#[test]
fn reports_an_applicability_skip() {
    let mut f = feature(
        "gated",
        "json",
        linear_canvas(serde_json::json!({"type": "add_attribute", "json_path": "$.a", "value": 1})),
    );
    f.applicability = Applicability {
        html_selector: None,
        json_selector: Some("$.nothing_here".into()),
    };

    let out = run(&bundle(vec![f]), &facts(), BodyKind::Json, r#"{"id":"a1"}"#);

    assert!(!out.changed);
    assert_eq!(out.body, r#"{"id":"a1"}"#);
    assert_eq!(
        out.features[0].skipped_reason,
        Some(SkipReason::Applicability)
    );
}

/// Identity is the whole personalisation contract at the edge. The same bundle
/// must decide differently for a visitor holding the product.
#[test]
fn has_product_routes_on_identity() {
    let canvas = decision_canvas(
        ProcessorRef {
            kind: "has_product".into(),
            config: serde_json::json!({"type": "has_product", "product": "ifcofa"}),
        },
        serde_json::json!({"type": "add_attribute", "json_path": "$.entitled", "value": true}),
    );
    let b = bundle(vec![feature("paywall", "json", canvas)]);

    let anon = run(&b, &facts(), BodyKind::Json, r#"{"id":"a1"}"#);
    assert!(
        !anon.changed,
        "a visitor without the product must not be entitled"
    );
    assert_eq!(anon.features[0].skipped_reason, Some(SkipReason::NoMatch));

    let mut subscriber = facts();
    subscriber.identity = rre_core::identity::Identity {
        logged_in: true,
        products: ["ifcofa".to_string()].into_iter().collect(),
    };
    let held = run(&b, &subscriber, BodyKind::Json, r#"{"id":"a1"}"#);
    assert!(
        held.changed,
        "a visitor holding the product must be entitled"
    );
    let v: serde_json::Value = serde_json::from_str(&held.body).unwrap();
    assert_eq!(v["entitled"], serde_json::json!(true));
}

/// An HTML body flows through the html action path and is rewritten in place.
#[test]
fn applies_an_html_feature() {
    let canvas = linear_canvas(serde_json::json!({
        "type": "trim_json", "json_path": "$.body", "length": 3
    }));
    // trim_json is a JSON action; on an HTML body it must be a no-op, not a
    // crash — the applier's own cross-kind guard.
    let out = run(
        &bundle(vec![feature("t", "html", canvas)]),
        &facts(),
        BodyKind::Html,
        "<div id=dn-content-ssr><p>a</p></div>",
    );
    assert_eq!(out.body, "<div id=dn-content-ssr><p>a</p></div>");
}

/// An empty bundle is valid and does nothing. This is what a site with no
/// published features exports, and it must not be mistaken for an error.
#[test]
fn an_empty_bundle_is_a_no_op() {
    let out = run(&bundle(vec![]), &facts(), BodyKind::Json, r#"{"id":"a1"}"#);
    assert!(!out.changed);
    assert_eq!(out.body, r#"{"id":"a1"}"#);
    assert!(out.features.is_empty());
}
