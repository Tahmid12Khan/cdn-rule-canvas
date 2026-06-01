//! Application configuration, loaded once from the environment via `envy` + `dotenvy`.
//!
//! There is exactly one source of truth for runtime settings — no scattered
//! `std::env::var` calls in app code.

use serde::Deserialize;

/// Strongly-typed runtime settings.
///
/// Populated by [`Settings::load`] from environment variables (field names are
/// upper-cased by `envy`, e.g. `database_url` ⇐ `DATABASE_URL`).
#[derive(Debug, Clone, Deserialize)]
pub struct Settings {
    /// Postgres connection string for sqlx.
    pub database_url: String,
    /// Deployment environment label: `dev` | `staging` | `prod`.
    #[serde(default = "default_app_env")]
    pub app_env: String,
    /// Application version reported by `/health`.
    #[serde(default = "default_app_version")]
    pub app_version: String,
    /// Git commit reported by `/health`.
    #[serde(default = "default_git_commit")]
    pub git_commit: String,
    /// CORS allow-origin for the admin frontend.
    #[serde(default = "default_frontend_origin")]
    pub frontend_origin: String,
    /// Axum bind address.
    #[serde(default = "default_bind_addr")]
    pub bind_addr: String,
    /// sqlx connection pool size.
    #[serde(default = "default_db_max_connections")]
    pub db_max_connections: u32,
}

impl Settings {
    /// Load `.env` (if present) then deserialize the process environment.
    ///
    /// # Errors
    /// Returns an error if a required variable (`DATABASE_URL`) is missing or a
    /// value fails to parse.
    pub fn load() -> Result<Self, envy::Error> {
        dotenvy::dotenv().ok();
        envy::from_env::<Settings>()
    }

    /// True when running in a development environment (pretty logs).
    pub fn is_dev(&self) -> bool {
        self.app_env == "dev" || self.app_env == "development" || self.app_env == "local"
    }
}

fn default_app_env() -> String {
    "dev".to_string()
}

fn default_app_version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}

fn default_git_commit() -> String {
    "unknown".to_string()
}

fn default_frontend_origin() -> String {
    "http://localhost:3000".to_string()
}

fn default_bind_addr() -> String {
    "0.0.0.0:8000".to_string()
}

fn default_db_max_connections() -> u32 {
    10
}
