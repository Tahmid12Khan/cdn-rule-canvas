//! Verifies the trace-id middleware echoes a generated id and respects an
//! inbound one, against the `/health` route (no upstream needed).

use std::sync::Arc;

use rre_proxy::build_app;
use rre_proxy::config::Settings;
use rre_proxy::domain::processors::default_registry;
use rre_proxy::infra::backend_client::BackendClient;
use rre_proxy::infra::compiled_cache::CompiledCache;
use rre_proxy::infra::component_cache::ComponentCache;
use rre_proxy::infra::saved_outcome_cache::SavedOutcomeCache;
use rre_proxy::infra::site_map::SiteMap;
use rre_proxy::middleware::trace::TRACE_HEADER;
use rre_proxy::state::AppState;

async fn spawn() -> String {
    let settings = Settings {
        proxy_bind_addr: "127.0.0.1:0".to_string(),
        upstream_base_url: "http://127.0.0.1:1".to_string(),
        backend_base_url: "http://127.0.0.1:1".to_string(),
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
    let site_map = SiteMap::new(http.clone(), settings.backend_base_url.clone(), 30);
    let backend = BackendClient::new(http.clone(), settings.backend_base_url.clone(), 30);
    let component_cache = ComponentCache::new(http.clone(), settings.backend_base_url.clone(), 30);
    let saved_outcome_cache =
        SavedOutcomeCache::new(http.clone(), settings.backend_base_url.clone(), 30);
    let sanitizer =
        rre_proxy::domain::applier::html_sanitizer::load_sanitizer("config/sanitizer.yaml")
            .unwrap();

    let state = AppState {
        settings: Arc::new(settings),
        http,
        site_map: Arc::new(site_map),
        backend: Arc::new(backend),
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
async fn generates_trace_id_when_absent() {
    let base = spawn().await;
    let res = reqwest::Client::new()
        .get(format!("{base}/health"))
        .send()
        .await
        .unwrap();
    assert!(res.headers().get(TRACE_HEADER).is_some());
}

#[tokio::test]
async fn echoes_inbound_trace_id() {
    let base = spawn().await;
    let res = reqwest::Client::new()
        .get(format!("{base}/health"))
        .header(TRACE_HEADER, "abc-123")
        .send()
        .await
        .unwrap();
    assert_eq!(
        res.headers()
            .get(TRACE_HEADER)
            .and_then(|v| v.to_str().ok()),
        Some("abc-123")
    );
}
