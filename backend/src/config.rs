//! Application configuration, loaded once from layered JSON files via the
//! `config` crate (BACKEND CONTRACT §0).
//!
//! Layers, highest-precedence last:
//!   `config/default.json` (committed base)
//!   → `config/{APP_ENV}.json` (profile, `APP_ENV` default `dev`, optional)
//!   → environment variables (top layer — secrets like `DATABASE_URL` and
//!     per-deploy overrides; nested keys separated by `__`).
//!
//! `.env` is loaded by `dotenvy` only to populate env vars before the env layer
//! is read. There is exactly one source of truth for runtime settings — no
//! scattered `std::env::var` calls in app code. Config file paths resolve
//! relative to the working directory.

use serde::Deserialize;

/// Strongly-typed runtime settings.
///
/// Populated by [`Settings::load`] from the layered JSON config files plus the
/// environment (env keys are upper-cased, e.g. `database_url` ⇐ `DATABASE_URL`).
#[derive(Debug, Clone, Deserialize)]
pub struct Settings {
    /// Postgres connection string for sqlx (env-only secret).
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
    /// Path to the node-type manifest JSON, resolved relative to the working dir.
    #[serde(default = "default_node_manifest_path")]
    pub node_manifest_path: String,
}

impl Settings {
    /// Load `.env` (if present), then the layered JSON config files plus the
    /// environment overrides.
    ///
    /// # Errors
    /// Returns an error if the config files are malformed, a required value
    /// (`DATABASE_URL`) is missing, or a value fails to parse.
    pub fn load() -> anyhow::Result<Self> {
        dotenvy::dotenv().ok();
        let env = std::env::var("APP_ENV").unwrap_or_else(|_| default_app_env());
        let settings = config::Config::builder()
            .add_source(config::File::with_name("config/default"))
            .add_source(config::File::with_name(&format!("config/{env}")).required(false))
            .add_source(config::Environment::default().separator("__"))
            .build()?
            .try_deserialize::<Settings>()?;
        Ok(settings)
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

fn default_node_manifest_path() -> String {
    "config/node_types.json".to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The layered `config` builder reads `config/default.json`, applies the
    /// env-only `DATABASE_URL` secret, and an env override wins over the JSON
    /// base value. Runs against the committed `config/` dir (CWD = crate root).
    #[test]
    fn load_layers_json_then_env() {
        // Serialize against the shared process environment.
        let url = "postgres://t:t@localhost:5432/t";
        std::env::set_var("DATABASE_URL", url);
        std::env::set_var("APP_ENV", "dev");
        std::env::set_var("FRONTEND_ORIGIN", "http://override.test");

        let settings = Settings::load().expect("load layered settings");

        assert_eq!(settings.database_url, url);
        // Env override beats the JSON layer.
        assert_eq!(settings.frontend_origin, "http://override.test");
        // JSON-layer defaults are honored.
        assert_eq!(settings.bind_addr, "0.0.0.0:8000");
        assert_eq!(settings.db_max_connections, 10);
        assert_eq!(settings.node_manifest_path, "config/node_types.json");
        assert!(settings.is_dev());

        std::env::remove_var("DATABASE_URL");
        std::env::remove_var("FRONTEND_ORIGIN");
    }
}
