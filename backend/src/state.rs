//! Shared application state injected into every handler via `State<AppState>`.

use std::sync::Arc;

use serde_json::Value;
use sqlx::PgPool;

use crate::config::Settings;
use crate::schemas::node_type::{LoadedManifest, NodeManifest};

/// Cloneable handle to shared infrastructure: the DB pool, immutable settings,
/// and the node-type manifest.
///
/// Cheap to clone — `PgPool` is internally `Arc`-backed and `settings` /
/// manifest are wrapped in `Arc`.
#[derive(Clone)]
pub struct AppState {
    /// Postgres connection pool.
    pub pool: PgPool,
    /// Immutable runtime settings.
    pub settings: Arc<Settings>,
    /// Typed node-type manifest (drives manifest-driven validation).
    pub node_manifest: Arc<NodeManifest>,
    /// Raw node-type manifest JSON, served verbatim at `GET /api/v1/node-types`.
    pub node_manifest_json: Arc<Value>,
}

impl AppState {
    /// Construct application state, loading the node-type manifest from the path
    /// in `settings`.
    ///
    /// # Errors
    /// Returns an error if the node-type manifest file is missing or malformed.
    pub fn new(pool: PgPool, settings: Settings) -> anyhow::Result<Self> {
        let manifest = LoadedManifest::load(&settings.node_manifest_path)?;
        Ok(Self {
            pool,
            settings: Arc::new(settings),
            node_manifest: manifest.typed,
            node_manifest_json: manifest.raw,
        })
    }
}
