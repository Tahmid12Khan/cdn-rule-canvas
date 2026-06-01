//! RRE backend entrypoint: load config, init telemetry, connect the pool, run
//! migrations, build the app, and serve.

#![forbid(unsafe_code)]

use std::net::SocketAddr;

use anyhow::Context;
use rre_backend::{build_app, config::Settings, db, state::AppState, telemetry};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let settings = Settings::load().context("failed to load settings")?;
    telemetry::init(settings.is_dev());

    tracing::info!(
        app_env = %settings.app_env,
        app_version = %settings.app_version,
        "starting rre-backend"
    );

    let pool = db::connect(&settings.database_url, settings.db_max_connections)
        .await
        .context("failed to connect to database")?;

    db::run_migrations(&pool)
        .await
        .context("failed to run migrations")?;

    let bind_addr: SocketAddr = settings
        .bind_addr
        .parse()
        .with_context(|| format!("invalid BIND_ADDR: {}", settings.bind_addr))?;

    let app = build_app(AppState::new(pool, settings));

    let listener = tokio::net::TcpListener::bind(bind_addr)
        .await
        .with_context(|| format!("failed to bind {bind_addr}"))?;

    tracing::info!(%bind_addr, "listening");
    axum::serve(listener, app).await.context("server error")?;

    Ok(())
}
