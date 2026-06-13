//! Shared application state. Cloned cheaply per request (everything behind `Arc`).

use std::sync::Arc;

use crate::config::Settings;
use crate::domain::processors::ProcessorRegistry;
use crate::infra::backend_client::BackendClient;
use crate::infra::compiled_cache::CompiledCache;
use crate::infra::component_cache::ComponentCache;
use crate::infra::site_map::SiteMap;

#[derive(Clone)]
pub struct AppState {
    pub settings: Arc<Settings>,
    /// One reqwest client per process — holds the connection pool.
    pub http: reqwest::Client,
    /// Dynamic source-host -> destination routing table (TTL-cached from the
    /// backend). Replaces the static `feature_map.yaml`.
    pub site_map: Arc<SiteMap>,
    pub backend: Arc<BackendClient>,
    pub compiled: Arc<CompiledCache>,
    /// SWR cache of resolved Component templates (Component Editor §4.1), keyed by
    /// `(component_id, VersionSelector)`. Pre-resolved on the async side before the
    /// sync applier renders `apply_component` / `apply_component_json` actions.
    pub component_cache: Arc<ComponentCache>,
    pub registry: Arc<ProcessorRegistry>,
    /// Built once at startup from `proxy/config/sanitizer.yaml`.
    pub sanitizer: Arc<ammonia::Builder<'static>>,
}
