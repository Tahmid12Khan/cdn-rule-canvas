//! Service-level integration tests for the test-preset domain (real Postgres via
//! testcontainers). Covers happy path + error paths per service method,
//! including the `name ILIKE '%q%'` + `kind` filters and the 409 conflict mapping.

mod common;

use rre_backend::{
    error::AppError,
    schemas::{
        pagination::PageParams,
        test_preset::{TestPresetCreate, TestPresetUpdate},
    },
    services::test_preset_service,
};
use serde_json::json;

fn create_input(slug: &str, name: &str, kind: &str) -> TestPresetCreate {
    TestPresetCreate {
        slug: slug.to_string(),
        name: name.to_string(),
        kind: kind.to_string(),
        payload: json!({ "feature_type": "html" }),
    }
}

#[tokio::test]
async fn create_then_get_roundtrips() {
    let db = common::setup().await;
    let pool = &db.state.pool;

    let created =
        test_preset_service::create(pool, create_input("mobile-paywall", "Mobile", "rule"))
            .await
            .expect("create");
    assert_eq!(created.slug, "mobile-paywall");
    assert_eq!(created.kind, "rule");
    assert_eq!(created.payload["feature_type"], "html");

    let fetched = test_preset_service::get(pool, "mobile-paywall")
        .await
        .expect("get");
    assert_eq!(fetched.slug, created.slug);
    assert_eq!(fetched.created_at, created.created_at);
}

#[tokio::test]
async fn create_duplicate_slug_conflicts() {
    let db = common::setup().await;
    let pool = &db.state.pool;

    test_preset_service::create(pool, create_input("dup-slug", "First", "rule"))
        .await
        .expect("first create");

    // Same slug, different name → still a PK conflict.
    let err = test_preset_service::create(pool, create_input("dup-slug", "Second", "rule"))
        .await
        .expect_err("expected slug conflict");
    assert!(matches!(err, AppError::SlugConflict(_)), "got {err:?}");
}

#[tokio::test]
async fn create_duplicate_name_conflicts() {
    let db = common::setup().await;
    let pool = &db.state.pool;

    test_preset_service::create(pool, create_input("preset-a", "Shared", "rule"))
        .await
        .expect("first create");

    // Different slug but same name → name unique index conflict mapped to 409.
    let err = test_preset_service::create(pool, create_input("preset-b", "Shared", "url"))
        .await
        .expect_err("expected name conflict");
    assert!(matches!(err, AppError::SlugConflict(_)), "got {err:?}");
}

#[tokio::test]
async fn create_invalid_slug_is_validation_error() {
    let db = common::setup().await;
    let pool = &db.state.pool;

    let err = test_preset_service::create(pool, create_input("AB", "Bad", "rule"))
        .await
        .expect_err("expected validation error");
    assert!(matches!(err, AppError::Validation { .. }), "got {err:?}");
}

#[tokio::test]
async fn create_invalid_kind_is_validation_error() {
    let db = common::setup().await;
    let pool = &db.state.pool;

    let err = test_preset_service::create(pool, create_input("demo-preset", "Bad", "other"))
        .await
        .expect_err("expected validation error");
    assert!(matches!(err, AppError::Validation { .. }), "got {err:?}");
}

#[tokio::test]
async fn create_non_object_payload_is_validation_error() {
    let db = common::setup().await;
    let pool = &db.state.pool;

    let mut input = create_input("demo-preset", "Demo", "rule");
    input.payload = json!([1, 2, 3]);

    let err = test_preset_service::create(pool, input)
        .await
        .expect_err("expected validation error");
    assert!(matches!(err, AppError::Validation { .. }), "got {err:?}");
}

#[tokio::test]
async fn create_oversized_payload_is_validation_error() {
    let db = common::setup().await;
    let pool = &db.state.pool;

    let mut input = create_input("demo-preset", "Demo", "rule");
    input.payload = json!({ "blob": "a".repeat(16_385) });

    let err = test_preset_service::create(pool, input)
        .await
        .expect_err("expected validation error");
    assert!(matches!(err, AppError::Validation { .. }), "got {err:?}");
}

#[tokio::test]
async fn list_filters_by_name_q_and_kind() {
    let db = common::setup().await;
    let pool = &db.state.pool;

    test_preset_service::create(pool, create_input("alpha-rule", "Alpha News", "rule"))
        .await
        .expect("create alpha");
    test_preset_service::create(pool, create_input("beta-url", "Beta Times", "url"))
        .await
        .expect("create beta");

    // Case-insensitive substring match on name.
    let by_q = test_preset_service::list(pool, &PageParams::default(), Some("alpha"), None)
        .await
        .expect("list by q");
    assert_eq!(by_q.total, 1);
    assert_eq!(by_q.items[0].slug, "alpha-rule");

    // Kind filter.
    let by_kind = test_preset_service::list(pool, &PageParams::default(), None, Some("url"))
        .await
        .expect("list by kind");
    assert_eq!(by_kind.total, 1);
    assert_eq!(by_kind.items[0].slug, "beta-url");

    // Empty q/kind treated as no filter.
    let all = test_preset_service::list(pool, &PageParams::default(), Some(""), Some(""))
        .await
        .expect("list all");
    assert_eq!(all.total, 2);

    let none = test_preset_service::list(pool, &PageParams::default(), None, None)
        .await
        .expect("list none-filter");
    assert_eq!(none.total, 2);
}

#[tokio::test]
async fn update_name_only_leaves_payload_and_kind() {
    let db = common::setup().await;
    let pool = &db.state.pool;

    test_preset_service::create(pool, create_input("mobile-paywall", "Old", "rule"))
        .await
        .expect("create");

    let updated = test_preset_service::update(
        pool,
        "mobile-paywall",
        TestPresetUpdate {
            name: Some("Renamed".to_string()),
            ..TestPresetUpdate::default()
        },
    )
    .await
    .expect("update");
    assert_eq!(updated.name, "Renamed");
    assert_eq!(updated.kind, "rule", "kind immutable");
    assert_eq!(updated.payload["feature_type"], "html", "payload unchanged");
}

#[tokio::test]
async fn update_payload_only_replaces_payload() {
    let db = common::setup().await;
    let pool = &db.state.pool;

    test_preset_service::create(pool, create_input("mobile-paywall", "Keep", "rule"))
        .await
        .expect("create");

    let updated = test_preset_service::update(
        pool,
        "mobile-paywall",
        TestPresetUpdate {
            payload: Some(json!({ "path": "/article" })),
            ..TestPresetUpdate::default()
        },
    )
    .await
    .expect("update");
    assert_eq!(updated.name, "Keep", "name unchanged");
    assert_eq!(updated.payload, json!({ "path": "/article" }));
}

#[tokio::test]
async fn update_empty_is_noop_returning_current() {
    let db = common::setup().await;
    let pool = &db.state.pool;

    test_preset_service::create(pool, create_input("mobile-paywall", "Keep", "rule"))
        .await
        .expect("create");

    let updated = test_preset_service::update(pool, "mobile-paywall", TestPresetUpdate::default())
        .await
        .expect("noop update");
    assert_eq!(updated.name, "Keep");
}

#[tokio::test]
async fn update_with_invalid_payload_is_validation_error() {
    let db = common::setup().await;
    let pool = &db.state.pool;

    test_preset_service::create(pool, create_input("mobile-paywall", "Demo", "rule"))
        .await
        .expect("create");

    let err = test_preset_service::update(
        pool,
        "mobile-paywall",
        TestPresetUpdate {
            payload: Some(json!("not-an-object")),
            ..TestPresetUpdate::default()
        },
    )
    .await
    .expect_err("expected validation error");
    assert!(matches!(err, AppError::Validation { .. }), "got {err:?}");
}

#[tokio::test]
async fn update_missing_is_not_found() {
    let db = common::setup().await;
    let pool = &db.state.pool;

    let err = test_preset_service::update(
        pool,
        "ghost",
        TestPresetUpdate {
            name: Some("X".to_string()),
            ..TestPresetUpdate::default()
        },
    )
    .await
    .expect_err("expected not found");
    assert!(
        matches!(err, AppError::TestPresetNotFound(_)),
        "got {err:?}"
    );
}

#[tokio::test]
async fn delete_then_get_is_not_found() {
    let db = common::setup().await;
    let pool = &db.state.pool;

    test_preset_service::create(pool, create_input("mobile-paywall", "Doomed", "rule"))
        .await
        .expect("create");

    test_preset_service::delete(pool, "mobile-paywall")
        .await
        .expect("delete");

    let err = test_preset_service::get(pool, "mobile-paywall")
        .await
        .expect_err("expected not found");
    assert!(
        matches!(err, AppError::TestPresetNotFound(_)),
        "got {err:?}"
    );
}

#[tokio::test]
async fn delete_missing_is_not_found() {
    let db = common::setup().await;
    let pool = &db.state.pool;

    let err = test_preset_service::delete(pool, "ghost")
        .await
        .expect_err("expected not found");
    assert!(
        matches!(err, AppError::TestPresetNotFound(_)),
        "got {err:?}"
    );
}
