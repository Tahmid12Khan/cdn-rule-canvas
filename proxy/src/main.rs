//! RRE proxy entrypoint. Loads config, initializes telemetry + metrics, builds
//! the shared `AppState` (reqwest client, site map, caches, registry,
//! sanitizer), and serves the app on the configured bind address (`:9000`).

use std::sync::Arc;
use std::time::Duration;

use anyhow::Context;
use rre_proxy::config::Settings;
use rre_proxy::domain::processors::default_registry;
use rre_proxy::infra::backend_client::BackendClient;
use rre_proxy::infra::compiled_cache::CompiledCache;
use rre_proxy::infra::component_cache::ComponentCache;
use rre_proxy::infra::site_map::SiteMap;
use rre_proxy::state::AppState;
use rre_proxy::{build_app, observability, telemetry};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenvy::dotenv().ok();

    let settings = Settings::load()?;
    telemetry::init(settings.is_dev());
    observability::init();

    // One reqwest client per process (connection pool), shared via AppState.
    // A reverse proxy must NOT follow upstream 3xx redirects — that turns a
    // malicious/compromised `Location` (169.254.169.254, RFC1918, the backend)
    // into an SSRF pivot. Disable redirect-following and surface the 3xx to the
    // client unchanged.
    let http = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .connect_timeout(Duration::from_secs(settings.upstream_connect_timeout_secs))
        .timeout(Duration::from_secs(settings.upstream_read_timeout_secs))
        .build()
        .context("building reqwest client")?;

    let site_map = SiteMap::new(
        http.clone(),
        settings.backend_base_url.clone(),
        settings.active_version_ttl_secs,
    );

    let backend = BackendClient::new(
        http.clone(),
        settings.backend_base_url.clone(),
        settings.active_version_ttl_secs,
    );

    let component_cache = ComponentCache::new(
        http.clone(),
        settings.backend_base_url.clone(),
        settings.active_version_ttl_secs,
    );

    let compiled = CompiledCache::new(settings.compiled_cache_capacity);

    let registry = default_registry();

    let sanitizer =
        rre_proxy::domain::applier::html_sanitizer::load_sanitizer(&settings.sanitizer_config_path)
            .with_context(|| {
                format!("loading sanitizer from {}", settings.sanitizer_config_path)
            })?;

    let bind_addr = settings.proxy_bind_addr.clone();

    let state = AppState {
        settings: Arc::new(settings),
        http,
        site_map: Arc::new(site_map),
        backend: Arc::new(backend),
        compiled: Arc::new(compiled),
        component_cache: Arc::new(component_cache),
        registry: Arc::new(registry),
        sanitizer: Arc::new(sanitizer),
    };

    let app = build_app(state);

    let listener = tokio::net::TcpListener::bind(&bind_addr)
        .await
        .with_context(|| format!("binding {bind_addr}"))?;
    tracing::info!(addr = %bind_addr, "rre-proxy listening");
    axum::serve(listener, app).await.context("server error")?;

    Ok(())
}
