//! Full-pipeline e2e for the Component Editor `apply_component` action (design
//! §4.3): a fake upstream serves a paywalled article, a fake backend serves an
//! active-version whose anonymous canvas has an `apply_component` expression node,
//! and a fake `component-templates/{cid}/resolve` endpoint returns the mustache
//! template. The proxy must PRE-RESOLVE the component on the async side, render it
//! with the action's `variables`, ammonia-sanitize, and inject it — so the served
//! body reflects the rendered component.

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

const FEATURE: &str = "demo-article";
const HOST: &str = "rre.test";
const CID: &str = "55555555-5555-5555-5555-555555555555";

async fn mount_feature_list(backend: &MockServer) {
    Mock::given(method("GET"))
        .and(path("/api/v1/features"))
        .and(query_param("page_size", "100"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "items": [{ "id": FEATURE, "name": FEATURE, "type": "html", "execution_order": 1 }]
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

/// Anonymous canvas: start -> meta(paywall?) ; yes -> apply_component -> end.
fn active_version_body() -> serde_json::Value {
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
                    { "kind": "expression", "id": "n_comp",
                      "action": {
                          "type": "apply_component",
                          "component_id": CID,
                          "version": "default",
                          "variables": { "headline": "Subscribe now", "cta": "Join" },
                          "target_selector": "#article-body",
                          "placement_mode": "append"
                      },
                      "position": { "x": 200.0, "y": 0.0 } },
                    { "kind": "end", "id": "end", "position": { "x": 400.0, "y": 100.0 } }
                ],
                "edges": [
                    { "id": "e0", "source_node_id": "start",  "target_node_id": "n_meta", "branch": "yes" },
                    { "id": "e1", "source_node_id": "n_meta", "target_node_id": "n_comp", "branch": "yes" },
                    { "id": "e2", "source_node_id": "n_meta", "target_node_id": "end",    "branch": "no" },
                    { "id": "e3", "source_node_id": "n_comp", "target_node_id": "end",    "branch": "yes" }
                ]
            },
        },
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

#[tokio::test]
async fn apply_component_renders_resolved_template_into_html() {
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
    mount_feature_list(&backend).await;
    mount_site_list(&backend, &upstream.uri()).await;
    Mock::given(method("GET"))
        .and(path(format!("/api/v1/features/{FEATURE}/active-version")))
        .respond_with(ResponseTemplate::new(200).set_body_json(active_version_body()))
        .mount(&backend)
        .await;
    // The proxy-facing resolve endpoint returns the mustache template. The
    // forwarder pre-resolves this BEFORE the sync apply (design §4.3).
    Mock::given(method("GET"))
        .and(path(format!("/api/v1/component-templates/{CID}/resolve")))
        .and(query_param("version", "default"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "version_number": 7,
            "html_body": "<div class=\"rre-cta\"><h2>{{headline}}</h2><a href=\"#\">{{cta}}</a></div>",
            "variables": [
                { "name": "headline", "title": "Headline" },
                { "name": "cta", "title": "CTA label" }
            ]
        })))
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
    // The rendered (variable-substituted) component is present.
    assert!(body.contains("Subscribe now"), "headline rendered: {body}");
    assert!(body.contains("Join"), "cta rendered: {body}");
    assert!(body.contains("class=\"rre-cta\""), "wrapper kept: {body}");
    // Injected inside the target, original article body retained.
    assert!(body.contains("<p>Body</p>"), "original retained: {body}");
}

/// When the resolve endpoint 404s (component deleted / unresolvable), the proxy
/// fails open: the body is served untouched, with `x-rre-apply-status: skipped`
/// (no expression node changed the body).
#[tokio::test]
async fn apply_component_resolve_404_fails_open() {
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
    mount_feature_list(&backend).await;
    mount_site_list(&backend, &upstream.uri()).await;
    Mock::given(method("GET"))
        .and(path(format!("/api/v1/features/{FEATURE}/active-version")))
        .respond_with(ResponseTemplate::new(200).set_body_json(active_version_body()))
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
