//! Infrastructure layer: config-file loaders, the backend HTTP client, and the
//! two caches (active-version TTL cache + compiled-graph LRU cache), plus body
//! encoding helpers. No business logic lives here.

pub mod backend_client;
pub mod compiled_cache;
pub mod encoding;
pub mod feature_map;
