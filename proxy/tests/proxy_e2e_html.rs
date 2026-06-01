//! Full pipeline e2e: a fake upstream (wiremock) serves paywalled HTML, a fake
//! backend (wiremock) serves the active-version with a paywall outcome that
//! injects HTML. The proxy should evaluate the anonymous canvas and inject.

use std::sync::Arc;

use rre_proxy::build_app;
use rre_proxy::config::Settings;
use rre_proxy::domain::processors::default_registry;
use rre_proxy::infra::backend_client::BackendClient;
use rre_proxy::infra::compiled_cache::CompiledCache;
use rre_proxy::infra::feature_map::{FeatureMap, FeatureMapEntry};
use rre_proxy::state::AppState;
use serde_json::json;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

const FEATURE: &str = "dn-article";
const HOST: &str = "rre.test";
const PAYWALL_OUTCOME: &str = "22222222-2222-2222-2222-222222222222";

fn active_version_body() -> serde_json::Value {
    // start -> n_meta (paywall?) ; yes -> apply_outcome(paywall) -> end ; no -> end.
    json!({
        "version_number": 1,
        "rule_graph": {
            "anonymous": {
                "root_node_id": "start",
                "nodes": [
                    { "kind": "start", "id": "start", "position": { "x": -200.0, "y": 0.0 } },
                    { "kind": "decision", "id": "n_meta",
                      "processor": { "type": "meta_tags", "tag_name": "paywall", "operator": "contains", "value": "true" },
                      "position": { "x": 0.0, "y": 0.0 } },
                    { "kind": "expression", "id": "n_paywall",
                      "action": { "type": "apply_outcome", "outcome_id": PAYWALL_OUTCOME },
                      "position": { "x": 200.0, "y": 0.0 } },
                    { "kind": "end", "id": "end", "position": { "x": 400.0, "y": 100.0 } }
                ],
                "edges": [
                    { "id": "e0", "source_node_id": "start",     "target_node_id": "n_meta",    "branch": "yes" },
                    { "id": "e1", "source_node_id": "n_meta",    "target_node_id": "n_paywall", "branch": "yes" },
                    { "id": "e2", "source_node_id": "n_meta",    "target_node_id": "end",       "branch": "no" },
                    { "id": "e3", "source_node_id": "n_paywall", "target_node_id": "end",       "branch": "yes" }
                ]
            },
            "registered": { "root_node_id": null, "nodes": [], "edges": [] },
            "customer":   { "root_node_id": null, "nodes": [], "edges": [] }
        },
        "outcomes": [
            {
                "id": PAYWALL_OUTCOME, "title": "Paywall", "is_builtin": false, "order_index": 0,
                "components": [
                    { "id": "44444444-4444-4444-4444-444444444444", "slug": "wall",
                      "type": "html_injection",
                      "config": { "type": "html_injection", "target_selector": "#article-body",
                                  "placement_mode": "append", "html_body": "<div class=\"rre-wall\">Subscribe to continue</div>" },
                      "placement": "inline", "order_index": 0 }
                ]
            },
            { "id": "33333333-3333-3333-3333-333333333333", "title": "ShowContent",
              "is_builtin": true, "order_index": 1, "components": [] }
        ]
    })
}

async fn spawn(upstream: &str, backend: &str) -> String {
    let settings = Settings {
        proxy_bind_addr: "127.0.0.1:0".to_string(),
        upstream_base_url: upstream.to_string(),
        backend_base_url: backend.to_string(),
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
        host: HOST.to_string(),
        path_glob: "/article*".to_string(),
        feature_id: FEATURE.to_string(),
    }])
    .unwrap();
    let backend_client = BackendClient::new(http.clone(), backend.to_string(), 30);
    let sanitizer =
        rre_proxy::domain::applier::html_sanitizer::load_sanitizer("config/sanitizer.yaml")
            .unwrap();

    let state = AppState {
        settings: Arc::new(settings),
        http,
        feature_map: Arc::new(feature_map),
        backend: Arc::new(backend_client),
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

#[tokio::test]
async fn injects_paywall_for_paywalled_article() {
    let upstream = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/article/1"))
        .respond_with(
            // Use `set_body_raw` so body + a SINGLE `text/html` content-type are
            // set atomically. `set_body_string` would force `text/plain` (and a
            // later `insert_header` can leave a duplicate/ordered content-type,
            // making the proxy's HTML gate flaky).
            ResponseTemplate::new(200).set_body_raw(
                r#"<html><head><meta name="paywall" content="true"></head><body><div id="article-body"><p>Body</p></div></body></html>"#
                    .as_bytes(),
                "text/html; charset=utf-8",
            ),
        )
        .mount(&upstream)
        .await;

    let backend = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path(format!("/api/v1/features/{FEATURE}/active-version")))
        .respond_with(ResponseTemplate::new(200).set_body_json(active_version_body()))
        .mount(&backend)
        .await;

    let base = spawn(&upstream.uri(), &backend.uri()).await;

    let res = reqwest::Client::new()
        .get(format!("{base}/article/1"))
        .header("host", HOST)
        .send()
        .await
        .unwrap();

    assert_eq!(
        res.headers()
            .get("x-rre-apply-status")
            .and_then(|v| v.to_str().ok()),
        Some("ok")
    );
    let body = res.text().await.unwrap();
    assert!(body.contains("Subscribe to continue"), "body: {body}");
}

#[tokio::test]
async fn passes_through_unmapped_path() {
    let upstream = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/other"))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("content-type", "text/html")
                .set_body_string("<html><body>untouched</body></html>"),
        )
        .mount(&upstream)
        .await;

    let backend = MockServer::start().await;
    let base = spawn(&upstream.uri(), &backend.uri()).await;

    let res = reqwest::Client::new()
        .get(format!("{base}/other"))
        .header("host", HOST)
        .send()
        .await
        .unwrap();

    assert_eq!(
        res.headers()
            .get("x-rre-apply-status")
            .and_then(|v| v.to_str().ok()),
        Some("skipped")
    );
    let body = res.text().await.unwrap();
    assert!(body.contains("untouched"));
}

#[tokio::test]
async fn fails_open_when_backend_unavailable() {
    let upstream = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/article/2"))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("content-type", "text/html")
                .set_body_string(
                    r#"<html><head><meta name="paywall" content="true"></head><body>original</body></html>"#,
                ),
        )
        .mount(&upstream)
        .await;

    // Backend returns 404 for active-version -> proxy fails open (pass-through).
    let backend = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path(format!("/api/v1/features/{FEATURE}/active-version")))
        .respond_with(ResponseTemplate::new(404))
        .mount(&backend)
        .await;

    let base = spawn(&upstream.uri(), &backend.uri()).await;

    let res = reqwest::Client::new()
        .get(format!("{base}/article/2"))
        .header("host", HOST)
        .send()
        .await
        .unwrap();

    assert_eq!(
        res.headers()
            .get("x-rre-apply-status")
            .and_then(|v| v.to_str().ok()),
        Some("skipped")
    );
    let body = res.text().await.unwrap();
    assert!(body.contains("original"));
}
