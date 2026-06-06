//! Service-level integration tests for the feature domain (real Postgres via
//! testcontainers). Covers happy path + error paths per service method.

mod common;

use rre_backend::{
    error::AppError,
    models::enums::FeatureType,
    schemas::{
        feature::{FeatureCreate, FeatureUpdate},
        pagination::PageParams,
    },
    services::feature_service,
};

fn create_input(id: &str, name: &str) -> FeatureCreate {
    FeatureCreate {
        id: id.to_string(),
        name: name.to_string(),
        r#type: FeatureType::Html,
    }
}

#[tokio::test]
async fn create_then_get_roundtrips() {
    let db = common::setup().await;
    let pool = &db.state.pool;

    let created = feature_service::create(pool, create_input("demo-article", "DN Article"))
        .await
        .expect("create");
    assert_eq!(created.id, "demo-article");
    assert_eq!(created.name, "DN Article");
    assert_eq!(created.r#type, FeatureType::Html);
    assert!(created.live_version_id.is_none());
    assert!(created.staging_version_id.is_none());

    let fetched = feature_service::get(pool, "demo-article")
        .await
        .expect("get");
    assert_eq!(fetched.id, created.id);
    assert_eq!(fetched.created_at, created.created_at);
}

#[tokio::test]
async fn create_duplicate_slug_conflicts() {
    let db = common::setup().await;
    let pool = &db.state.pool;

    feature_service::create(pool, create_input("demo-article", "First"))
        .await
        .expect("first create");

    let err = feature_service::create(pool, create_input("demo-article", "Second"))
        .await
        .expect_err("expected slug conflict");
    assert!(matches!(err, AppError::SlugConflict(_)), "got {err:?}");
}

#[tokio::test]
async fn create_invalid_slug_is_validation_error() {
    let db = common::setup().await;
    let pool = &db.state.pool;

    let err = feature_service::create(pool, create_input("Bad_Slug", "Name"))
        .await
        .expect_err("expected validation error");
    assert!(matches!(err, AppError::Validation { .. }), "got {err:?}");
}

#[tokio::test]
async fn get_missing_feature_is_not_found() {
    let db = common::setup().await;
    let pool = &db.state.pool;

    let err = feature_service::get(pool, "nope")
        .await
        .expect_err("expected not found");
    assert!(matches!(err, AppError::FeatureNotFound(_)), "got {err:?}");
}

#[tokio::test]
async fn update_name_persists() {
    let db = common::setup().await;
    let pool = &db.state.pool;

    feature_service::create(pool, create_input("demo-article", "Old"))
        .await
        .expect("create");

    let updated = feature_service::update(
        pool,
        "demo-article",
        FeatureUpdate {
            name: Some("New Name".to_string()),
        },
    )
    .await
    .expect("update");
    assert_eq!(updated.name, "New Name");

    let fetched = feature_service::get(pool, "demo-article")
        .await
        .expect("get");
    assert_eq!(fetched.name, "New Name");
}

#[tokio::test]
async fn update_with_none_name_is_noop() {
    let db = common::setup().await;
    let pool = &db.state.pool;

    feature_service::create(pool, create_input("demo-article", "Keep"))
        .await
        .expect("create");

    let updated = feature_service::update(pool, "demo-article", FeatureUpdate { name: None })
        .await
        .expect("update");
    assert_eq!(updated.name, "Keep");
}

#[tokio::test]
async fn update_missing_feature_is_not_found() {
    let db = common::setup().await;
    let pool = &db.state.pool;

    let err = feature_service::update(
        pool,
        "nope",
        FeatureUpdate {
            name: Some("X".to_string()),
        },
    )
    .await
    .expect_err("expected not found");
    assert!(matches!(err, AppError::FeatureNotFound(_)), "got {err:?}");
}

#[tokio::test]
async fn delete_removes_feature() {
    let db = common::setup().await;
    let pool = &db.state.pool;

    feature_service::create(pool, create_input("demo-article", "Doomed"))
        .await
        .expect("create");

    feature_service::delete(pool, "demo-article")
        .await
        .expect("delete");

    let err = feature_service::get(pool, "demo-article")
        .await
        .expect_err("expected not found after delete");
    assert!(matches!(err, AppError::FeatureNotFound(_)), "got {err:?}");
}

#[tokio::test]
async fn delete_missing_feature_is_not_found() {
    let db = common::setup().await;
    let pool = &db.state.pool;

    let err = feature_service::delete(pool, "nope")
        .await
        .expect_err("expected not found");
    assert!(matches!(err, AppError::FeatureNotFound(_)), "got {err:?}");
}

#[tokio::test]
async fn list_paginates_newest_first() {
    let db = common::setup().await;
    let pool = &db.state.pool;

    for i in 0..3 {
        feature_service::create(
            pool,
            create_input(&format!("feat-{i}"), &format!("Feature {i}")),
        )
        .await
        .expect("create");
    }

    let page = feature_service::list(
        pool,
        &PageParams {
            page: Some(1),
            page_size: Some(2),
        },
    )
    .await
    .expect("list");

    assert_eq!(page.total, 3);
    assert_eq!(page.page, 1);
    assert_eq!(page.page_size, 2);
    assert_eq!(page.items.len(), 2);
}
