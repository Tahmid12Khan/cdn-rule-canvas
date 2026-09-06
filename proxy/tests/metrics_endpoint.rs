use std::sync::Arc;

use rre_proxy::build_app;
use rre_proxy::config::Settings;
use rre_proxy::domain::processors::default_registry;
use rre_proxy::infra::backend_client::BackendClient;
use rre_proxy::infra::compiled_cache::CompiledCache;
use rre_proxy::infra::component_cache::ComponentCache;
use rre_proxy::infra::site_map::SiteMap;
use rre_proxy::state::AppState;

fn settings() -> Settings {
    Settings {
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
    }
}

#[tokio::test]
async fn metrics_endpoint_responds() {
    rre_proxy::observability::init();
    let http = reqwest::Client::new();
    let site_map = SiteMap::new(http.clone(), "http://127.0.0.1:1".to_string(), 30);
    let backend = BackendClient::new(http.clone(), "http://127.0.0.1:1".to_string(), 30);
    let component_cache = ComponentCache::new(http.clone(), "http://127.0.0.1:1".to_string(), 30);
    let sanitizer =
        rre_proxy::domain::applier::html_sanitizer::load_sanitizer("config/sanitizer.yaml")
            .unwrap();

    let state = AppState {
        settings: Arc::new(settings()),
        http,
        site_map: Arc::new(site_map),
        backend: Arc::new(backend),
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

    let res = reqwest::get(format!("http://{addr}/metrics"))
        .await
        .unwrap();
    // 200 if the recorder is installed, 503 otherwise — both are valid responses.
    assert!(res.status().is_success() || res.status().as_u16() == 503);
}
