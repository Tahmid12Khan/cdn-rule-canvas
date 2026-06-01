//! Prometheus metrics. A single recorder handle is installed at startup; the
//! `/metrics` route renders it. Metric names follow the proxy contract §10.

use axum::http::StatusCode;
use metrics_exporter_prometheus::{PrometheusBuilder, PrometheusHandle};
use std::sync::OnceLock;

static HANDLE: OnceLock<PrometheusHandle> = OnceLock::new();

/// Install the global Prometheus recorder once. Safe to call multiple times;
/// subsequent calls are no-ops.
pub fn init() {
    if HANDLE.get().is_some() {
        return;
    }
    let recorder = PrometheusBuilder::new();
    if let Ok(handle) = recorder.install_recorder() {
        let _ = HANDLE.set(handle);
    }
}

/// GET /metrics — renders the Prometheus exposition text.
pub async fn metrics_handler() -> (StatusCode, String) {
    match HANDLE.get() {
        Some(h) => (StatusCode::OK, h.render()),
        None => (
            StatusCode::SERVICE_UNAVAILABLE,
            "metrics recorder not installed".to_string(),
        ),
    }
}
