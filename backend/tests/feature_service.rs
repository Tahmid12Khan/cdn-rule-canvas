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
        execution_order: None,
    }
}

fn create_input_typed(id: &str, name: &str, r#type: FeatureType) -> FeatureCreate {
    FeatureCreate {
        id: id.to_string(),
        name: name.to_string(),
        r#type,
        execution_order: None,
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
    assert_eq!(
        created.execution_order, 1,
        "first html feature auto-assigns 1"
    );
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
            execution_order: None,
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

    let updated = feature_service::update(
        pool,
        "demo-article",
        FeatureUpdate {
            name: None,
            execution_order: None,
        },
    )
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
            execution_order: None,
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
async fn list_paginates() {
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

#[tokio::test]
async fn create_auto_assigns_next_order_per_type() {
    let db = common::setup().await;
    let pool = &db.state.pool;

    let first = feature_service::create(pool, create_input("html-a", "A"))
        .await
        .expect("first html");
    let second = feature_service::create(pool, create_input("html-b", "B"))
        .await
        .expect("second html");
    let third = feature_service::create(pool, create_input("html-c", "C"))
        .await
        .expect("third html");

    assert_eq!(first.execution_order, 1);
    assert_eq!(second.execution_order, 2);
    assert_eq!(third.execution_order, 3);
}

#[tokio::test]
async fn html_and_json_share_the_number_space_independently() {
    let db = common::setup().await;
    let pool = &db.state.pool;

    let html = feature_service::create(pool, create_input_typed("html-1", "H", FeatureType::Html))
        .await
        .expect("html");
    let json = feature_service::create(pool, create_input_typed("json-1", "J", FeatureType::Json))
        .await
        .expect("json");

    // Both can be order 1 because UNIQUE is per type.
    assert_eq!(html.execution_order, 1);
    assert_eq!(json.execution_order, 1);
}

#[tokio::test]
async fn create_duplicate_order_within_type_conflicts() {
    let db = common::setup().await;
    let pool = &db.state.pool;

    let mut pinned = create_input("first", "First");
    pinned.execution_order = Some(5);
    feature_service::create(pool, pinned)
        .await
        .expect("first at order 5");

    let mut collide = create_input("second", "Second");
    collide.execution_order = Some(5);
    let err = feature_service::create(pool, collide)
        .await
        .expect_err("expected execution-order conflict");
    assert!(
        matches!(err, AppError::ExecutionOrderConflict(_)),
        "got {err:?}"
    );
}

#[tokio::test]
async fn reorder_via_patch_preserves_gaps() {
    let db = common::setup().await;
    let pool = &db.state.pool;

    let a = feature_service::create(pool, create_input("html-a", "A"))
        .await
        .expect("a"); // order 1
    let b = feature_service::create(pool, create_input("html-b", "B"))
        .await
        .expect("b"); // order 2
    assert_eq!(a.execution_order, 1);
    assert_eq!(b.execution_order, 2);

    // Move A to order 10 — B must NOT be renumbered (gaps are preserved).
    let moved = feature_service::update(
        pool,
        "html-a",
        FeatureUpdate {
            name: None,
            execution_order: Some(10),
        },
    )
    .await
    .expect("reorder a");
    assert_eq!(moved.execution_order, 10);

    let b_after = feature_service::get(pool, "html-b").await.expect("get b");
    assert_eq!(b_after.execution_order, 2, "B's order is untouched");
}

#[tokio::test]
async fn reorder_onto_taken_order_conflicts() {
    let db = common::setup().await;
    let pool = &db.state.pool;

    feature_service::create(pool, create_input("html-a", "A"))
        .await
        .expect("a"); // order 1
    feature_service::create(pool, create_input("html-b", "B"))
        .await
        .expect("b"); // order 2

    let err = feature_service::update(
        pool,
        "html-b",
        FeatureUpdate {
            name: None,
            execution_order: Some(1),
        },
    )
    .await
    .expect_err("expected execution-order conflict");
    assert!(
        matches!(err, AppError::ExecutionOrderConflict(_)),
        "got {err:?}"
    );
}

#[tokio::test]
async fn list_ordered_by_type_then_execution_order() {
    let db = common::setup().await;
    let pool = &db.state.pool;

    // Insert out of order across both types.
    feature_service::create(pool, create_input_typed("html-2", "H2", FeatureType::Html))
        .await
        .expect("html-2"); // order 1
    feature_service::create(pool, create_input_typed("html-1", "H1", FeatureType::Html))
        .await
        .expect("html-1"); // order 2
    feature_service::create(pool, create_input_typed("json-1", "J1", FeatureType::Json))
        .await
        .expect("json-1"); // order 1

    let page = feature_service::list(
        pool,
        &PageParams {
            page: Some(1),
            page_size: Some(20),
        },
    )
    .await
    .expect("list");

    let ordered: Vec<(String, i32)> = page
        .items
        .iter()
        .map(|f| (format!("{:?}", f.r#type), f.execution_order))
        .collect();

    // html (asc by execution_order) before json; within a type, lowest first.
    assert_eq!(
        ordered,
        vec![
            ("Html".to_string(), 1),
            ("Html".to_string(), 2),
            ("Json".to_string(), 1),
        ]
    );
}
