//! Verifies the `seed_demo` binary is idempotent: running it twice against the
//! same database leaves exactly one demo feature, the same version/outcome/
//! component counts, and never errors on the second pass.
//!
//! Uses an ephemeral Postgres (testcontainers) and invokes the *compiled*
//! `seed_demo` binary via `CARGO_BIN_EXE_seed_demo` (Cargo sets this for
//! integration tests), so the real seeding code path is exercised end-to-end.

mod common;

use std::process::Command;

use sqlx::Row;

#[tokio::test]
async fn seed_demo_is_idempotent() {
    let db = common::setup().await;
    let pool = db.state.pool.clone();
    let database_url = db.state.settings.database_url.clone();

    // Run the seed binary twice.
    run_seed(&database_url);
    run_seed(&database_url);

    // Exactly one demo feature.
    let feature_count: i64 =
        sqlx::query("SELECT count(*) FROM rre.features WHERE id = 'dn-article'")
            .fetch_one(&pool)
            .await
            .expect("count features")
            .get(0);
    assert_eq!(feature_count, 1, "exactly one dn-article feature expected");

    // Two versions: LIVE v1 + DRAFT v2.
    let version_count: i64 =
        sqlx::query("SELECT count(*) FROM rre.versions WHERE feature_id = 'dn-article'")
            .fetch_one(&pool)
            .await
            .expect("count versions")
            .get(0);
    assert_eq!(version_count, 2, "two versions after re-seeding");

    let live_count: i64 = sqlx::query(
        "SELECT count(*) FROM rre.versions WHERE feature_id = 'dn-article' AND status = 'live'",
    )
    .fetch_one(&pool)
    .await
    .expect("count live versions")
    .get(0);
    assert_eq!(live_count, 1, "exactly one LIVE version");

    // v1 (LIVE) has its three outcomes.
    let v1_outcomes: i64 = sqlx::query(
        "SELECT count(*) FROM rre.outcomes o \
         JOIN rre.versions v ON v.id = o.version_id \
         WHERE v.feature_id = 'dn-article' AND v.status = 'live'",
    )
    .fetch_one(&pool)
    .await
    .expect("count v1 outcomes")
    .get(0);
    assert_eq!(v1_outcomes, 3, "three outcomes on the live version");

    // Components are stable (3 total on the live version's outcomes).
    let v1_components: i64 = sqlx::query(
        "SELECT count(*) FROM rre.components c \
         JOIN rre.outcomes o ON o.id = c.outcome_id \
         JOIN rre.versions v ON v.id = o.version_id \
         WHERE v.feature_id = 'dn-article' AND v.status = 'live'",
    )
    .fetch_one(&pool)
    .await
    .expect("count v1 components")
    .get(0);
    assert_eq!(v1_components, 3, "three components on the live version");

    // The feature's live slot points at the live version.
    let live_id_matches: bool = sqlx::query(
        "SELECT f.live_version_id = v.id \
         FROM rre.features f \
         JOIN rre.versions v ON v.feature_id = f.id AND v.status = 'live' \
         WHERE f.id = 'dn-article'",
    )
    .fetch_one(&pool)
    .await
    .expect("check live slot")
    .get(0);
    assert!(
        live_id_matches,
        "features.live_version_id points at the LIVE version"
    );
}

fn run_seed(database_url: &str) {
    let bin = env!("CARGO_BIN_EXE_seed_demo");
    let status = Command::new(bin)
        .env("DATABASE_URL", database_url)
        .env("APP_ENV", "test")
        .status()
        .expect("spawn seed_demo binary");
    assert!(status.success(), "seed_demo exited non-zero");
}
