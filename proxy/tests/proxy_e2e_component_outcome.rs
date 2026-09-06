//! Full-pipeline e2e for the `component_ref` / `component_ref_json` COMPONENTS
//! inside an outcome (Component Editor, design §4): the active version's canvas has
//! an `apply_outcome` expression node, and the referenced outcome carries a
//! `component_ref` (HTML) / `component_ref_json` (JSON) component. The proxy must
//! collect the component's `(component_id, version)` ref from INSIDE the applied
//! outcome (not just from a direct `apply_component` action), PRE-RESOLVE it on the
//! async side, render with the component's `variables`, ammonia-sanitize, and
//! inject / set-at-path during outcome application.

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
use serde_json::json;
use wiremock::matchers::{method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

const HTML_FEATURE: &str = "demo-article";
const JSON_FEATURE: &str = "demo-json";
const HOST: &str = "rre.test";
const CID: &str = "55555555-5555-5555-5555-555555555555";
const OUTCOME_ID: &str = "99999999-9999-9999-9999-999999999999";

async fn mount_feature_list(backend: &MockServer, id: &str, kind: &str) {
    Mock::given(method("GET"))
        .and(path("/api/v1/features"))
        .and(query_param("page_size", "100"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "items": [{ "id": id, "name": id, "type": kind, "execution_order": 1 }]
        })))
        .mount(backend)
        .await;
}

async fn mount_site_list(backend: &MockServer, upstream_uri: &str) {
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

/// HTML active-version: start -> meta(paywall?) ; yes -> apply_outcome -> end, with
/// the outcome carrying a single `component_ref` inline component.
fn html_active_version() -> serde_json::Value {
    json!({
        "version_number": 1,
        "rule_graph": {
            "canvas": {
                "root_node_id": "start",
                "nodes": [
                    { "kind": "start", "id": "start", "position": { "x": -200.0, "y": 0.0 } },
                    { "kind": "decision", "id": "n_meta",
                      "processor": { "type": "meta_tags", "tag_name": "paywall", "operator": "contains", "value": "true" },
                      "position": { "x": 0.0, "y": 0.0 } },
                    { "kind": "expression", "id": "n_out",
                      "action": { "type": "apply_outcome", "outcome_id": OUTCOME_ID },
                      "position": { "x": 200.0, "y": 0.0 } },
                    { "kind": "end", "id": "end", "position": { "x": 400.0, "y": 100.0 } }
                ],
                "edges": [
                    { "id": "e0", "source_node_id": "start",  "target_node_id": "n_meta", "branch": "yes" },
                    { "id": "e1", "source_node_id": "n_meta", "target_node_id": "n_out",  "branch": "yes" },
                    { "id": "e2", "source_node_id": "n_meta", "target_node_id": "end",    "branch": "no" },
                    { "id": "e3", "source_node_id": "n_out",  "target_node_id": "end",    "branch": "yes" }
                ]
            },
        },
        "outcomes": [{
            "id": OUTCOME_ID,
            "title": "Paywall",
            "is_builtin": false,
            "order_index": 0,
            "components": [{
                "id": "66666666-6666-6666-6666-666666666666",
                "slug": "cta",
                "type": "component_ref",
                "config": {
                    "type": "component_ref",
                    "component_id": CID,
                    "version": "default",
                    "variables": { "headline": "Subscribe now", "cta": "Join" },
                    "target_selector": "#article-body",
                    "placement_mode": "append"
                },
                "placement": "inline",
                "order_index": 0
            }]
        }]
    })
}

/// JSON active-version: start -> apply_outcome -> end, with the outcome carrying a
/// `component_ref_json` component setting the rendered HTML string at `$.content.html`.
fn json_active_version() -> serde_json::Value {
    json!({
        "version_number": 1,
        "rule_graph": {
            "canvas": {
                "root_node_id": "start",
                "nodes": [
                    { "kind": "start", "id": "start", "position": { "x": -200.0, "y": 0.0 } },
                    { "kind": "expression", "id": "n_out",
                      "action": { "type": "apply_outcome", "outcome_id": OUTCOME_ID },
                      "position": { "x": 0.0, "y": 0.0 } },
                    { "kind": "end", "id": "end", "position": { "x": 200.0, "y": 100.0 } }
                ],
                "edges": [
                    { "id": "e0", "source_node_id": "start", "target_node_id": "n_out", "branch": "yes" },
                    { "id": "e1", "source_node_id": "n_out", "target_node_id": "end",   "branch": "yes" }
                ]
            },
        },
        "outcomes": [{
            "id": OUTCOME_ID,
            "title": "Embed",
            "is_builtin": false,
            "order_index": 0,
            "components": [{
                "id": "77777777-7777-7777-7777-777777777777",
                "slug": "embed",
                "type": "component_ref_json",
                "config": {
                    "type": "component_ref_json",
                    "component_id": CID,
                    "version": "default",
                    "variables": { "headline": "Subscribe now" },
                    "target_path": "$.content.html"
                },
                "placement": "inline",
                "order_index": 0
            }]
        }]
    })
}

async fn mount_resolve(backend: &MockServer, html_body: &str) {
    Mock::given(method("GET"))
        .and(path(format!("/api/v1/component-templates/{CID}/resolve")))
        .and(query_param("version", "default"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "version_number": 7,
            "html_body": html_body,
            "variables": []
        })))
        .mount(backend)
        .await;
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

#[tokio::test]
async fn component_ref_inside_outcome_renders_into_html() {
    let upstream = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/article/1"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(
            r#"<html><head><meta name="paywall" content="true"></head><body><div id="article-body"><p>Body</p></div></body></html>"#
                .as_bytes(),
            "text/html; charset=utf-8",
        ))
        .mount(&upstream)
        .await;

    let backend = MockServer::start().await;
    mount_feature_list(&backend, HTML_FEATURE, "html").await;
    mount_site_list(&backend, &upstream.uri()).await;
    Mock::given(method("GET"))
        .and(path(format!(
            "/api/v1/features/{HTML_FEATURE}/active-version"
        )))
        .respond_with(ResponseTemplate::new(200).set_body_json(html_active_version()))
        .mount(&backend)
        .await;
    mount_resolve(
        &backend,
        "<div class=\"rre-cta\"><h2>{{headline}}</h2><a href=\"#\">{{cta}}</a></div>",
    )
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
    assert!(body.contains("Subscribe now"), "headline rendered: {body}");
    assert!(body.contains("Join"), "cta rendered: {body}");
    assert!(body.contains("class=\"rre-cta\""), "wrapper kept: {body}");
    assert!(body.contains("<p>Body</p>"), "original retained: {body}");
}

#[tokio::test]
async fn component_ref_json_inside_outcome_sets_rendered_string() {
    let upstream = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/data"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(
            r#"{"content":{"title":"t"}}"#.as_bytes(),
            "application/json; charset=utf-8",
        ))
        .mount(&upstream)
        .await;

    let backend = MockServer::start().await;
    mount_feature_list(&backend, JSON_FEATURE, "json").await;
    mount_site_list(&backend, &upstream.uri()).await;
    Mock::given(method("GET"))
        .and(path(format!(
            "/api/v1/features/{JSON_FEATURE}/active-version"
        )))
        .respond_with(ResponseTemplate::new(200).set_body_json(json_active_version()))
        .mount(&backend)
        .await;
    mount_resolve(&backend, "<p>{{headline}}</p>").await;

    let base = spawn(&upstream.uri(), &backend.uri()).await;

    let res = reqwest::Client::new()
        .get(format!("{base}/api/data"))
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
    let body: serde_json::Value = res.json().await.unwrap();
    assert_eq!(
        body["content"]["html"],
        json!("<p>Subscribe now</p>"),
        "rendered string set at path: {body}"
    );
    assert_eq!(body["content"]["title"], json!("t"), "sibling untouched");
}

/// The resolve endpoint 404s for the outcome's `component_ref`: the proxy fails open
/// (body served untouched, no component injected).
#[tokio::test]
async fn component_ref_inside_outcome_resolve_404_fails_open() {
    let upstream = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/article/2"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(
            r#"<html><head><meta name="paywall" content="true"></head><body><div id="article-body"><p>Body</p></div></body></html>"#
                .as_bytes(),
            "text/html; charset=utf-8",
        ))
        .mount(&upstream)
        .await;

    let backend = MockServer::start().await;
    mount_feature_list(&backend, HTML_FEATURE, "html").await;
    mount_site_list(&backend, &upstream.uri()).await;
    Mock::given(method("GET"))
        .and(path(format!(
            "/api/v1/features/{HTML_FEATURE}/active-version"
        )))
        .respond_with(ResponseTemplate::new(200).set_body_json(html_active_version()))
        .mount(&backend)
        .await;
    Mock::given(method("GET"))
        .and(path(format!("/api/v1/component-templates/{CID}/resolve")))
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

    let body = res.text().await.unwrap();
    assert!(!body.contains("rre-cta"), "no component injected: {body}");
    assert!(body.contains("<p>Body</p>"), "original served: {body}");
}
