//! Full pipeline e2e for the new expression-node JSON action chain (spec §7
//! worked example): a fake upstream serves an `application/json` body, a fake
//! backend serves an active-version whose anonymous canvas is
//! `start -> json_expression -> trim_json -> add_attribute -> end`. When
//! `$.api == "dn-article"`, the proxy empties `$.body` and sets `$.paywall_show`.

use std::sync::Arc;

use rre_proxy::build_app;
use rre_proxy::config::Settings;
use rre_proxy::domain::processors::default_registry;
use rre_proxy::infra::backend_client::BackendClient;
use rre_proxy::infra::compiled_cache::CompiledCache;
use rre_proxy::infra::feature_map::{FeatureMap, FeatureMapEntry};
use rre_proxy::state::AppState;
use serde_json::{json, Value};
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

const FEATURE: &str = "dn-json-article";
const HOST: &str = "rre.test";

/// Active-version: start -> d_api (json_expression $.api == dn-article) ;
/// yes -> trim_json($.body, 0) -> add_attribute($.paywall_show) -> end ;
/// no  -> end (pass-through).
fn active_version_body() -> Value {
    json!({
        "version_number": 1,
        "rule_graph": {
            "anonymous": {
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
            },
            "registered": { "root_node_id": null, "nodes": [], "edges": [] },
            "customer":   { "root_node_id": null, "nodes": [], "edges": [] }
        },
        "applicability": {},
        "outcomes": []
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

async fn run(upstream_json: &str, upstream_path: &str) -> (String, String) {
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
        .and(path(format!("/api/v1/features/{FEATURE}/active-version")))
        .respond_with(ResponseTemplate::new(200).set_body_json(active_version_body()))
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
async fn matching_api_trims_body_and_adds_paywall() {
    let (status, body) = run(
        r#"{"api":"dn-article","body":["p1","p2","p3"]}"#,
        "/article/1",
    )
    .await;
    assert_eq!(status, "ok", "body: {body}");
    let v: Value = serde_json::from_str(&body).unwrap();
    assert_eq!(v["body"], json!([]), "body should be emptied");
    assert_eq!(v["paywall_show"], json!("<html>paywall_showed</html>"));
    assert_eq!(v["api"], json!("dn-article"));
}

#[tokio::test]
async fn non_matching_api_passes_through() {
    let (status, body) = run(r#"{"api":"other","body":["p1","p2","p3"]}"#, "/article/2").await;
    assert_eq!(status, "skipped", "body: {body}");
    let v: Value = serde_json::from_str(&body).unwrap();
    // Unchanged: body intact, no paywall_show added.
    assert_eq!(v["body"], json!(["p1", "p2", "p3"]));
    assert!(v.get("paywall_show").is_none());
}

/// Like `run`, but returns the full `reqwest::Response` so headers can be
/// inspected (spec §1 match-marker header) alongside the body.
async fn run_full(upstream_json: &str, upstream_path: &str) -> reqwest::Response {
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
        .and(path(format!("/api/v1/features/{FEATURE}/active-version")))
        .respond_with(ResponseTemplate::new(200).set_body_json(active_version_body()))
        .mount(&backend)
        .await;

    let base = spawn(&upstream.uri(), &backend.uri()).await;
    reqwest::Client::new()
        .get(format!("{base}{upstream_path}"))
        .header("host", HOST)
        .send()
        .await
        .unwrap()
}

/// Spec §1/§2: a matched feature stamps `x-rre-feature-<id>: true` and injects
/// `body.rre.features_matched[<id>]` with the spec §2 shape (parallel
/// outcome_ids/labels, `time_took` as a `d.dd` string, expensive_nodes top-3).
#[tokio::test]
async fn matched_feature_injects_header_and_features_matched() {
    let res = run_full(
        r#"{"api":"dn-article","body":["p1","p2","p3"]}"#,
        "/article/3",
    )
    .await;

    // §1: match-marker header.
    assert_eq!(
        res.headers()
            .get(format!("x-rre-feature-{FEATURE}"))
            .and_then(|v| v.to_str().ok()),
        Some("true"),
    );

    let body = res.text().await.unwrap();
    let v: Value = serde_json::from_str(&body).unwrap();
    let entry = &v["rre"]["features_matched"][FEATURE];

    // The matched path traversed t_body then a_pw (parallel arrays, in order).
    assert_eq!(entry["outcome_ids"], json!(["t_body", "a_pw"]));
    assert_eq!(
        entry["outcome_labels"],
        json!(["trim_json", "add_attribute"])
    );

    // §3: time_took is a STRING formatted `d.dd` (two decimals).
    let time_took = entry["time_took"].as_str().expect("time_took is a string");
    assert!(
        is_d_dd(time_took),
        "time_took should be d.dd, got {time_took:?}"
    );

    // §2: expensive_nodes is the top-3 (here 2) expression nodes, DESC by time,
    // each a `d.dd` string.
    let expensive = entry["expensive_nodes"].as_array().unwrap();
    assert_eq!(expensive.len(), 2, "two expression nodes -> two expensive");
    let mut prev = f64::INFINITY;
    for n in expensive {
        assert!(n["outcome_id"].is_string());
        assert!(n["outcome_label"].is_string());
        let t = n["outcome_time_in_ms"].as_str().unwrap();
        assert!(is_d_dd(t), "outcome_time_in_ms should be d.dd, got {t:?}");
        let parsed: f64 = t.parse().unwrap();
        assert!(parsed <= prev, "expensive_nodes must be DESC by time");
        prev = parsed;
    }

    // The actual body transform is unchanged.
    assert_eq!(v["body"], json!([]));
    assert_eq!(v["paywall_show"], json!("<html>paywall_showed</html>"));
}

/// Spec §1/§2: a non-matching feature gets NO header and NO `rre` key.
#[tokio::test]
async fn non_matching_feature_omits_header_and_rre() {
    let res = run_full(r#"{"api":"other","body":["p1","p2","p3"]}"#, "/article/4").await;
    assert!(
        res.headers()
            .get(format!("x-rre-feature-{FEATURE}"))
            .is_none(),
        "non-matching feature must not stamp a marker header"
    );
    let body = res.text().await.unwrap();
    let v: Value = serde_json::from_str(&body).unwrap();
    assert!(v.get("rre").is_none(), "no rre key when nothing matched");
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
