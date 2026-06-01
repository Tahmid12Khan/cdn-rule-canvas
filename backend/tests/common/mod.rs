//! Shared test helpers: spin up an ephemeral Postgres via testcontainers,
//! connect a pool, and run migrations. Used by integration tests.
//!
//! Each test that needs a DB calls [`setup`], which returns a live [`AppState`]
//! plus the container guard (keep it alive for the test's duration).

#![allow(dead_code)]

use rre_backend::{config::Settings, db, state::AppState};
use testcontainers_modules::{
    postgres::Postgres,
    testcontainers::{runners::AsyncRunner, ContainerAsync, ImageExt},
};

/// A running Postgres container plus a connected, migrated [`AppState`].
pub struct TestDb {
    /// Keep the container alive for the lifetime of the test.
    pub container: ContainerAsync<Postgres>,
    /// Application state wired to the container's pool.
    pub state: AppState,
}

/// Start Postgres, connect, run migrations, and build an [`AppState`].
pub async fn setup() -> TestDb {
    let container = Postgres::default()
        .with_tag("16-alpine")
        .start()
        .await
        .expect("start postgres container");

    let host = container.get_host().await.expect("container host");
    let port = container
        .get_host_port_ipv4(5432)
        .await
        .expect("container port");

    let database_url = format!("postgres://postgres:postgres@{host}:{port}/postgres");

    let pool = db::connect(&database_url, 5).await.expect("connect pool");
    db::run_migrations(&pool).await.expect("run migrations");

    let settings = Settings {
        database_url,
        app_env: "test".to_string(),
        app_version: "test".to_string(),
        git_commit: "test".to_string(),
        frontend_origin: "http://localhost:3000".to_string(),
        bind_addr: "0.0.0.0:0".to_string(),
        db_max_connections: 5,
        node_manifest_path: "config/node_types.json".to_string(),
    };

    let state = AppState::new(pool, settings).expect("load node-type manifest");
    TestDb { container, state }
}
