//! Full pipeline e2e for the JSON branch (§3.6): a fake upstream serves an
//! `application/json` body, a fake backend serves an active-version whose
//! anonymous canvas has a `json_expression` decision routing to a JSON-mutation
//! outcome (`json_set`). Also exercises the version-level `json_selector`
//! applicability gate (hit applies, miss serves the original).

use std::sync::Arc;

use rre_proxy::build_app;
use rre_proxy::config::Settings;
use rre_proxy::domain::processors::default_registry;
use rre_proxy::infra::backend_client::BackendClient;
use rre_proxy::infra::compiled_cache::CompiledCache;
use rre_proxy::infra::component_cache::ComponentCache;
use rre_proxy::infra::site_map::SiteMap;
use rre_proxy::state::AppState;
use serde_json::{json, Value};
use wiremock::matchers::{method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

const FEATURE: &str = "demo-article";
const HOST: &str = "rre.test";
const LOCK_OUTCOME: &str = "22222222-2222-2222-2222-222222222222";
const SHOW_OUTCOME: &str = "33333333-3333-3333-3333-333333333333";

/// Active-version with a json_expression decision: type==premium -> lock outcome
/// (json_set $.locked=true), else builtin ShowContent. `applicability` overrides
/// the default `{}` when provided.
fn active_version_body(applicability: Value) -> Value {
    json!({
        "version_number": 1,
        "rule_graph": {
            "canvas": {
                "root_node_id": "start",
                "nodes": [
                    { "kind": "start", "id": "start", "position": { "x": -200.0, "y": 0.0 } },
                    { "kind": "decision", "id": "n_json",
                      "processor": { "type": "json_expression", "json_path": "$.type", "operator": "equals", "value": "premium" },
                      "position": { "x": 0.0, "y": 0.0 } },
                    { "kind": "expression", "id": "n_lock",
                      "action": { "type": "apply_outcome", "outcome_id": LOCK_OUTCOME },
                      "position": { "x": 200.0, "y": 0.0 } },
                    { "kind": "end", "id": "end", "position": { "x": 400.0, "y": 100.0 } }
                ],
                "edges": [
                    { "id": "e0", "source_node_id": "start",  "target_node_id": "n_json", "branch": "yes" },
                    { "id": "e1", "source_node_id": "n_json", "target_node_id": "n_lock", "branch": "yes" },
                    { "id": "e2", "source_node_id": "n_json", "target_node_id": "end",    "branch": "no" },
                    { "id": "e3", "source_node_id": "n_lock", "target_node_id": "end",    "branch": "yes" }
                ]
            },
        },
        "applicability": applicability,
        "outcomes": [
            {
                "id": LOCK_OUTCOME, "title": "Lock", "is_builtin": false, "order_index": 0,
                "components": [
                    { "id": "44444444-4444-4444-4444-444444444444", "slug": "lock",
                      "type": "json_set",
                      "config": { "type": "json_set", "target_path": "$.locked", "value": true },
                      "placement": "inline", "order_index": 0 }
                ]
            },
            { "id": SHOW_OUTCOME, "title": "ShowContent", "is_builtin": true, "order_index": 1, "components": [] }
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
        max_upstream_body_bytes: 16 * 1024 * 1024,
        max_decompressed_bytes: 16 * 1024 * 1024,
        sanitizer_config_path: "config/sanitizer.yaml".to_string(),
    };
    let http = reqwest::Client::new();
    let site_map = SiteMap::new(http.clone(), backend.to_string(), 30);
    let backend_client = BackendClient::new(http.clone(), backend.to_string(), 30);
    let component_cache = ComponentCache::new(http.clone(), backend.to_string(), 30);
    let sanitizer =
        rre_proxy::domain::applier::html_sanitizer::load_sanitizer("config/sanitizer.yaml")
            .unwrap();

    let state = AppState {
        settings: Arc::new(settings),
        http,
        site_map: Arc::new(site_map),
        backend: Arc::new(backend_client),
        compiled: Arc::new(CompiledCache::new(256)),
        component_cache: Arc::new(component_cache),
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

async fn run(upstream_json: &str, upstream_path: &str, applicability: Value) -> (String, String) {
    let upstream = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path(upstream_path.to_string()))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_raw(upstream_json.as_bytes(), "application/json; charset=utf-8"),
        )
        .mount(&upstream)
        .await;

    let backend = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/v1/features"))
        .and(query_param("page_size", "100"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "items": [{ "id": FEATURE, "name": FEATURE, "type": "json", "execution_order": 1 }]
        })))
        .mount(&backend)
        .await;
    Mock::given(method("GET"))
        .and(path(format!("/api/v1/features/{FEATURE}/active-version")))
        .respond_with(ResponseTemplate::new(200).set_body_json(active_version_body(applicability)))
        .mount(&backend)
        .await;

    let base = spawn(&upstream.uri(), &backend.uri()).await;
    let res = reqwest::Client::new()
        .get(format!("{base}{upstream_path}"))
        .header("host", HOST)
        .send()
        .await
        .unwrap();

    let status = res
        .headers()
        .get("x-rre-apply-status")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_string();
    let body = res.text().await.unwrap();
    (status, body)
}

#[tokio::test]
async fn premium_json_gets_locked() {
    let (status, body) = run(
        r#"{"type":"premium","locked":false}"#,
        "/article/1",
        json!({}),
    )
    .await;
    assert_eq!(status, "ok", "body: {body}");
    let v: Value = serde_json::from_str(&body).unwrap();
    assert_eq!(v["locked"], json!(true));
    assert_eq!(v["type"], json!("premium"));
}

#[tokio::test]
async fn free_json_routes_to_show_content_skipped() {
    let (status, body) = run(r#"{"type":"free"}"#, "/article/2", json!({})).await;
    assert_eq!(status, "skipped", "body: {body}");
    let v: Value = serde_json::from_str(&body).unwrap();
    assert_eq!(v["type"], json!("free"));
    assert!(v.get("locked").is_none());
}

#[tokio::test]
async fn json_selector_hit_applies() {
    // json_selector requires `$.type` to exist; it does -> apply.
    let app = json!({ "json_selector": "$.type" });
    let (status, body) = run(r#"{"type":"premium","locked":false}"#, "/article/3", app).await;
    assert_eq!(status, "ok", "body: {body}");
    let v: Value = serde_json::from_str(&body).unwrap();
    assert_eq!(v["locked"], json!(true));
}

#[tokio::test]
async fn json_selector_miss_serves_original() {
    // json_selector requires `$.nonexistent`; it does not match -> serve original.
    let app = json!({ "json_selector": "$.nonexistent" });
    let (status, body) = run(r#"{"type":"premium","locked":false}"#, "/article/4", app).await;
    assert_eq!(status, "skipped", "body: {body}");
    let v: Value = serde_json::from_str(&body).unwrap();
    // Unmodified: locked stays false.
    assert_eq!(v["locked"], json!(false));
}
