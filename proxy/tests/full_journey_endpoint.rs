//! Integration tests for `POST /__rre/eval-full-journey`.
//!
//! Spins up the full `build_app` stack against a wiremock backend (feature list +
//! active versions) and a wiremock upstream (real response body), then asserts the
//! endpoint runs EVERY feature of the response's content type in execution order,
//! CHAINING the body feature→feature exactly like the production forwarder, and
//! returns each feature's per-node Transformation Journey.

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

const HOST: &str = "rre.test";

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

/// Mount a Site routing the inbound `host` (`HOST:80`) to the wiremock upstream.
async fn mount_site(backend: &MockServer, upstream_uri: &str) {
    let authority = upstream_uri.trim_start_matches("http://");
    let (dest_host, dest_port) = authority
        .rsplit_once(':')
        .map(|(h, p)| (h.to_string(), p.parse::<u16>().unwrap()))
        .unwrap();
    Mock::given(method("GET"))
        .and(path("/api/v1/sites"))
        .and(query_param("page_size", "100"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "items": [{
                "slug": "demo-site",
                "source_host": HOST,
                "source_port": 80,
                "dest_protocol": "http",
                "dest_host": dest_host,
                "dest_port": dest_port,
                "headers": {}
            }]
        })))
        .mount(backend)
        .await;
}

/// A JSON active-version whose anonymous canvas is:
/// start -> json_expression(`json_path` == `value`) ; yes -> add_attribute(`set_path` = `set_value`) -> end.
/// Body-mutating actions only (no outcomes needed), so the journey shows REAL JSON
/// changes and feature→feature chaining.
fn json_active_version(
    json_path: &str,
    value: &str,
    set_path: &str,
    set_value: &str,
    vnum: i32,
) -> Value {
    json!({
        "version_number": vnum,
        "rule_graph": {
            "canvas": {
                "root_node_id": "start",
                "nodes": [
                    { "kind": "start", "id": "start", "position": { "x": -200.0, "y": 0.0 } },
                    { "kind": "decision", "id": "d_api",
                      "processor": { "type": "json_expression", "json_path": json_path, "operator": "equals", "value": value },
                      "position": { "x": 0.0, "y": 0.0 } },
                    { "kind": "expression", "id": "a_set",
                      "action": { "type": "add_attribute", "json_path": set_path, "value": set_value },
                      "position": { "x": 200.0, "y": 0.0 } },
                    { "kind": "end", "id": "end", "position": { "x": 400.0, "y": 100.0 } }
                ],
                "edges": [
                    { "id": "e0", "source_node_id": "start", "target_node_id": "d_api", "branch": "yes" },
                    { "id": "e1", "source_node_id": "d_api", "target_node_id": "a_set", "branch": "yes" },
                    { "id": "e2", "source_node_id": "d_api", "target_node_id": "end",   "branch": "no"  },
                    { "id": "e3", "source_node_id": "a_set", "target_node_id": "end",   "branch": "yes" }
                ]
            },
        },
        "applicability": {},
        "outcomes": [
            { "id": "33333333-3333-3333-3333-333333333333", "title": "ShowContent", "is_builtin": true, "order_index": 0, "components": [] }
        ]
    })
}

/// An HTML active-version: start -> apply_outcome(html_injection appending
/// `<div class="{marker_class}">` to `#article-body`) -> end. Always routes to the
/// injection (no decision); the only gate is the version-level
/// `applicability.html_selector`, so chaining is provable: a later feature can gate
/// on an element an earlier feature injected. Each call MUST pass a distinct
/// `component_id` — the html_injection renderer is idempotent per component id, so a
/// shared id would make the second feature's injection a no-op (its marker already
/// present from the first feature's chained body).
fn html_active_version(
    marker_class: &str,
    html_selector: Value,
    outcome_id: &str,
    component_id: &str,
    vnum: i32,
) -> Value {
    json!({
        "version_number": vnum,
        "rule_graph": {
            "canvas": {
                "root_node_id": "start",
                "nodes": [
                    { "kind": "start", "id": "start", "position": { "x": -200.0, "y": 0.0 } },
                    { "kind": "expression", "id": "n_inject",
                      "action": { "type": "apply_outcome", "outcome_id": outcome_id },
                      "position": { "x": 0.0, "y": 0.0 } },
                    { "kind": "end", "id": "end", "position": { "x": 200.0, "y": 100.0 } }
                ],
                "edges": [
                    { "id": "e0", "source_node_id": "start",    "target_node_id": "n_inject", "branch": "yes" },
                    { "id": "e1", "source_node_id": "n_inject", "target_node_id": "end",      "branch": "yes" }
                ]
            },
        },
        "applicability": { "html_selector": html_selector },
        "outcomes": [
            {
                "id": outcome_id, "title": "Inject", "is_builtin": false, "order_index": 0,
                "components": [
                    { "id": component_id, "slug": "wall",
                      "type": "html_injection",
                      "config": { "type": "html_injection", "target_selector": "#article-body",
                                  "placement_mode": "append",
                                  "html_body": format!("<div class=\"{marker_class}\">x</div>") },
                      "placement": "inline", "order_index": 0 }
                ]
            },
            { "id": "33333333-3333-3333-3333-333333333333", "title": "ShowContent", "is_builtin": true, "order_index": 1, "components": [] }
        ]
    })
}

/// Two JSON features chained: f-one (exec 1) keys off `$.api == "demo"` and sets
/// `$.from_one`; f-two (exec 2) keys off `$.from_one == "1"` — which is ONLY true
/// AFTER f-one ran. So f-two matching proves the body was chained feature→feature.
#[tokio::test]
async fn json_full_journey_chains_two_features_in_execution_order() {
    let upstream = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/article/1"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(
            r#"{"api":"demo"}"#.as_bytes(),
            "application/json; charset=utf-8",
        ))
        .mount(&upstream)
        .await;

    let backend = MockServer::start().await;
    mount_site(&backend, &upstream.uri()).await;
    // Feature list: two JSON features + one HTML feature (filtered out). The list is
    // returned in execution order (type asc, execution_order asc).
    Mock::given(method("GET"))
        .and(path("/api/v1/features"))
        .and(query_param("page_size", "100"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "items": [
                { "id": "f-html", "name": "HTML feature", "type": "html", "execution_order": 1 },
                { "id": "f-one",  "name": "First JSON",    "type": "json", "execution_order": 1 },
                { "id": "f-two",  "name": "Second JSON",   "type": "json", "execution_order": 2 }
            ],
            "page": 1, "page_size": 100, "total": 3
        })))
        .mount(&backend)
        .await;
    // f-one: $.api == "demo" -> set $.from_one = "1".
    Mock::given(method("GET"))
        .and(path("/api/v1/features/f-one/active-version"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(json_active_version(
                "$.api",
                "demo",
                "$.from_one",
                "1",
                7,
            )),
        )
        .mount(&backend)
        .await;
    // f-two: $.from_one == "1" (only true after f-one) -> set $.from_two = "2".
    Mock::given(method("GET"))
        .and(path("/api/v1/features/f-two/active-version"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(json_active_version(
                "$.from_one",
                "1",
                "$.from_two",
                "2",
                4,
            )),
        )
        .mount(&backend)
        .await;
    // The HTML feature's active-version is never fetched (filtered by content type).

    // Bogus upstream_base_url so ONLY the matched Site can route.
    let base = spawn("http://127.0.0.1:1", &backend.uri()).await;

    let res = reqwest::Client::new()
        .post(format!("{base}/__rre/eval-full-journey"))
        .json(&json!({ "url": format!("http://{HOST}/article/1") }))
        .send()
        .await
        .unwrap();

    assert_eq!(res.status().as_u16(), 200, "full-journey succeeds");
    let body: Value = res.json().await.unwrap();

    assert_eq!(body["content_kind"], json!("json"));
    assert_eq!(body["site"], json!("demo-site"));
    assert!(
        is_d_dd(body["total_time_ms"].as_str().unwrap()),
        "total_time_ms d.dd: {:?}",
        body["total_time_ms"]
    );

    let features = body["features"].as_array().unwrap();
    // Only the two JSON features are present (HTML feature filtered out), in order.
    assert_eq!(features.len(), 2, "two JSON features: {features:?}");
    assert_eq!(features[0]["feature_id"], json!("f-one"));
    assert_eq!(features[0]["execution_order"], json!(1));
    assert_eq!(features[0]["version_number"], json!(7));
    assert_eq!(features[1]["feature_id"], json!("f-two"));
    assert_eq!(features[1]["execution_order"], json!(2));
    assert_eq!(features[1]["version_number"], json!(4));

    // BOTH matched — f-two only matched because f-one chained `from_one` into the body.
    assert_eq!(features[0]["matched"], json!(true));
    assert_eq!(features[1]["matched"], json!(true));

    // f-one journey: start body is the raw upstream; its end body has from_one set.
    let j0 = features[0]["journey"].as_array().unwrap();
    assert_eq!(j0.first().unwrap()["kind"], json!("start"));
    assert_eq!(j0.first().unwrap()["body_after"], json!({ "api": "demo" }));
    assert_eq!(j0.last().unwrap()["kind"], json!("end"));
    assert_eq!(j0.last().unwrap()["body_after"]["from_one"], json!("1"));

    // f-two journey START body equals f-one's END body (CHAINED) — proves the
    // first-feature-start↔last-feature-end diff has a real chain between them.
    let j1 = features[1]["journey"].as_array().unwrap();
    assert_eq!(j1.first().unwrap()["body_after"]["from_one"], json!("1"));
    // f-two END body carries BOTH mutations.
    let last = j1.last().unwrap();
    assert_eq!(last["body_after"]["from_one"], json!("1"));
    assert_eq!(last["body_after"]["from_two"], json!("2"));

    // Each matched feature carries a summary + d.dd timings.
    for f in features {
        assert!(is_d_dd(f["time_took_ms"].as_str().unwrap()));
        let summary = &f["summary"];
        let exprs = summary["expressions"].as_array().unwrap();
        assert_eq!(exprs.len(), 1, "one add_attribute expression each");
        assert_eq!(exprs[0]["expression_label"], json!("add_attribute"));
    }
}

/// A feature whose selector gate fails is skipped: `matched:false`, empty journey,
/// no summary, and the running body is NOT mutated by it.
#[tokio::test]
async fn json_full_journey_selector_miss_skips_without_mutating() {
    let upstream = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/article/2"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(
            r#"{"api":"demo"}"#.as_bytes(),
            "application/json; charset=utf-8",
        ))
        .mount(&upstream)
        .await;

    let backend = MockServer::start().await;
    mount_site(&backend, &upstream.uri()).await;
    Mock::given(method("GET"))
        .and(path("/api/v1/features"))
        .and(query_param("page_size", "100"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "items": [{ "id": "f-gate", "name": "Gated", "type": "json", "execution_order": 1 }],
            "page": 1, "page_size": 100, "total": 1
        })))
        .mount(&backend)
        .await;
    // Active version with a json_selector that does NOT match -> selector gate fails.
    let mut av = json_active_version("$.api", "demo", "$.set", "x", 1);
    av["applicability"] = json!({ "json_selector": "$.nonexistent" });
    Mock::given(method("GET"))
        .and(path("/api/v1/features/f-gate/active-version"))
        .respond_with(ResponseTemplate::new(200).set_body_json(av))
        .mount(&backend)
        .await;

    let base = spawn("http://127.0.0.1:1", &backend.uri()).await;
    let res = reqwest::Client::new()
        .post(format!("{base}/__rre/eval-full-journey"))
        .json(&json!({ "url": format!("http://{HOST}/article/2") }))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status().as_u16(), 200);
    let body: Value = res.json().await.unwrap();

    let f = &body["features"].as_array().unwrap()[0];
    assert_eq!(f["matched"], json!(false), "selector miss -> not matched");
    assert_eq!(f["version_number"], json!(1), "version still resolved");
    assert!(f["journey"].as_array().unwrap().is_empty(), "empty journey");
    assert!(f.get("summary").is_none(), "no summary when skipped");
}

/// `version_overrides` pins a feature to a specific saved version, fetched via
/// `GET /features/{fid}/versions/{vnum}` instead of its active version.
#[tokio::test]
async fn json_full_journey_version_override_uses_pinned_version() {
    let upstream = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/article/3"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(
            r#"{"api":"demo"}"#.as_bytes(),
            "application/json; charset=utf-8",
        ))
        .mount(&upstream)
        .await;

    let backend = MockServer::start().await;
    mount_site(&backend, &upstream.uri()).await;
    Mock::given(method("GET"))
        .and(path("/api/v1/features"))
        .and(query_param("page_size", "100"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "items": [{ "id": "f-pin", "name": "Pinned", "type": "json", "execution_order": 1 }],
            "page": 1, "page_size": 100, "total": 1
        })))
        .mount(&backend)
        .await;
    // The pinned version 9 (a VersionRead shape: version_number + rule_graph +
    // applicability; outcomes NOT needed) sets $.pinned = "yes".
    Mock::given(method("GET"))
        .and(path("/api/v1/features/f-pin/versions/9"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(json_active_version("$.api", "demo", "$.pinned", "yes", 9)),
        )
        .mount(&backend)
        .await;
    // The active version would set $.pinned = "no"; if the override is honored, it
    // is never fetched. `expect(0)` would over-constrain (cache TTL); we assert on
    // the body instead.

    let base = spawn("http://127.0.0.1:1", &backend.uri()).await;
    let res = reqwest::Client::new()
        .post(format!("{base}/__rre/eval-full-journey"))
        .json(&json!({
            "url": format!("http://{HOST}/article/3"),
            "version_overrides": { "f-pin": 9 }
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status().as_u16(), 200);
    let body: Value = res.json().await.unwrap();

    let f = &body["features"].as_array().unwrap()[0];
    assert_eq!(f["version_number"], json!(9), "pinned version resolved");
    assert_eq!(f["matched"], json!(true));
    let last = f["journey"].as_array().unwrap().last().unwrap();
    assert_eq!(last["body_after"]["pinned"], json!("yes"));
}

/// SSRF guard: a URL whose host matches no configured Site -> 400 NO_SITE
/// (mirrors `/__rre/eval-url`).
#[tokio::test]
async fn full_journey_rejects_unconfigured_host() {
    let backend = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/v1/sites"))
        .and(query_param("page_size", "100"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "items": [] })))
        .mount(&backend)
        .await;
    let base = spawn("http://127.0.0.1:1", &backend.uri()).await;

    let res = reqwest::Client::new()
        .post(format!("{base}/__rre/eval-full-journey"))
        .json(&json!({ "url": "http://unknown.test/article/1" }))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status().as_u16(), 400);
    let body: Value = res.json().await.unwrap();
    assert_eq!(body["error"]["code"], json!("NO_SITE"));
}

/// A malformed / non-http(s) URL is rejected with 400 BAD_URL before any fetch.
#[tokio::test]
async fn full_journey_rejects_bad_url() {
    let backend = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/v1/sites"))
        .and(query_param("page_size", "100"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "items": [] })))
        .mount(&backend)
        .await;
    let base = spawn("http://127.0.0.1:1", &backend.uri()).await;

    let res = reqwest::Client::new()
        .post(format!("{base}/__rre/eval-full-journey"))
        .json(&json!({ "url": "ftp://nope" }))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status().as_u16(), 400);
    let body: Value = res.json().await.unwrap();
    assert_eq!(body["error"]["code"], json!("BAD_URL"));
}

/// HTML branch coverage: two HTML features fold transforms over a running HTML body.
/// h-one (exec 1) always applies, appending `<div class="from-one">`. h-two (exec 2)
/// gates on `html_selector: ".from-one"` — present ONLY after h-one injected it — so
/// h-two matching proves `current_html` chaining + `final_body.as_str()` extraction.
/// Asserts ordering and that h-two's start body == h-one's end body.
#[tokio::test]
async fn html_full_journey_chains_two_features_in_execution_order() {
    let upstream = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/article/h"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(
            r#"<html><head></head><body><div id="article-body"><p>Body</p></div></body></html>"#
                .as_bytes(),
            "text/html; charset=utf-8",
        ))
        .mount(&upstream)
        .await;

    let backend = MockServer::start().await;
    mount_site(&backend, &upstream.uri()).await;
    // Feature list: two HTML features + one JSON feature (filtered out), in order.
    Mock::given(method("GET"))
        .and(path("/api/v1/features"))
        .and(query_param("page_size", "100"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "items": [
                { "id": "h-one",  "name": "First HTML",  "type": "html", "execution_order": 1 },
                { "id": "h-two",  "name": "Second HTML", "type": "html", "execution_order": 2 },
                { "id": "j-skip", "name": "JSON feat",   "type": "json", "execution_order": 1 }
            ],
            "page": 1, "page_size": 100, "total": 3
        })))
        .mount(&backend)
        .await;
    // h-one: no selector gate (apply always) -> inject .from-one.
    Mock::given(method("GET"))
        .and(path("/api/v1/features/h-one/active-version"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(html_active_version(
                "from-one",
                Value::Null,
                "11111111-1111-1111-1111-111111111111",
                "aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa",
                5,
            )),
        )
        .mount(&backend)
        .await;
    // h-two: html_selector ".from-one" (only true after h-one) -> inject .from-two.
    Mock::given(method("GET"))
        .and(path("/api/v1/features/h-two/active-version"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(html_active_version(
                "from-two",
                json!(".from-one"),
                "22222222-2222-2222-2222-222222222222",
                "bbbbbbbb-bbbb-bbbb-bbbb-bbbbbbbbbbbb",
                8,
            )),
        )
        .mount(&backend)
        .await;

    let base = spawn("http://127.0.0.1:1", &backend.uri()).await;
    let res = reqwest::Client::new()
        .post(format!("{base}/__rre/eval-full-journey"))
        .json(&json!({ "url": format!("http://{HOST}/article/h") }))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status().as_u16(), 200);
    let body: Value = res.json().await.unwrap();

    assert_eq!(body["content_kind"], json!("html"));
    let features = body["features"].as_array().unwrap();
    // Only the two HTML features (JSON feature filtered out), in execution order.
    assert_eq!(features.len(), 2, "two HTML features: {features:?}");
    assert_eq!(features[0]["feature_id"], json!("h-one"));
    assert_eq!(features[0]["execution_order"], json!(1));
    assert_eq!(features[1]["feature_id"], json!("h-two"));
    assert_eq!(features[1]["execution_order"], json!(2));
    assert_eq!(features[0]["matched"], json!(true));
    // h-two matched ONLY because h-one chained `.from-one` into the running HTML.
    assert_eq!(features[1]["matched"], json!(true));

    // h-one journey: start body is the raw upstream (no markers); end body has from-one.
    let j0 = features[0]["journey"].as_array().unwrap();
    let h_one_start = j0.first().unwrap()["body_after"].as_str().unwrap();
    let h_one_end = j0.last().unwrap()["body_after"].as_str().unwrap();
    assert!(
        !h_one_start.contains("from-one"),
        "h-one start is the raw body"
    );
    assert!(h_one_end.contains("from-one"), "h-one injected from-one");

    // h-two journey START body == h-one END body (CHAINED HTML string).
    let j1 = features[1]["journey"].as_array().unwrap();
    let h_two_start = j1.first().unwrap()["body_after"].as_str().unwrap();
    assert_eq!(
        h_two_start, h_one_end,
        "h-two start body == h-one end body (chained)"
    );
    // h-two END body carries BOTH injected markers.
    let h_two_end = j1.last().unwrap()["body_after"].as_str().unwrap();
    assert!(
        h_two_end.contains("from-one"),
        "from-one survives the chain"
    );
    assert!(h_two_end.contains("from-two"), "h-two injected from-two");
}

/// Fail-open: an unresolved active version (404) -> feature entry `matched:false`,
/// empty journey, `version_number:null`, the running body untouched, and a LATER
/// feature still runs against the unchanged body.
#[tokio::test]
async fn json_full_journey_unresolved_version_skips_and_continues() {
    let upstream = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/article/u"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(
            r#"{"api":"demo"}"#.as_bytes(),
            "application/json; charset=utf-8",
        ))
        .mount(&upstream)
        .await;

    let backend = MockServer::start().await;
    mount_site(&backend, &upstream.uri()).await;
    Mock::given(method("GET"))
        .and(path("/api/v1/features"))
        .and(query_param("page_size", "100"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "items": [
                { "id": "f-404",  "name": "Unresolved", "type": "json", "execution_order": 1 },
                { "id": "f-live", "name": "Live",        "type": "json", "execution_order": 2 }
            ],
            "page": 1, "page_size": 100, "total": 2
        })))
        .mount(&backend)
        .await;
    // f-404: active-version 404 (NO_LIVE_VERSION) -> unresolved.
    Mock::given(method("GET"))
        .and(path("/api/v1/features/f-404/active-version"))
        .respond_with(
            ResponseTemplate::new(404).set_body_json(
                json!({ "error": { "code": "NO_LIVE_VERSION", "message": "none" } }),
            ),
        )
        .mount(&backend)
        .await;
    // f-live: resolves -> sets $.from_live keyed off the ORIGINAL (unchanged) body.
    Mock::given(method("GET"))
        .and(path("/api/v1/features/f-live/active-version"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(json_active_version(
                "$.api",
                "demo",
                "$.from_live",
                "yes",
                2,
            )),
        )
        .mount(&backend)
        .await;

    let base = spawn("http://127.0.0.1:1", &backend.uri()).await;
    let res = reqwest::Client::new()
        .post(format!("{base}/__rre/eval-full-journey"))
        .json(&json!({ "url": format!("http://{HOST}/article/u") }))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status().as_u16(), 200);
    let body: Value = res.json().await.unwrap();

    let features = body["features"].as_array().unwrap();
    assert_eq!(features.len(), 2);
    // f-404 skipped: not matched, null version, empty journey, no summary.
    assert_eq!(features[0]["feature_id"], json!("f-404"));
    assert_eq!(features[0]["matched"], json!(false));
    assert_eq!(features[0]["version_number"], Value::Null);
    assert!(features[0]["journey"].as_array().unwrap().is_empty());
    assert!(features[0].get("summary").is_none());
    // f-live still ran against the UNCHANGED original body and matched.
    assert_eq!(features[1]["feature_id"], json!("f-live"));
    assert_eq!(features[1]["matched"], json!(true));
    let f_live_start = features[1]["journey"].as_array().unwrap().first().unwrap();
    assert_eq!(
        f_live_start["body_after"],
        json!({ "api": "demo" }),
        "body untouched by the skipped feature"
    );
    let f_live_end = features[1]["journey"].as_array().unwrap().last().unwrap();
    assert_eq!(f_live_end["body_after"]["from_live"], json!("yes"));
}

/// Fail-open: a feature whose canvas fails to evaluate (an unknown processor kind
/// -> zen eval error) is skipped (`matched:false`, empty journey, body untouched,
/// no panic), and a LATER valid feature still runs against the unchanged body.
#[tokio::test]
async fn json_full_journey_eval_failure_skips_and_continues() {
    let upstream = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/article/e"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(
            r#"{"api":"demo"}"#.as_bytes(),
            "application/json; charset=utf-8",
        ))
        .mount(&upstream)
        .await;

    let backend = MockServer::start().await;
    mount_site(&backend, &upstream.uri()).await;
    Mock::given(method("GET"))
        .and(path("/api/v1/features"))
        .and(query_param("page_size", "100"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "items": [
                { "id": "f-bad",  "name": "Broken", "type": "json", "execution_order": 1 },
                { "id": "f-good", "name": "Good",   "type": "json", "execution_order": 2 }
            ],
            "page": 1, "page_size": 100, "total": 2
        })))
        .mount(&backend)
        .await;
    // f-bad: a decision node with an UNKNOWN processor kind -> the adapter returns a
    // NodeError -> zen eval errors -> evaluate_with_trace Err -> the feature skips.
    let bad_av = json!({
        "version_number": 1,
        "rule_graph": {
            "canvas": {
                "root_node_id": "start",
                "nodes": [
                    { "kind": "start", "id": "start", "position": { "x": 0.0, "y": 0.0 } },
                    { "kind": "decision", "id": "d_bad",
                      "processor": { "type": "totally_unknown_processor", "value": "x" },
                      "position": { "x": 200.0, "y": 0.0 } },
                    { "kind": "expression", "id": "a_set",
                      "action": { "type": "add_attribute", "json_path": "$.from_bad", "value": "1" },
                      "position": { "x": 400.0, "y": 0.0 } },
                    { "kind": "end", "id": "end", "position": { "x": 600.0, "y": 0.0 } }
                ],
                "edges": [
                    { "id": "e0", "source_node_id": "start", "target_node_id": "d_bad", "branch": "yes" },
                    { "id": "e1", "source_node_id": "d_bad", "target_node_id": "a_set", "branch": "yes" },
                    { "id": "e2", "source_node_id": "d_bad", "target_node_id": "end",   "branch": "no"  },
                    { "id": "e3", "source_node_id": "a_set", "target_node_id": "end",   "branch": "yes" }
                ]
            },
        },
        "applicability": {},
        "outcomes": [
            { "id": "33333333-3333-3333-3333-333333333333", "title": "ShowContent", "is_builtin": true, "order_index": 0, "components": [] }
        ]
    });
    Mock::given(method("GET"))
        .and(path("/api/v1/features/f-bad/active-version"))
        .respond_with(ResponseTemplate::new(200).set_body_json(bad_av))
        .mount(&backend)
        .await;
    // f-good: a valid canvas that sets $.from_good.
    Mock::given(method("GET"))
        .and(path("/api/v1/features/f-good/active-version"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(json_active_version(
                "$.api",
                "demo",
                "$.from_good",
                "yes",
                3,
            )),
        )
        .mount(&backend)
        .await;

    let base = spawn("http://127.0.0.1:1", &backend.uri()).await;
    let res = reqwest::Client::new()
        .post(format!("{base}/__rre/eval-full-journey"))
        .json(&json!({ "url": format!("http://{HOST}/article/e") }))
        .send()
        .await
        .unwrap();
    // No panic; a successful 200 with the broken feature skipped.
    assert_eq!(res.status().as_u16(), 200);
    let body: Value = res.json().await.unwrap();

    let features = body["features"].as_array().unwrap();
    assert_eq!(features.len(), 2);
    // f-bad skipped on eval failure: not matched, empty journey, version resolved.
    assert_eq!(features[0]["feature_id"], json!("f-bad"));
    assert_eq!(features[0]["matched"], json!(false));
    assert_eq!(
        features[0]["version_number"],
        json!(1),
        "version still resolved"
    );
    assert!(features[0]["journey"].as_array().unwrap().is_empty());
    assert!(features[0].get("summary").is_none());
    // f-good still ran against the UNCHANGED original body.
    assert_eq!(features[1]["feature_id"], json!("f-good"));
    assert_eq!(features[1]["matched"], json!(true));
    let f_good_end = features[1]["journey"].as_array().unwrap().last().unwrap();
    assert_eq!(f_good_end["body_after"]["from_good"], json!("yes"));
    // The broken feature left no trace in the chained body.
    assert!(f_good_end["body_after"].get("from_bad").is_none());
}

/// A malformed-but-`application/json` upstream body runs ZERO features (mirrors
/// production "nothing runs"): the response is a successful 200 with `features: []`,
/// `content_kind: json`, the site, and `total_time_ms: "0.00"`.
#[tokio::test]
async fn json_full_journey_parse_failure_runs_no_features() {
    let upstream = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/article/bad"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(
            // Invalid JSON but advertised as application/json.
            r#"{"api": not valid json"#.as_bytes(),
            "application/json; charset=utf-8",
        ))
        .mount(&upstream)
        .await;

    let backend = MockServer::start().await;
    mount_site(&backend, &upstream.uri()).await;
    // A JSON feature is listed; it must NOT run (the body never parsed). Its
    // active-version is not mounted — a fetch would 404, but it must never happen.
    Mock::given(method("GET"))
        .and(path("/api/v1/features"))
        .and(query_param("page_size", "100"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "items": [{ "id": "f-json", "name": "JSON", "type": "json", "execution_order": 1 }],
            "page": 1, "page_size": 100, "total": 1
        })))
        .mount(&backend)
        .await;

    let base = spawn("http://127.0.0.1:1", &backend.uri()).await;
    let res = reqwest::Client::new()
        .post(format!("{base}/__rre/eval-full-journey"))
        .json(&json!({ "url": format!("http://{HOST}/article/bad") }))
        .send()
        .await
        .unwrap();
    assert_eq!(
        res.status().as_u16(),
        200,
        "parse failure is a successful no-op"
    );
    let body: Value = res.json().await.unwrap();

    assert_eq!(body["content_kind"], json!("json"));
    assert_eq!(body["site"], json!("demo-site"));
    assert_eq!(body["total_time_ms"], json!("0.00"), "no loop ran");
    assert!(
        body["features"].as_array().unwrap().is_empty(),
        "ZERO features run on a JSON parse failure: {:?}",
        body["features"]
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
