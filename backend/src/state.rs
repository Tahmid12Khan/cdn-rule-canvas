//! Shared application state injected into every handler via `State<AppState>`.

use std::sync::Arc;

use sqlx::PgPool;

use crate::config::Settings;

/// Cloneable handle to shared infrastructure: the DB pool and immutable settings.
///
/// Cheap to clone — `PgPool` is internally `Arc`-backed and `settings` is wrapped
/// in `Arc`.
#[derive(Clone)]
pub struct AppState {
    /// Postgres connection pool.
    pub pool: PgPool,
    /// Immutable runtime settings.
    pub settings: Arc<Settings>,
}

impl AppState {
    /// Construct application state from a pool and settings.
    pub fn new(pool: PgPool, settings: Settings) -> Self {
        Self {
            pool,
            settings: Arc::new(settings),
        }
    }
}
