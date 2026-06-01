//! Tracing/log initialization. JSON layer in non-dev, pretty in dev.

use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

/// Initialize the global tracing subscriber.
///
/// Honors `RUST_LOG`; defaults to `info` for app + `warn` for noisy crates.
/// `is_dev` selects a human-readable pretty format; otherwise structured JSON.
pub fn init(is_dev: bool) {
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("info,sqlx=warn,tower_http=info,hyper=warn"));

    let registry = tracing_subscriber::registry().with(filter);

    if is_dev {
        registry
            .with(tracing_subscriber::fmt::layer().pretty())
            .init();
    } else {
        registry
            .with(tracing_subscriber::fmt::layer().json())
            .init();
    }
}
