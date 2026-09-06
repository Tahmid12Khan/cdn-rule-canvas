//! Infrastructure layer: the backend HTTP client, the dynamic site-routing map,
//! and the two caches (active-version TTL cache + compiled-graph LRU cache), plus
//! body encoding helpers. No business logic lives here.

pub mod backend_client;
pub mod compiled_cache;
pub mod component_cache;
pub mod encoding;
pub mod saved_outcome_cache;
pub mod site_map;
