//! Proxy configuration. A single `Settings` struct is loaded once at startup via
//! the layered `config` crate: `config/default.json` (committed base) is overlaid
//! by `config/{APP_ENV}.json` (optional per-profile), then by environment
//! variables (top layer — secrets / per-deploy overrides). No scattered
//! `std::env::var` calls live in app code.

use serde::Deserialize;

/// Process-wide configuration. Field meanings are unchanged from the previous
/// `envy` layout; values now live in `config/default.json` + profile files, with
/// environment variables as the top override layer.
#[derive(Debug, Clone, Deserialize)]
pub struct Settings {
    pub proxy_bind_addr: String,
    pub upstream_base_url: String,
    pub backend_base_url: String,
    pub app_env: String,
    pub active_version_ttl_secs: u64,
    pub compiled_cache_capacity: u64,
    pub upstream_connect_timeout_secs: u64,
    pub upstream_read_timeout_secs: u64,
    pub feature_map_path: String,
    pub sanitizer_config_path: String,
}

impl Settings {
    /// Load layered configuration: `proxy/config/default.json`, then the optional
    /// `proxy/config/{APP_ENV}.json` profile, then environment variables.
    /// `APP_ENV` defaults to `dev`. Nested keys use `__` as the env separator.
    /// Paths resolve relative to the working directory (repo root in dev, `/app`
    /// in the container), matching the existing `proxy/config/*.yaml` convention.
    pub fn load() -> anyhow::Result<Self> {
        let env = std::env::var("APP_ENV").unwrap_or_else(|_| "dev".to_string());
        let settings = config::Config::builder()
            .add_source(config::File::with_name("proxy/config/default"))
            .add_source(config::File::with_name(&format!("proxy/config/{env}")).required(false))
            .add_source(config::Environment::default().separator("__"))
            .build()
            .map_err(|e| anyhow::anyhow!("invalid configuration: {e}"))?;
        settings
            .try_deserialize()
            .map_err(|e| anyhow::anyhow!("invalid configuration: {e}"))
    }

    /// True when running in a development environment (pretty logs).
    pub fn is_dev(&self) -> bool {
        self.app_env.eq_ignore_ascii_case("dev")
    }
}
