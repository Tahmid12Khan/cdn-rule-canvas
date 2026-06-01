use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;

use http::HeaderMap;
use rre_proxy::domain::context::EvaluationContextParts;
use rre_proxy::domain::evaluator::{GraphEvaluator, MatchedAction};
use rre_proxy::domain::graph::{Canvas, CanvasGraph};
use rre_proxy::domain::processors::default_registry;
use rre_proxy::infra::compiled_cache::CompiledCache;

/// The worked-example anonymous canvas in the new start->decision->expression->end
/// shape. Built from a raw JSON string (not the `json!` macro) so rustfmt leaves
/// the literal untouched. Each expression node applies a distinct outcome id, so
/// the matched `MatchedAction` order encodes the routed path.
const ANONYMOUS_CANVAS_JSON: &str = r#"{
  "root_node_id": "start",
  "nodes": [
    { "kind": "start", "id": "start", "position": { "x": -200.0, "y": 200.0 } },
    { "kind": "decision", "id": "n_meta",
      "processor": { "type": "meta_tags", "tag_name": "paywall", "operator": "contains", "value": "true" },
      "position": { "x": 80.0, "y": 200.0 } },
    { "kind": "decision", "id": "n_dev",
      "processor": { "type": "device_type", "operator": "equals", "value": "mobile" },
      "position": { "x": 360.0, "y": 120.0 } },
    { "kind": "expression", "id": "n_regwall",
      "action": { "type": "apply_outcome", "outcome_id": "11111111-1111-1111-1111-111111111111" },
      "position": { "x": 640.0, "y": 60.0 } },
    { "kind": "expression", "id": "n_paywall",
      "action": { "type": "apply_outcome", "outcome_id": "22222222-2222-2222-2222-222222222222" },
      "position": { "x": 640.0, "y": 200.0 } },
    { "kind": "expression", "id": "n_content",
      "action": { "type": "apply_outcome", "outcome_id": "33333333-3333-3333-3333-333333333333" },
      "position": { "x": 360.0, "y": 320.0 } },
    { "kind": "end", "id": "end", "position": { "x": 900.0, "y": 200.0 } }
  ],
  "edges": [
    { "id": "e0", "source_node_id": "start",     "target_node_id": "n_meta",    "branch": "yes" },
    { "id": "e1", "source_node_id": "n_meta",    "target_node_id": "n_dev",     "branch": "yes" },
    { "id": "e2", "source_node_id": "n_meta",    "target_node_id": "n_content", "branch": "no"  },
    { "id": "e3", "source_node_id": "n_dev",     "target_node_id": "n_regwall", "branch": "yes" },
    { "id": "e4", "source_node_id": "n_dev",     "target_node_id": "n_paywall", "branch": "no"  },
    { "id": "e5", "source_node_id": "n_regwall", "target_node_id": "end",       "branch": "yes" },
    { "id": "e6", "source_node_id": "n_paywall", "target_node_id": "end",       "branch": "yes" },
    { "id": "e7", "source_node_id": "n_content", "target_node_id": "end",       "branch": "yes" }
  ]
}"#;

fn anonymous_canvas() -> CanvasGraph {
    serde_json::from_str(ANONYMOUS_CANVAS_JSON).unwrap()
}

fn parts(html: &str, ua: Option<&str>) -> EvaluationContextParts {
    let mut headers = HeaderMap::new();
    if let Some(ua) = ua {
        headers.insert(http::header::USER_AGENT, ua.parse().unwrap());
    }
    EvaluationContextParts::from_request(
        &headers,
        "/article/1",
        &HashMap::new(),
        html.to_string(),
        false,
    )
}

/// The matched expression node ids (in trace order) extracted from the actions.
fn node_ids(actions: &[MatchedAction]) -> Vec<&str> {
    actions.iter().map(|a| a.node_id.as_str()).collect()
}

/// The `outcome_id` of an `apply_outcome` action (assert routing target).
fn outcome_id(action: &MatchedAction) -> &str {
    action.action["outcome_id"].as_str().unwrap()
}

#[tokio::test]
async fn paywall_plus_mobile_routes_to_regwall() {
    let registry = Arc::new(default_registry());
    let cache = CompiledCache::new(256);
    let evaluator = GraphEvaluator::new(registry, &cache);

    let canvas = anonymous_canvas();
    let html = r#"<html><head><meta name="paywall" content="true"></head><body></body></html>"#;
    let ctx = parts(html, Some("iPhone Mobile"));

    let actions = evaluator
        .evaluate(&canvas, ctx, "dn-article", 1, Canvas::Anonymous)
        .await;

    assert_eq!(node_ids(&actions), vec!["n_regwall"]);
    assert_eq!(
        outcome_id(&actions[0]),
        "11111111-1111-1111-1111-111111111111"
    );
}

#[tokio::test]
async fn paywall_plus_desktop_routes_to_paywall() {
    let registry = Arc::new(default_registry());
    let cache = CompiledCache::new(256);
    let evaluator = GraphEvaluator::new(registry, &cache);

    let canvas = anonymous_canvas();
    let html = r#"<html><head><meta name="paywall" content="true"></head><body></body></html>"#;
    let ctx = parts(html, Some("Macintosh Desktop"));

    let actions = evaluator
        .evaluate(&canvas, ctx, "dn-article", 1, Canvas::Anonymous)
        .await;

    assert_eq!(node_ids(&actions), vec!["n_paywall"]);
    assert_eq!(
        outcome_id(&actions[0]),
        "22222222-2222-2222-2222-222222222222"
    );
}

#[tokio::test]
async fn no_paywall_routes_to_content() {
    let registry = Arc::new(default_registry());
    let cache = CompiledCache::new(256);
    let evaluator = GraphEvaluator::new(registry, &cache);

    let canvas = anonymous_canvas();
    let html = "<html><head></head><body></body></html>";
    let ctx = parts(html, Some("iPhone Mobile"));

    let actions = evaluator
        .evaluate(&canvas, ctx, "dn-article", 1, Canvas::Anonymous)
        .await;

    assert_eq!(node_ids(&actions), vec!["n_content"]);
    assert_eq!(
        outcome_id(&actions[0]),
        "33333333-3333-3333-3333-333333333333"
    );
}

#[tokio::test]
async fn empty_canvas_yields_no_actions() {
    let registry = Arc::new(default_registry());
    let cache = CompiledCache::new(256);
    let evaluator = GraphEvaluator::new(registry, &cache);

    let canvas = CanvasGraph::default();
    let ctx = parts("<html></html>", None);

    let actions = evaluator
        .evaluate(&canvas, ctx, "dn-article", 1, Canvas::Anonymous)
        .await;

    assert!(actions.is_empty());
}

/// Latency smoke: 100 concurrent evaluations should complete with a healthy p95.
/// Each eval translates+compiles once (cache warms) then runs zen in
/// spawn_blocking. We assert p95 < 25ms (generous for CI; budget is < 5ms warm).
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn latency_smoke_100_concurrent() {
    let registry = Arc::new(default_registry());
    let cache = Arc::new(CompiledCache::new(256));
    let canvas = Arc::new(anonymous_canvas());
    let html = r#"<html><head><meta name="paywall" content="true"></head><body></body></html>"#;

    // Warm the compiled cache once.
    {
        let evaluator = GraphEvaluator::new(registry.clone(), &cache);
        let ctx = parts(html, Some("iPhone Mobile"));
        let _ = evaluator
            .evaluate(&canvas, ctx, "dn-article", 1, Canvas::Anonymous)
            .await;
    }

    let mut handles = Vec::new();
    for _ in 0..100 {
        let registry = registry.clone();
        let cache = cache.clone();
        let canvas = canvas.clone();
        handles.push(tokio::spawn(async move {
            let evaluator = GraphEvaluator::new(registry, &cache);
            let ctx = parts(html, Some("iPhone Mobile"));
            let start = Instant::now();
            let _ = evaluator
                .evaluate(&canvas, ctx, "dn-article", 1, Canvas::Anonymous)
                .await;
            start.elapsed().as_secs_f64() * 1000.0
        }));
    }

    let mut samples = Vec::new();
    for h in handles {
        samples.push(h.await.unwrap());
    }
    samples.sort_by(|a, b| a.partial_cmp(b).unwrap());

    let p = |q: f64| samples[((samples.len() as f64 * q) as usize).min(samples.len() - 1)];
    let p50 = p(0.50);
    let p95 = p(0.95);
    let p99 = p(0.99);
    eprintln!("eval latency ms: p50={p50:.3} p95={p95:.3} p99={p99:.3}");

    // Generous CI bound; warm-cache budget is < 5ms p99.
    assert!(p95 < 25.0, "p95 {p95:.3}ms exceeded 25ms");
}
