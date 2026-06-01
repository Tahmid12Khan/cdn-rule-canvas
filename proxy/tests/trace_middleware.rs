//! Verifies the trace-id middleware echoes a generated id and respects an
//! inbound one, against the `/health` route (no upstream needed).

use std::sync::Arc;

use rre_proxy::build_app;
use rre_proxy::config::Settings;
use rre_proxy::domain::processors::default_registry;
use rre_proxy::infra::backend_client::BackendClient;
use rre_proxy::infra::compiled_cache::CompiledCache;
use rre_proxy::infra::feature_map::{FeatureMap, FeatureMapEntry};
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
        feature_map_path: "config/feature_map.yaml".to_string(),
        sanitizer_config_path: "config/sanitizer.yaml".to_string(),
    };
    let http = reqwest::Client::new();
    let feature_map = FeatureMap::from_entries(vec![FeatureMapEntry {
        host: "ignored".to_string(),
        path_glob: "/never".to_string(),
        feature_id: "x".to_string(),
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
