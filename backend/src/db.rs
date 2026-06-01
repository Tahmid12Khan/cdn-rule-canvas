//! Database connectivity: pool construction and migration runner.

use sqlx::postgres::{PgPool, PgPoolOptions};
use sqlx::Executor;

/// Build a Postgres connection pool from the connection string.
///
/// Every connection's `search_path` is set to `public, rre` so unqualified
/// names resolve into the `rre` schema — in particular the sqlx-derived enum
/// type lookups (e.g. `version_status`, `feature_type`, `placement`), whose
/// declared `type_name` is unqualified.
///
/// `public` is listed FIRST so sqlx's `_sqlx_migrations` bookkeeping table always
/// lands in `public` (it is created on the very first connection, before the
/// `rre` schema exists). Listing `rre` first would place that table in `public`
/// on the first run but search `rre` on later runs, making migrations re-run.
///
/// # Errors
/// Returns a [`sqlx::Error`] if the pool cannot establish an initial connection.
pub async fn connect(database_url: &str, max_connections: u32) -> Result<PgPool, sqlx::Error> {
    PgPoolOptions::new()
        .max_connections(max_connections)
        .after_connect(|conn, _meta| {
            Box::pin(async move {
                conn.execute("SET search_path TO public, rre").await?;
                Ok(())
            })
        })
        .connect(database_url)
        .await
}

/// Run all embedded migrations (`./migrations`) against the pool.
///
/// Migrations are compiled into the binary via `sqlx::migrate!`, so the running
/// service needs no migration files on disk.
///
/// # Errors
/// Returns a [`sqlx::migrate::MigrateError`] if any migration fails to apply.
pub async fn run_migrations(pool: &PgPool) -> Result<(), sqlx::migrate::MigrateError> {
    sqlx::migrate!("./migrations").run(pool).await
}
