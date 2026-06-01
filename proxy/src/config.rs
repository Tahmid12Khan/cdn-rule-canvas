//! Proxy configuration. A single `Settings` struct is loaded once at startup via
//! `dotenvy::dotenv().ok()` + `envy::from_env::<Settings>()`. No scattered
//! `std::env::var` calls live in app code.

use serde::Deserialize;

fn default_bind_addr() -> String {
    "0.0.0.0:9000".to_string()
}
fn default_upstream_base_url() -> String {
    "http://demo-upstream:8081".to_string()
}
fn default_backend_base_url() -> String {
    "http://backend:8000".to_string()
}
fn default_app_env() -> String {
    "dev".to_string()
}
fn default_active_version_ttl_secs() -> u64 {
    30
}
fn default_compiled_cache_capacity() -> u64 {
    256
}
fn default_upstream_connect_timeout_secs() -> u64 {
    2
}
fn default_upstream_read_timeout_secs() -> u64 {
    10
}
fn default_feature_map_path() -> String {
    "proxy/config/feature_map.yaml".to_string()
}
fn default_sanitizer_config_path() -> String {
    "proxy/config/sanitizer.yaml".to_string()
}

/// Process-wide configuration, deserialized from the environment.
#[derive(Debug, Clone, Deserialize)]
pub struct Settings {
    #[serde(default = "default_bind_addr")]
    pub proxy_bind_addr: String,
    #[serde(default = "default_upstream_base_url")]
    pub upstream_base_url: String,
    #[serde(default = "default_backend_base_url")]
    pub backend_base_url: String,
    #[serde(default = "default_app_env")]
    pub app_env: String,
    #[serde(default = "default_active_version_ttl_secs")]
    pub active_version_ttl_secs: u64,
    #[serde(default = "default_compiled_cache_capacity")]
    pub compiled_cache_capacity: u64,
    #[serde(default = "default_upstream_connect_timeout_secs")]
    pub upstream_connect_timeout_secs: u64,
    #[serde(default = "default_upstream_read_timeout_secs")]
    pub upstream_read_timeout_secs: u64,
    #[serde(default = "default_feature_map_path")]
    pub feature_map_path: String,
    #[serde(default = "default_sanitizer_config_path")]
    pub sanitizer_config_path: String,
}

impl Settings {
    /// Load settings from the environment (after `dotenvy::dotenv().ok()`).
    pub fn from_env() -> anyhow::Result<Self> {
        envy::from_env::<Settings>().map_err(|e| anyhow::anyhow!("invalid configuration: {e}"))
    }

    /// True when running in a development environment (pretty logs).
    pub fn is_dev(&self) -> bool {
        self.app_env.eq_ignore_ascii_case("dev")
    }
}
