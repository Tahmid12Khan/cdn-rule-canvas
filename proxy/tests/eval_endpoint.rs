//! Integration tests for `POST /__rre/eval`.
//!
//! Spins up the full `build_app` stack (no wiremock — the eval endpoint does
//! not fetch from upstream or backend) and sends JSON requests, asserting the
//! correct matched expression node and traversal path for the new
//! start->decision->expression->end shape, plus the §5 Transformation Journey.

use std::sync::Arc;

use rre_proxy::build_app;
use rre_proxy::config::Settings;
use rre_proxy::domain::processors::default_registry;
use rre_proxy::infra::backend_client::BackendClient;
use rre_proxy::infra::compiled_cache::CompiledCache;
use rre_proxy::infra::feature_map::{FeatureMap, FeatureMapEntry};
use rre_proxy::state::AppState;
use serde_json::{json, Value};

/// The worked-example canvas in the new start->decision->expression->end shape.
fn canvas() -> Value {
    json!({
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
    })
}

async fn spawn_app() -> String {
    let settings = Settings {
        proxy_bind_addr: "127.0.0.1:0".to_string(),
        upstream_base_url: "http://127.0.0.1:1".to_string(),
        backend_base_url: "http://127.0.0.1:1".to_string(),
        app_env: "dev".to_string(),
        active_version_ttl_secs: 30,
        compiled_cache_capacity: 256,
        upstream_connect_timeout_secs: 2,
        upstream_read_timeout_secs: 10,
        feature_map_path: "config/feature_map.yaml".to_string(),
        sanitizer_config_path: "config/sanitizer.yaml".to_string(),
    };
    let http = reqwest::Client::new();
    let feature_map = FeatureMap::from_entries(vec![FeatureMapEntry {
        host: "rre.test".to_string(),
        path_glob: "/article*".to_string(),
        feature_id: "dn-article".to_string(),
    }])
    .unwrap();
    let backend = BackendClient::new(http.clone(), settings.backend_base_url.clone(), 30);
    let sanitizer =
        rre_proxy::domain::applier::html_sanitizer::load_sanitizer("config/sanitizer.yaml")
            .unwrap();

    let state = AppState {
        settings: Arc::new(settings),
        http,
        feature_map: Arc::new(feature_map),
        backend: Arc::new(backend),
        compiled: Arc::new(CompiledCache::new(256)),
        registry: Arc::new(default_registry()),
        sanitizer: Arc::new(sanitizer),
    };

    let app = build_app(state);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    format!("http://{addr}")
}

/// Branch A: paywall=true + desktop -> n_meta:yes -> n_dev:no -> n_paywall
#[tokio::test]
async fn paywall_desktop_routes_to_paywall_node() {
    let base = spawn_app().await;
    let client = reqwest::Client::new();

    let body = json!({
        "canvas": canvas(),
        "context": {
            "device_type": "desktop",
            "meta_tags": { "paywall": "true" }
        }
    });

    let resp = client
        .post(format!("{base}/__rre/eval"))
        .json(&body)
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 200);
    let v: Value = resp.json().await.unwrap();

    assert_eq!(v["matched_node_id"].as_str(), Some("n_paywall"));

    let nodes = v["traversed_node_ids"].as_array().unwrap();
    assert!(nodes.contains(&json!("n_meta")));
    assert!(nodes.contains(&json!("n_dev")));
    assert!(nodes.contains(&json!("n_paywall")));
    assert!(!nodes.contains(&json!("n_regwall")));

    let edges = v["traversed_edge_ids"].as_array().unwrap();
    assert!(edges.contains(&json!("e1")), "expected e1 (meta yes)");
    assert!(
        edges.contains(&json!("e4")),
        "expected e4 (dev no -> paywall)"
    );
    assert!(
        !edges.contains(&json!("e3")),
        "e3 (regwall) must not be taken"
    );

    let steps = v["steps"].as_array().unwrap();
    let meta_step = steps.iter().find(|s| s["node_id"] == "n_meta").unwrap();
    assert_eq!(meta_step["branch"].as_str(), Some("yes"));
    let dev_step = steps.iter().find(|s| s["node_id"] == "n_dev").unwrap();
    assert_eq!(dev_step["branch"].as_str(), Some("no"));

    // Transformation Journey: start, n_meta, n_dev, n_paywall, end.
    let journey = v["journey"].as_array().unwrap();
    let kinds: Vec<&str> = journey
        .iter()
        .map(|e| e["kind"].as_str().unwrap())
        .collect();
    assert_eq!(kinds.first(), Some(&"start"));
    assert_eq!(kinds.last(), Some(&"end"));
    assert!(journey.iter().any(|e| e["node_id"] == "n_paywall"));
}

/// Branch B: paywall=true + mobile -> n_meta:yes -> n_dev:yes -> n_regwall
#[tokio::test]
async fn paywall_mobile_routes_to_regwall_node() {
    let base = spawn_app().await;
    let client = reqwest::Client::new();

    let body = json!({
        "canvas": canvas(),
        "context": {
            "device_type": "mobile",
            "meta_tags": { "paywall": "true" }
        }
    });

    let resp = client
        .post(format!("{base}/__rre/eval"))
        .json(&body)
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 200);
    let v: Value = resp.json().await.unwrap();

    assert_eq!(v["matched_node_id"].as_str(), Some("n_regwall"));

    let nodes = v["traversed_node_ids"].as_array().unwrap();
    assert!(nodes.contains(&json!("n_meta")));
    assert!(nodes.contains(&json!("n_dev")));
    assert!(nodes.contains(&json!("n_regwall")));
    assert!(!nodes.contains(&json!("n_paywall")));

    let edges = v["traversed_edge_ids"].as_array().unwrap();
    assert!(edges.contains(&json!("e1")), "expected e1 (meta yes)");
    assert!(
        edges.contains(&json!("e3")),
        "expected e3 (dev yes -> regwall)"
    );
    assert!(
        !edges.contains(&json!("e4")),
        "e4 (paywall) must not be taken"
    );
}

/// No paywall tag -> n_meta:no -> n_content (skip the device branch entirely).
#[tokio::test]
async fn no_paywall_routes_to_content_node() {
    let base = spawn_app().await;
    let client = reqwest::Client::new();

    let body = json!({
        "canvas": canvas(),
        "context": {
            "device_type": "desktop",
            "meta_tags": {}
        }
    });

    let resp = client
        .post(format!("{base}/__rre/eval"))
        .json(&body)
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 200);
    let v: Value = resp.json().await.unwrap();

    assert_eq!(v["matched_node_id"].as_str(), Some("n_content"));

    let nodes = v["traversed_node_ids"].as_array().unwrap();
    assert!(nodes.contains(&json!("n_meta")));
    // Device node should NOT appear: the meta:no branch skips it.
    assert!(!nodes.contains(&json!("n_dev")));

    let edges = v["traversed_edge_ids"].as_array().unwrap();
    assert!(
        edges.contains(&json!("e2")),
        "expected e2 (meta no -> content)"
    );
}

/// Bad JSON (missing `canvas` field) -> 422 (axum JSON extractor).
#[tokio::test]
async fn missing_canvas_returns_422() {
    let base = spawn_app().await;
    let client = reqwest::Client::new();

    let resp = client
        .post(format!("{base}/__rre/eval"))
        .header("content-type", "application/json")
        .body(r#"{"context":{"device_type":"desktop"}}"#)
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 422);
}

/// A JSON canvas with a `json_expression` decision routing to a `trim_json` +
/// `add_attribute` chain (spec §7 worked example), proving the §5 journey replays
/// the body through the expression actions.
fn json_canvas() -> Value {
    json!({
        "root_node_id": "start",
        "nodes": [
            { "kind": "start", "id": "start", "position": { "x": -200.0, "y": 0.0 } },
            { "kind": "decision", "id": "d_api",
              "processor": { "type": "json_expression", "json_path": "$.api", "operator": "equals", "value": "dn-article" },
              "position": { "x": 0.0, "y": 0.0 } },
            { "kind": "expression", "id": "t_body",
              "action": { "type": "trim_json", "json_path": "$.body", "length": 0 },
              "position": { "x": 200.0, "y": 0.0 } },
            { "kind": "expression", "id": "a_pw",
              "action": { "type": "add_attribute", "json_path": "$.paywall_show", "value": "<html>paywall_showed</html>" },
              "position": { "x": 400.0, "y": 0.0 } },
            { "kind": "end", "id": "end", "position": { "x": 600.0, "y": 100.0 } }
        ],
        "edges": [
            { "id": "e0", "source_node_id": "start",  "target_node_id": "d_api",  "branch": "yes" },
            { "id": "e1", "source_node_id": "d_api",  "target_node_id": "t_body", "branch": "yes" },
            { "id": "e2", "source_node_id": "d_api",  "target_node_id": "end",    "branch": "no"  },
            { "id": "e3", "source_node_id": "t_body", "target_node_id": "a_pw",   "branch": "yes" },
            { "id": "e4", "source_node_id": "a_pw",   "target_node_id": "end",    "branch": "yes" }
        ]
    })
}

/// §5: the journey replays the matched JSON path's body mutations step by step.
#[tokio::test]
async fn json_journey_replays_body_mutations() {
    let base = spawn_app().await;
    let client = reqwest::Client::new();

    let body = json!({
        "canvas": json_canvas(),
        "context": {
            "content_kind": "json",
            "response_json": { "api": "dn-article", "body": [1, 2, 3] }
        }
    });

    let resp = client
        .post(format!("{base}/__rre/eval"))
        .json(&body)
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let v: Value = resp.json().await.unwrap();

    assert_eq!(v["matched_node_id"].as_str(), Some("a_pw"));

    let journey = v["journey"].as_array().unwrap();
    // start, d_api, t_body, a_pw, end.
    assert_eq!(journey.len(), 5, "journey: {journey:?}");
    assert_eq!(journey[0]["kind"], "start");
    assert_eq!(journey[0]["body_after"]["body"], json!([1, 2, 3]));

    // After d_api (decision): body unchanged.
    assert_eq!(journey[1]["node_id"], "d_api");
    assert_eq!(journey[1]["body_after"]["body"], json!([1, 2, 3]));

    // After t_body (trim to 0): body array emptied.
    assert_eq!(journey[2]["node_id"], "t_body");
    assert_eq!(journey[2]["body_after"]["body"], json!([]));

    // After a_pw (add_attribute): paywall_show set.
    assert_eq!(journey[3]["node_id"], "a_pw");
    assert_eq!(
        journey[3]["body_after"]["paywall_show"],
        json!("<html>paywall_showed</html>")
    );

    // End: body unchanged from the previous step.
    assert_eq!(journey[4]["kind"], "end");
    assert_eq!(journey[4]["body_after"]["body"], json!([]));
}

/// Spec §5/§8: every journey step carries a `time_ms` `d.dd` string (start /
/// decision / end == "0.00") and the response carries a `summary` built exactly
/// like one feature's `features_matched` entry for the canvas under test.
#[tokio::test]
async fn json_eval_has_summary_and_per_step_time() {
    let base = spawn_app().await;
    let client = reqwest::Client::new();

    let body = json!({
        "canvas": json_canvas(),
        "context": {
            "content_kind": "json",
            "response_json": { "api": "dn-article", "body": [1, 2, 3] }
        }
    });

    let resp = client
        .post(format!("{base}/__rre/eval"))
        .json(&body)
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let v: Value = resp.json().await.unwrap();

    // Every journey step has a `time_ms` formatted `d.dd`.
    let journey = v["journey"].as_array().unwrap();
    for step in journey {
        let t = step["time_ms"].as_str().expect("time_ms is a string");
        assert!(is_d_dd(t), "journey time_ms should be d.dd, got {t:?}");
    }
    // Non-expression steps (start / decision / end) are exactly "0.00".
    for step in journey {
        if step["kind"] != "expression" {
            assert_eq!(
                step["time_ms"],
                json!("0.00"),
                "non-expression step: {step}"
            );
        }
    }

    // Summary: built like a feature entry (v2.2 shape).
    let summary = &v["summary"];
    let expressions = summary["expressions"].as_array().unwrap();
    let ids: Vec<&str> = expressions
        .iter()
        .map(|e| e["expression_id"].as_str().unwrap())
        .collect();
    assert_eq!(ids, vec!["t_body", "a_pw"]);
    let labels: Vec<&str> = expressions
        .iter()
        .map(|e| e["expression_label"].as_str().unwrap())
        .collect();
    assert_eq!(labels, vec!["trim_json", "add_attribute"]);
    for e in expressions {
        assert_eq!(e["custom_expression_label"].as_str(), Some(""));
        let t = e["expression_time_ms"].as_str().unwrap();
        assert!(is_d_dd(t), "expression_time_ms d.dd, got {t:?}");
    }
    let time_took_ms = summary["time_took_ms"].as_str().expect("time_took_ms string");
    assert!(is_d_dd(time_took_ms), "time_took_ms d.dd, got {time_took_ms:?}");

    let expensive = summary["expensive_nodes"].as_array().unwrap();
    assert_eq!(expensive.len(), 2, "two expression nodes");
    let mut prev = f64::INFINITY;
    for n in expensive {
        let t = n["expression_time_ms"].as_str().unwrap();
        assert!(is_d_dd(t), "expression_time_ms d.dd, got {t:?}");
        let parsed: f64 = t.parse().unwrap();
        assert!(parsed <= prev, "expensive_nodes DESC by time");
        prev = parsed;
    }
}

/// Spec §5/§8: a path with no expression node (the `no` branch) yields NO
/// `summary` (omitted) — mirroring an unmatched feature.
#[tokio::test]
async fn json_eval_no_match_omits_summary() {
    let base = spawn_app().await;
    let client = reqwest::Client::new();

    let body = json!({
        "canvas": json_canvas(),
        "context": {
            "content_kind": "json",
            "response_json": { "api": "other", "body": [1, 2, 3] }
        }
    });

    let resp = client
        .post(format!("{base}/__rre/eval"))
        .json(&body)
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let v: Value = resp.json().await.unwrap();
    assert!(
        v.get("summary").is_none(),
        "no summary when no expression matched"
    );
}

/// A value is `d.dd`: digits, a dot, exactly two trailing digits.
fn is_d_dd(s: &str) -> bool {
    match s.split_once('.') {
        Some((int_part, frac)) => {
            !int_part.is_empty()
                && int_part.chars().all(|c| c.is_ascii_digit())
                && frac.len() == 2
                && frac.chars().all(|c| c.is_ascii_digit())
        }
        None => false,
    }
}

/// The `no` branch of the JSON canvas reaches `end` with no actions -> no body
/// mutation in the journey.
#[tokio::test]
async fn json_journey_no_branch_passes_through() {
    let base = spawn_app().await;
    let client = reqwest::Client::new();

    let body = json!({
        "canvas": json_canvas(),
        "context": {
            "content_kind": "json",
            "response_json": { "api": "other", "body": [1, 2, 3] }
        }
    });

    let resp = client
        .post(format!("{base}/__rre/eval"))
        .json(&body)
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let v: Value = resp.json().await.unwrap();

    // No expression node matched.
    assert_eq!(v["matched_node_id"].as_str(), None);

    let journey = v["journey"].as_array().unwrap();
    // start, d_api, end — body unchanged throughout.
    assert!(journey
        .iter()
        .all(|e| e["body_after"]["body"] == json!([1, 2, 3])));
    assert_eq!(journey.last().unwrap()["kind"], "end");
}

/// CORS preflight on `/__rre/eval` should respond 200 with the correct headers.
#[tokio::test]
async fn cors_preflight_returns_allow_origin() {
    let base = spawn_app().await;
    let client = reqwest::Client::new();

    let resp = client
        .request(reqwest::Method::OPTIONS, format!("{base}/__rre/eval"))
        .header("origin", "http://localhost:3000")
        .header("access-control-request-method", "POST")
        .header("access-control-request-headers", "content-type")
        .send()
        .await
        .unwrap();

    assert!(
        resp.status().is_success(),
        "preflight status: {}",
        resp.status()
    );
    let allow_origin = resp
        .headers()
        .get("access-control-allow-origin")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    assert_eq!(
        allow_origin, "http://localhost:3000",
        "CORS allow-origin header"
    );
}
