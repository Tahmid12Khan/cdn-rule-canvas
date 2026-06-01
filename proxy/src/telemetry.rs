//! Tracing setup. JSON logs in production, pretty logs in dev (gated by APP_ENV).
//! NEVER log raw request/response bodies or cookie values — structured fields only.

use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::EnvFilter;

/// Initialize the global tracing subscriber. Idempotent-safe per process
/// (calling twice will error from `try_init`, which we ignore).
pub fn init(is_dev: bool) {
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("info,rre_proxy=debug"));

    let registry = tracing_subscriber::registry().with(filter);

    if is_dev {
        let _ = registry
            .with(tracing_subscriber::fmt::layer().pretty())
            .try_init();
    } else {
        let _ = registry
            .with(tracing_subscriber::fmt::layer().json())
            .try_init();
    }
}
