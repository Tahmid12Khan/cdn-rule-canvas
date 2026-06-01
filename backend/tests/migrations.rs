//! Migrations apply cleanly and create the `rre` schema, enum types, and tables.

mod common;

#[tokio::test]
async fn migrations_create_schema_and_enums() {
    let db = common::setup().await;
    let pool = &db.state.pool;

    // Schema exists.
    let schema_exists: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM information_schema.schemata WHERE schema_name = 'rre')",
    )
    .fetch_one(pool)
    .await
    .unwrap();
    assert!(schema_exists, "rre schema should exist");

    // The three enum types exist in the rre schema.
    for ty in ["feature_type", "version_status", "placement"] {
        let exists: bool = sqlx::query_scalar(
            "SELECT EXISTS(\
                SELECT 1 FROM pg_type t \
                JOIN pg_namespace n ON n.oid = t.typnamespace \
                WHERE n.nspname = 'rre' AND t.typname = $1)",
        )
        .bind(ty)
        .fetch_one(pool)
        .await
        .unwrap();
        assert!(exists, "enum type rre.{ty} should exist");
    }

    // Core tables exist (created by domain-owned migrations 0002-0004).
    for table in ["features", "versions", "outcomes", "components"] {
        let exists: bool = sqlx::query_scalar(
            "SELECT EXISTS(\
                SELECT 1 FROM information_schema.tables \
                WHERE table_schema = 'rre' AND table_name = $1)",
        )
        .bind(table)
        .fetch_one(pool)
        .await
        .unwrap();
        assert!(exists, "table rre.{table} should exist");
    }
}
