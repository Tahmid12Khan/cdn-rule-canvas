//! Full pipeline e2e for the new expression-node JSON action chain (spec §7
//! worked example): a fake upstream serves an `application/json` body, a fake
//! backend serves an active-version whose anonymous canvas is
//! `start -> json_expression -> trim_json -> add_attribute -> end`. When
//! `$.api == "demo-article"`, the proxy empties `$.body` and sets `$.paywall_show`.

use std::sync::Arc;

use rre_proxy::build_app;
use rre_proxy::config::Settings;
use rre_proxy::domain::processors::default_registry;
use rre_proxy::infra::backend_client::BackendClient;
use rre_proxy::infra::compiled_cache::CompiledCache;
use rre_proxy::infra::component_cache::ComponentCache;
use rre_proxy::infra::saved_outcome_cache::SavedOutcomeCache;
use rre_proxy::infra::site_map::SiteMap;
use rre_proxy::state::AppState;
use serde_json::{json, Value};
use wiremock::matchers::{method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

const FEATURE: &str = "demo-json-article";
const HOST: &str = "rre.test";

/// Mount `GET /api/v1/features?page_size=100` returning the single demo feature so
/// the proxy's per-feature pipeline runs it for every request (no feature_map).
/// `type: "json"` so the content-type filter keeps it on a JSON response.
async fn mount_feature_list(backend: &MockServer) {
    Mock::given(method("GET"))
        .and(path("/api/v1/features"))
        .and(query_param("page_size", "100"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "items": [{ "id": FEATURE, "name": FEATURE, "type": "json", "execution_order": 1 }]
        })))
        .mount(backend)
        .await;
}

/// Active-version: start -> d_api (json_expression $.api == demo-article) ;
/// yes -> trim_json($.body, 0) -> add_attribute($.paywall_show) -> end ;
/// no  -> end (pass-through).
fn active_version_body() -> Value {
    json!({
        "version_number": 1,
        "rule_graph": {
            "canvas": {
                "root_node_id": "start",
                "nodes": [
                    { "kind": "start", "id": "start", "position": { "x": -200.0, "y": 0.0 } },
                    { "kind": "decision", "id": "d_api",
                      "processor": { "type": "json_expression", "json_path": "$.api", "operator": "equals", "value": "demo-article" },
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
        max_upstream_body_bytes: 16 * 1024 * 1024,
        max_decompressed_bytes: 16 * 1024 * 1024,
        sanitizer_config_path: "config/sanitizer.yaml".to_string(),
        identity_user_cookie: "rre_user".to_string(),
        identity_products_cookie: "rre_products".to_string(),
        identity_user_header: "x-rre-user".to_string(),
        identity_products_header: "x-rre-products".to_string(),
    };
    let http = reqwest::Client::new();
    let site_map = SiteMap::new(http.clone(), backend.to_string(), 30);
    let backend_client = BackendClient::new(http.clone(), backend.to_string(), 30);
    let component_cache = ComponentCache::new(http.clone(), backend.to_string(), 30);
    let saved_outcome_cache = SavedOutcomeCache::new(http.clone(), backend.to_string(), 30);
    let sanitizer = rre_core::default_sanitizer();

    let state = AppState {
        settings: Arc::new(settings),
        http,
        site_map: Arc::new(site_map),
        backend: Arc::new(backend_client),
        compiled: Arc::new(CompiledCache::new(256)),
        component_cache: Arc::new(component_cache),
        saved_outcome_cache: Arc::new(saved_outcome_cache),
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

    mount_feature_list(&backend).await;
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
        r#"{"api":"demo-article","body":["p1","p2","p3"]}"#,
        "/article/1",
    )
    .await;
    assert_eq!(status, "ok", "body: {body}");
    let v: Value = serde_json::from_str(&body).unwrap();
    assert_eq!(v["body"], json!([]), "body should be emptied");
    assert_eq!(v["paywall_show"], json!("<html>paywall_showed</html>"));
    assert_eq!(v["api"], json!("demo-article"));
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

    mount_feature_list(&backend).await;
    let base = spawn(&upstream.uri(), &backend.uri()).await;
    reqwest::Client::new()
        .get(format!("{base}{upstream_path}"))
        .header("host", HOST)
        .send()
        .await
        .unwrap()
}

/// Spec §1/§2 + v2.2: a matched feature stamps `x-rre-feature-<id>: true` and
/// injects `body.rre.feature_expressions[<id>]` with the v2.2 shape (an
/// `expressions` array of objects, `time_took_ms` as a `d.dd` string,
/// expensive_nodes top-3 in the same object shape).
#[tokio::test]
async fn matched_feature_injects_header_and_feature_expressions() {
    let res = run_full(
        r#"{"api":"demo-article","body":["p1","p2","p3"]}"#,
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
    let entry = &v["rre"]["feature_expressions"][FEATURE];

    // The entry carries the served active `version` (== 1, the mounted version).
    assert_eq!(
        entry["version"],
        json!(1),
        "entry carries the served active version_number: {entry}"
    );

    // v2.2: the matched path traversed t_body then a_pw, as an `expressions`
    // array of objects (in order).
    let expressions = entry["expressions"].as_array().unwrap();
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
        // custom_expression_label is always present (here "" — none set).
        assert_eq!(e["custom_expression_label"].as_str(), Some(""));
        let t = e["expression_time_ms"].as_str().unwrap();
        assert!(is_d_dd(t), "expression_time_ms should be d.dd, got {t:?}");
    }

    // §3: time_took_ms is a STRING formatted `d.dd` (two decimals).
    let time_took_ms = entry["time_took_ms"]
        .as_str()
        .expect("time_took_ms is a string");
    assert!(
        is_d_dd(time_took_ms),
        "time_took_ms should be d.dd, got {time_took_ms:?}"
    );

    // v2.2: expensive_nodes is the top-3 (here 2) expression nodes, DESC by time,
    // each the SAME Expression object shape, with `d.dd` strings.
    let expensive = entry["expensive_nodes"].as_array().unwrap();
    assert_eq!(expensive.len(), 2, "two expression nodes -> two expensive");
    let mut prev = f64::INFINITY;
    for n in expensive {
        assert!(n["expression_id"].is_string());
        assert!(n["expression_label"].is_string());
        assert!(n["custom_expression_label"].is_string());
        let t = n["expression_time_ms"].as_str().unwrap();
        assert!(is_d_dd(t), "expression_time_ms should be d.dd, got {t:?}");
        let parsed: f64 = t.parse().unwrap();
        assert!(parsed <= prev, "expensive_nodes must be DESC by time");
        prev = parsed;
    }

    // spec item 7: `rre.total_time_ms` is a top-level `d.dd` STRING sibling of
    // feature_expressions (NOT nested per-feature).
    let total_time_ms = v["rre"]["total_time_ms"]
        .as_str()
        .expect("rre.total_time_ms is a string sibling of feature_expressions");
    assert!(
        is_d_dd(total_time_ms),
        "total_time_ms should be d.dd, got {total_time_ms:?}"
    );

    // `rre.compute_time_ms` is a `d.dd` STRING sibling, <= total_time_ms (it
    // excludes the per-feature rule-fetch I/O wait).
    let compute_time_ms = v["rre"]["compute_time_ms"]
        .as_str()
        .expect("rre.compute_time_ms is a string sibling of feature_expressions");
    assert!(
        is_d_dd(compute_time_ms),
        "compute_time_ms should be d.dd, got {compute_time_ms:?}"
    );
    let total_f: f64 = total_time_ms.parse().unwrap();
    let compute_f: f64 = compute_time_ms.parse().unwrap();
    assert!(
        compute_f <= total_f + 1e-9,
        "compute_time_ms ({compute_f}) <= total_time_ms ({total_f})"
    );

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

const HTML_OUTCOME: &str = "22222222-2222-2222-2222-222222222222";

/// Active-version whose only expression is an `apply_outcome` referencing an
/// HTML-only outcome (a single `html_injection` component). On a JSON response
/// `apply_action_json` skips the non-JSON component -> `changed = false`, so the
/// applied-gate (spec v2.1) must NOT mark the feature.
fn html_only_active_version_body() -> Value {
    json!({
        "version_number": 1,
        "rule_graph": {
            "canvas": {
                "root_node_id": "start",
                "nodes": [
                    { "kind": "start", "id": "start", "position": { "x": -200.0, "y": 0.0 } },
                    { "kind": "decision", "id": "d_api",
                      "processor": { "type": "json_expression", "json_path": "$.api", "operator": "equals", "value": "demo-article" },
                      "position": { "x": 0.0, "y": 0.0 } },
                    { "kind": "expression", "id": "n_html",
                      "action": { "type": "apply_outcome", "outcome_id": HTML_OUTCOME },
                      "position": { "x": 200.0, "y": 0.0 } },
                    { "kind": "end", "id": "end", "position": { "x": 400.0, "y": 0.0 } }
                ],
                "edges": [
                    { "id": "e0", "source_node_id": "start",  "target_node_id": "d_api",  "branch": "yes" },
                    { "id": "e1", "source_node_id": "d_api",  "target_node_id": "n_html", "branch": "yes" },
                    { "id": "e2", "source_node_id": "d_api",  "target_node_id": "end",    "branch": "no"  },
                    { "id": "e3", "source_node_id": "n_html", "target_node_id": "end",    "branch": "yes" }
                ]
            },
        },
        "applicability": {},
        "outcomes": [
            {
                "id": HTML_OUTCOME, "title": "HtmlWall", "is_builtin": false, "order_index": 0,
                "components": [
                    { "id": "44444444-4444-4444-4444-444444444444", "slug": "wall",
                      "type": "html_injection",
                      "config": { "type": "html_injection", "target_selector": "#article-body",
                                  "placement_mode": "append", "html_body": "<div>wall</div>" },
                      "placement": "inline", "order_index": 0 }
                ]
            }
        ]
    })
}

/// Spec v2.1 applied-gate: a feature whose only expression is a no-op on the
/// response (an `apply_outcome` of an HTML-only outcome, on a JSON body ->
/// `changed=false`) does NOT get an `x-rre-feature` header and is ABSENT from
/// `feature_expressions` — even though it traversed an expression node and its
/// (empty) json_selector would "always apply".
#[tokio::test]
async fn no_op_expression_is_not_marked_applied_gate() {
    let upstream = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/article/5"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(
            r#"{"api":"demo-article","body":["p1","p2","p3"]}"#.as_bytes(),
            "application/json; charset=utf-8",
        ))
        .mount(&upstream)
        .await;

    let backend = MockServer::start().await;
    mount_feature_list(&backend).await;
    Mock::given(method("GET"))
        .and(path(format!("/api/v1/features/{FEATURE}/active-version")))
        .respond_with(ResponseTemplate::new(200).set_body_json(html_only_active_version_body()))
        .mount(&backend)
        .await;

    let base = spawn(&upstream.uri(), &backend.uri()).await;
    let res = reqwest::Client::new()
        .get(format!("{base}/article/5"))
        .header("host", HOST)
        .send()
        .await
        .unwrap();

    // No-op on JSON -> overall apply_status skipped, no marker header.
    assert_eq!(
        res.headers()
            .get("x-rre-apply-status")
            .and_then(|v| v.to_str().ok()),
        Some("skipped"),
    );
    assert!(
        res.headers()
            .get(format!("x-rre-feature-{FEATURE}"))
            .is_none(),
        "a no-op expression must not stamp a marker header (applied-gate)"
    );

    let body = res.text().await.unwrap();
    let v: Value = serde_json::from_str(&body).unwrap();
    // Absent from feature_expressions: no rre key at all (nothing applied).
    assert!(
        v.get("rre").is_none(),
        "no rre key when the only expression was a no-op"
    );
    // The body itself is unchanged.
    assert_eq!(v["body"], json!(["p1", "p2", "p3"]));
}

/// Content-type filter: on a JSON response the proxy evaluates ONLY json-type
/// features — an html-type feature's active-version endpoint is NEVER fetched
/// (and would not be evaluated). Proves the hot-path filter removes cross-type
/// evals + their cold rule fetches.
#[tokio::test]
async fn json_response_does_not_fetch_or_eval_html_features() {
    const HTML_FEATURE: &str = "demo-html-article";

    let upstream = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/article/ct"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(
            r#"{"api":"demo-article","body":["p1","p2","p3"]}"#.as_bytes(),
            "application/json; charset=utf-8",
        ))
        .mount(&upstream)
        .await;

    let backend = MockServer::start().await;
    // Feature list with BOTH a json and an html feature.
    Mock::given(method("GET"))
        .and(path("/api/v1/features"))
        .and(query_param("page_size", "100"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "items": [
                { "id": FEATURE,      "name": FEATURE,      "type": "json", "execution_order": 1 },
                { "id": HTML_FEATURE, "name": HTML_FEATURE, "type": "html", "execution_order": 1 }
            ]
        })))
        .mount(&backend)
        .await;
    // The json feature IS fetched + applied.
    Mock::given(method("GET"))
        .and(path(format!("/api/v1/features/{FEATURE}/active-version")))
        .respond_with(ResponseTemplate::new(200).set_body_json(active_version_body()))
        .mount(&backend)
        .await;
    // The html feature's active-version MUST NOT be fetched on a JSON response.
    // `expect(0)` fails the test on any hit.
    Mock::given(method("GET"))
        .and(path(format!(
            "/api/v1/features/{HTML_FEATURE}/active-version"
        )))
        .respond_with(ResponseTemplate::new(200).set_body_json(active_version_body()))
        .expect(0)
        .mount(&backend)
        .await;

    let base = spawn(&upstream.uri(), &backend.uri()).await;
    let res = reqwest::Client::new()
        .get(format!("{base}/article/ct"))
        .header("host", HOST)
        .send()
        .await
        .unwrap();

    assert_eq!(res.status().as_u16(), 200);
    let body = res.text().await.unwrap();
    let v: Value = serde_json::from_str(&body).unwrap();
    // The json feature still applied (body emptied) — the filter kept it.
    assert_eq!(v["body"], json!([]), "json feature applied: {body}");
    // Only the json feature is in feature_expressions; the html feature is absent.
    let fe = &v["rre"]["feature_expressions"];
    assert!(fe.get(FEATURE).is_some(), "json feature present: {fe}");
    assert!(
        fe.get(HTML_FEATURE).is_none(),
        "html feature must not appear on a JSON response: {fe}"
    );
    // The `expect(0)` mock is verified on drop (no html active-version fetch).
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
