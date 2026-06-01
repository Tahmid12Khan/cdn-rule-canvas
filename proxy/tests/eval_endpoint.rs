//! Integration tests for `POST /__rre/eval`.
//!
//! Spins up the full `build_app` stack (no wiremock — the eval endpoint does
//! not fetch from upstream or backend) and sends JSON requests, asserting the
//! correct outcome ids and traversal paths for both test branches of the
//! dn-article anonymous canvas.

use std::sync::Arc;

use rre_proxy::build_app;
use rre_proxy::config::Settings;
use rre_proxy::domain::processors::default_registry;
use rre_proxy::infra::backend_client::BackendClient;
use rre_proxy::infra::compiled_cache::CompiledCache;
use rre_proxy::infra::feature_map::{FeatureMap, FeatureMapEntry};
use rre_proxy::state::AppState;
use serde_json::{json, Value};

/// The worked-example canvas shared with `evaluator.rs` integration tests.
fn canvas() -> Value {
    json!({
        "root_node_id": "n_meta",
        "nodes": [
            { "kind": "decision", "id": "n_meta",
              "processor": { "type": "meta_tags", "tag_name": "paywall", "operator": "contains", "value": "true" },
              "position": { "x": 80.0, "y": 200.0 } },
            { "kind": "decision", "id": "n_dev",
              "processor": { "type": "device_type", "operator": "equals", "value": "mobile" },
              "position": { "x": 360.0, "y": 120.0 } },
            { "kind": "outcome", "id": "n_regwall",
              "outcome_id": "11111111-1111-1111-1111-111111111111",
              "position": { "x": 640.0, "y": 60.0 } },
            { "kind": "outcome", "id": "n_paywall",
              "outcome_id": "22222222-2222-2222-2222-222222222222",
              "position": { "x": 640.0, "y": 200.0 } },
            { "kind": "outcome", "id": "n_content",
              "outcome_id": "33333333-3333-3333-3333-333333333333",
              "position": { "x": 360.0, "y": 320.0 } }
        ],
        "edges": [
            { "id": "e1", "source_node_id": "n_meta", "target_node_id": "n_dev",     "branch": "yes" },
            { "id": "e2", "source_node_id": "n_meta", "target_node_id": "n_content", "branch": "no"  },
            { "id": "e3", "source_node_id": "n_dev",  "target_node_id": "n_regwall", "branch": "yes" },
            { "id": "e4", "source_node_id": "n_dev",  "target_node_id": "n_paywall", "branch": "no"  }
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
/// Expected outcome: 22222222-…
#[tokio::test]
async fn paywall_desktop_routes_to_paywall_outcome() {
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

    assert_eq!(
        v["matched_outcome_id"].as_str(),
        Some("22222222-2222-2222-2222-222222222222"),
        "expected paywall outcome"
    );
    assert_eq!(v["matched_node_id"].as_str(), Some("n_paywall"));

    let nodes = v["traversed_node_ids"].as_array().unwrap();
    assert!(nodes.contains(&json!("n_meta")));
    assert!(nodes.contains(&json!("n_dev")));
    assert!(nodes.contains(&json!("n_paywall")));
    assert!(!nodes.contains(&json!("n_regwall")));

    let edges = v["traversed_edge_ids"].as_array().unwrap();
    // e1 = n_meta:yes -> n_dev; e4 = n_dev:no -> n_paywall
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
}

/// Branch B: paywall=true + mobile -> n_meta:yes -> n_dev:yes -> n_regwall
/// Expected outcome: 11111111-…
#[tokio::test]
async fn paywall_mobile_routes_to_regwall_outcome() {
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

    assert_eq!(
        v["matched_outcome_id"].as_str(),
        Some("11111111-1111-1111-1111-111111111111"),
        "expected regwall outcome"
    );
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
async fn no_paywall_routes_to_content_outcome() {
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

    assert_eq!(
        v["matched_outcome_id"].as_str(),
        Some("33333333-3333-3333-3333-333333333333"),
        "expected show-content outcome"
    );
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

/// Bad JSON (missing `canvas` field) -> 400 with `error` key.
#[tokio::test]
async fn missing_canvas_returns_400() {
    let base = spawn_app().await;
    let client = reqwest::Client::new();

    let resp = client
        .post(format!("{base}/__rre/eval"))
        .header("content-type", "application/json")
        .body(r#"{"context":{"device_type":"desktop"}}"#)
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 422); // axum JSON extractor returns 422 for missing fields
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

    // tower-http CorsLayer returns 200 for valid preflights.
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
