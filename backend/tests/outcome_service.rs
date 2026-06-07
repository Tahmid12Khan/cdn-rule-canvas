//! Integration tests for `outcome_service` against a real Postgres
//! (testcontainers). Covers seeding the builtin outcome, CRUD, edit-lock guards,
//! builtin-delete protection, deep clone, component config validation, and
//! reorder.

mod common;

use rre_backend::models::enums::{Placement, VersionStatus};
use rre_backend::schemas::component::{
    ComponentConfig, ComponentCreate, ComponentUpdate, HtmlPlacementMode,
};
use rre_backend::schemas::outcome::{OutcomeCreate, OutcomeUpdate, ReorderItem};
use rre_backend::services::outcome_service;
use sqlx::PgPool;
use uuid::Uuid;

/// Insert a feature + a version with the given status; return the version id.
async fn seed_version(pool: &PgPool, status: VersionStatus) -> Uuid {
    let feature_id = format!("feat-{}", &Uuid::new_v4().to_string()[..8]);
    sqlx::query(
        "INSERT INTO rre.features (id, name, type, execution_order) \
         VALUES ($1, $2, 'html', \
                 (SELECT COALESCE(MAX(execution_order), 0) + 1 FROM rre.features WHERE type = 'html'))",
    )
        .bind(&feature_id)
        .bind("Test Feature")
        .execute(pool)
        .await
        .expect("insert feature");

    let version_id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO rre.versions (id, feature_id, version_number, status) \
         VALUES ($1, $2, 1, $3)",
    )
    .bind(version_id)
    .bind(&feature_id)
    .bind(status)
    .execute(pool)
    .await
    .expect("insert version");

    version_id
}

fn html_injection() -> ComponentConfig {
    ComponentConfig::HtmlInjection {
        target_selector: ".article".to_string(),
        placement_mode: HtmlPlacementMode::Append,
        html_body: "<p>promo</p>".to_string(),
        theme: None,
    }
}

#[tokio::test]
async fn seed_builtin_creates_protected_outcome() {
    let db = common::setup().await;
    let pool = &db.state.pool;
    let vid = seed_version(pool, VersionStatus::Draft).await;

    outcome_service::seed_builtin(pool, vid).await.unwrap();

    let outcomes = outcome_service::list(pool, vid).await.unwrap();
    assert_eq!(outcomes.len(), 1);
    assert!(outcomes[0].is_builtin);
    assert_eq!(
        outcomes[0].title,
        outcome_service::BUILTIN_SHOW_CONTENT_TITLE
    );
}

#[tokio::test]
async fn create_and_get_outcome_roundtrip() {
    let db = common::setup().await;
    let pool = &db.state.pool;
    let vid = seed_version(pool, VersionStatus::Draft).await;

    let created = outcome_service::create(
        pool,
        vid,
        OutcomeCreate {
            title: "Paywall".to_string(),
            description: Some("Hard paywall".to_string()),
        },
    )
    .await
    .unwrap();

    assert_eq!(created.title, "Paywall");
    assert!(!created.is_builtin);

    let fetched = outcome_service::get_with_components(pool, created.id)
        .await
        .unwrap();
    assert_eq!(fetched.id, created.id);
    assert_eq!(fetched.description.as_deref(), Some("Hard paywall"));
}

#[tokio::test]
async fn create_outcome_on_live_version_is_locked() {
    let db = common::setup().await;
    let pool = &db.state.pool;
    let vid = seed_version(pool, VersionStatus::Live).await;

    let err = outcome_service::create(
        pool,
        vid,
        OutcomeCreate {
            title: "Nope".to_string(),
            description: None,
        },
    )
    .await
    .unwrap_err();
    assert_eq!(err.code(), "VERSION_EDIT_LOCKED");
}

#[tokio::test]
async fn create_outcome_on_missing_version_is_404() {
    let db = common::setup().await;
    let pool = &db.state.pool;

    let err = outcome_service::create(
        pool,
        Uuid::new_v4(),
        OutcomeCreate {
            title: "Ghost".to_string(),
            description: None,
        },
    )
    .await
    .unwrap_err();
    assert_eq!(err.code(), "VERSION_NOT_FOUND");
}

#[tokio::test]
async fn update_outcome_changes_title_keeps_description() {
    let db = common::setup().await;
    let pool = &db.state.pool;
    let vid = seed_version(pool, VersionStatus::Draft).await;

    let created = outcome_service::create(
        pool,
        vid,
        OutcomeCreate {
            title: "Old".to_string(),
            description: Some("keep me".to_string()),
        },
    )
    .await
    .unwrap();

    let updated = outcome_service::update(
        pool,
        created.id,
        OutcomeUpdate {
            title: Some("New".to_string()),
            description: None,
            order_index: None,
        },
    )
    .await
    .unwrap();

    assert_eq!(updated.title, "New");
    assert_eq!(updated.description.as_deref(), Some("keep me"));
}

#[tokio::test]
async fn delete_builtin_outcome_is_protected() {
    let db = common::setup().await;
    let pool = &db.state.pool;
    let vid = seed_version(pool, VersionStatus::Draft).await;
    outcome_service::seed_builtin(pool, vid).await.unwrap();

    let builtin = outcome_service::list(pool, vid).await.unwrap();
    let builtin_id = builtin[0].id;

    let err = outcome_service::delete(pool, builtin_id).await.unwrap_err();
    assert_eq!(err.code(), "BUILTIN_OUTCOME_PROTECTED");
}

#[tokio::test]
async fn delete_non_builtin_outcome_succeeds() {
    let db = common::setup().await;
    let pool = &db.state.pool;
    let vid = seed_version(pool, VersionStatus::Draft).await;

    let created = outcome_service::create(
        pool,
        vid,
        OutcomeCreate {
            title: "Disposable".to_string(),
            description: None,
        },
    )
    .await
    .unwrap();

    outcome_service::delete(pool, created.id).await.unwrap();

    let err = outcome_service::get_with_components(pool, created.id)
        .await
        .unwrap_err();
    assert_eq!(err.code(), "OUTCOME_NOT_FOUND");
}

#[tokio::test]
async fn delete_outcome_on_non_draft_is_locked() {
    let db = common::setup().await;
    let pool = &db.state.pool;
    let vid = seed_version(pool, VersionStatus::Draft).await;
    let created = outcome_service::create(
        pool,
        vid,
        OutcomeCreate {
            title: "x".to_string(),
            description: None,
        },
    )
    .await
    .unwrap();

    // Flip the version to staging out-of-band.
    sqlx::query("UPDATE rre.versions SET status = 'staging' WHERE id = $1")
        .bind(vid)
        .execute(pool)
        .await
        .unwrap();

    let err = outcome_service::delete(pool, created.id).await.unwrap_err();
    assert_eq!(err.code(), "VERSION_EDIT_LOCKED");
}

#[tokio::test]
async fn clone_outcome_deep_copies_components_with_new_ids() {
    let db = common::setup().await;
    let pool = &db.state.pool;
    let vid = seed_version(pool, VersionStatus::Draft).await;

    let outcome = outcome_service::create(
        pool,
        vid,
        OutcomeCreate {
            title: "Source".to_string(),
            description: Some("d".to_string()),
        },
    )
    .await
    .unwrap();

    let comp = outcome_service::add_component(
        pool,
        outcome.id,
        ComponentCreate {
            slug: "promo".to_string(),
            r#type: "html_injection".to_string(),
            config: html_injection(),
            placement: Placement::Inline,
            order_index: None,
        },
    )
    .await
    .unwrap();

    let cloned = outcome_service::clone_outcome(pool, outcome.id)
        .await
        .unwrap();

    assert_ne!(cloned.id, outcome.id);
    assert_eq!(cloned.title, "Source (copy)");
    assert!(!cloned.is_builtin);
    assert_eq!(cloned.components.len(), 1);
    assert_ne!(cloned.components[0].id, comp.id);
    assert_eq!(cloned.components[0].slug, "promo");
}

#[tokio::test]
async fn add_component_rejects_type_mismatch() {
    let db = common::setup().await;
    let pool = &db.state.pool;
    let vid = seed_version(pool, VersionStatus::Draft).await;
    let outcome = outcome_service::create(
        pool,
        vid,
        OutcomeCreate {
            title: "O".to_string(),
            description: None,
        },
    )
    .await
    .unwrap();

    let err = outcome_service::add_component(
        pool,
        outcome.id,
        ComponentCreate {
            slug: "bad".to_string(),
            // type says truncation but config is html_injection
            r#type: "content_truncation".to_string(),
            config: html_injection(),
            placement: Placement::Inline,
            order_index: None,
        },
    )
    .await
    .unwrap_err();
    assert_eq!(err.code(), "VALIDATION_ERROR");
}

#[tokio::test]
async fn add_component_persists_config_with_discriminator() {
    let db = common::setup().await;
    let pool = &db.state.pool;
    let vid = seed_version(pool, VersionStatus::Draft).await;
    let outcome = outcome_service::create(
        pool,
        vid,
        OutcomeCreate {
            title: "O".to_string(),
            description: None,
        },
    )
    .await
    .unwrap();

    let comp = outcome_service::add_component(
        pool,
        outcome.id,
        ComponentCreate {
            slug: "trunc".to_string(),
            r#type: "content_truncation".to_string(),
            config: ComponentConfig::ContentTruncation {
                target_selector: ".body".to_string(),
                word_count: 50,
                fade_out: true,
            },
            placement: Placement::Inline,
            order_index: None,
        },
    )
    .await
    .unwrap();

    assert_eq!(comp.config["type"], "content_truncation");
    assert_eq!(comp.config["word_count"], 50);
}

#[tokio::test]
async fn update_component_revalidates_config() {
    let db = common::setup().await;
    let pool = &db.state.pool;
    let vid = seed_version(pool, VersionStatus::Draft).await;
    let outcome = outcome_service::create(
        pool,
        vid,
        OutcomeCreate {
            title: "O".to_string(),
            description: None,
        },
    )
    .await
    .unwrap();
    let comp = outcome_service::add_component(
        pool,
        outcome.id,
        ComponentCreate {
            slug: "c".to_string(),
            r#type: "html_injection".to_string(),
            config: html_injection(),
            placement: Placement::Inline,
            order_index: None,
        },
    )
    .await
    .unwrap();

    // Swap placement only — config left untouched.
    let updated = outcome_service::update_component(
        pool,
        comp.id,
        ComponentUpdate {
            slug: None,
            r#type: None,
            config: None,
            placement: Some(Placement::Popup),
            order_index: None,
        },
    )
    .await
    .unwrap();
    assert_eq!(updated.placement, Placement::Popup);
}

#[tokio::test]
async fn delete_component_succeeds_and_then_404() {
    let db = common::setup().await;
    let pool = &db.state.pool;
    let vid = seed_version(pool, VersionStatus::Draft).await;
    let outcome = outcome_service::create(
        pool,
        vid,
        OutcomeCreate {
            title: "O".to_string(),
            description: None,
        },
    )
    .await
    .unwrap();
    let comp = outcome_service::add_component(
        pool,
        outcome.id,
        ComponentCreate {
            slug: "c".to_string(),
            r#type: "html_injection".to_string(),
            config: html_injection(),
            placement: Placement::Inline,
            order_index: None,
        },
    )
    .await
    .unwrap();

    outcome_service::delete_component(pool, comp.id)
        .await
        .unwrap();

    let err = outcome_service::delete_component(pool, comp.id)
        .await
        .unwrap_err();
    assert_eq!(err.code(), "COMPONENT_NOT_FOUND");
}

#[tokio::test]
async fn reorder_components_updates_order_and_rejects_foreign_id() {
    let db = common::setup().await;
    let pool = &db.state.pool;
    let vid = seed_version(pool, VersionStatus::Draft).await;
    let outcome = outcome_service::create(
        pool,
        vid,
        OutcomeCreate {
            title: "O".to_string(),
            description: None,
        },
    )
    .await
    .unwrap();

    let c1 = outcome_service::add_component(
        pool,
        outcome.id,
        ComponentCreate {
            slug: "a".to_string(),
            r#type: "html_injection".to_string(),
            config: html_injection(),
            placement: Placement::Inline,
            order_index: Some(0),
        },
    )
    .await
    .unwrap();
    let c2 = outcome_service::add_component(
        pool,
        outcome.id,
        ComponentCreate {
            slug: "b".to_string(),
            r#type: "html_injection".to_string(),
            config: html_injection(),
            placement: Placement::Inline,
            order_index: Some(1),
        },
    )
    .await
    .unwrap();

    // Swap order: c2 first, c1 second.
    let reordered = outcome_service::reorder(
        pool,
        outcome.id,
        vec![
            ReorderItem {
                id: c2.id,
                order_index: 0,
            },
            ReorderItem {
                id: c1.id,
                order_index: 1,
            },
        ],
    )
    .await
    .unwrap();
    assert_eq!(reordered[0].id, c2.id);
    assert_eq!(reordered[1].id, c1.id);

    // A foreign component id is rejected.
    let err = outcome_service::reorder(
        pool,
        outcome.id,
        vec![ReorderItem {
            id: Uuid::new_v4(),
            order_index: 0,
        }],
    )
    .await
    .unwrap_err();
    assert_eq!(err.code(), "VALIDATION_ERROR");
}

#[tokio::test]
async fn component_type_check_constraint_rejects_unknown() {
    // DB backstop (migration 0005): inserting an unknown type directly fails.
    let db = common::setup().await;
    let pool = &db.state.pool;
    let vid = seed_version(pool, VersionStatus::Draft).await;
    let outcome = outcome_service::create(
        pool,
        vid,
        OutcomeCreate {
            title: "O".to_string(),
            description: None,
        },
    )
    .await
    .unwrap();

    let res = sqlx::query(
        "INSERT INTO rre.components (id, outcome_id, slug, type, config, placement) \
         VALUES ($1, $2, 'x', 'unknown_kind', '{}'::jsonb, 'inline')",
    )
    .bind(Uuid::new_v4())
    .bind(outcome.id)
    .execute(pool)
    .await;
    assert!(
        res.is_err(),
        "unknown component type should violate CHECK constraint"
    );
}
