//! Integration tests for `version_service` against a real Postgres
//! (testcontainers). Cover the status lifecycle, edit locks, delete guards, and
//! the active-version read.

mod common;

use std::collections::HashSet;

use rre_backend::{
    error::AppError,
    models::enums::{Placement, VersionStatus},
    schemas::{
        component::{ComponentConfig, ComponentCreate, HtmlPlacementMode},
        outcome::OutcomeCreate,
        rule_graph::{CanvasGraph, Node, Position, RuleGraph},
        version::{PublishEnvironment, VersionCreate, VersionUpdate},
    },
    services::{outcome_service, version_service},
    state::AppState,
};
use sqlx::PgPool;
use uuid::Uuid;

const FID: &str = "dn-article";

/// Insert a feature row directly (the features API is owned by another module).
async fn seed_feature(pool: &PgPool, id: &str) {
    sqlx::query("INSERT INTO rre.features (id, name, type) VALUES ($1, $2, 'html')")
        .bind(id)
        .bind(format!("Feature {id}"))
        .execute(pool)
        .await
        .expect("seed feature");
}

async fn setup_with_feature() -> (common::TestDb, AppState) {
    let db = common::setup().await;
    seed_feature(&db.state.pool, FID).await;
    let state = db.state.clone();
    (db, state)
}

/// Create a draft version (no description) and return its number.
async fn make_version(state: &AppState) -> i32 {
    let body = VersionCreate {
        description: None,
        ..Default::default()
    };
    version_service::create_version(&state.pool, FID, body, &state.node_manifest)
        .await
        .expect("create version")
        .version_number
}

async fn live_pointers(pool: &PgPool) -> (Option<Uuid>, Option<Uuid>) {
    sqlx::query_as("SELECT staging_version_id, live_version_id FROM rre.features WHERE id = $1")
        .bind(FID)
        .fetch_one(pool)
        .await
        .unwrap()
}

#[tokio::test]
async fn create_seeds_draft_with_builtin_outcome() {
    let (_db, state) = setup_with_feature().await;
    let body = VersionCreate {
        description: Some("first".to_string()),
        ..Default::default()
    };

    let v = version_service::create_version(&state.pool, FID, body, &state.node_manifest)
        .await
        .expect("create version");

    assert_eq!(v.version_number, 1);
    assert_eq!(v.status, VersionStatus::Draft);
    assert_eq!(v.description.as_deref(), Some("first"));

    let count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM rre.outcomes WHERE version_id = $1 AND is_builtin = true",
    )
    .bind(v.id)
    .fetch_one(&state.pool)
    .await
    .unwrap();
    assert_eq!(count, 1);
}

#[tokio::test]
async fn create_increments_version_number() {
    let (_db, state) = setup_with_feature().await;
    let v1 = make_version(&state).await;
    let v2 = make_version(&state).await;
    assert_eq!(v1, 1);
    assert_eq!(v2, 2);
}

#[tokio::test]
async fn create_first_version_seeds_exactly_one_builtin() {
    let (_db, state) = setup_with_feature().await;
    let pool = &state.pool;

    let v1 = version_service::create_version(
        pool,
        FID,
        VersionCreate {
            description: None,
            ..Default::default()
        },
        &state.node_manifest,
    )
    .await
    .expect("create first version");

    let outcomes = outcome_service::list(pool, v1.id).await.unwrap();
    assert_eq!(outcomes.len(), 1, "first version has a single outcome");
    assert!(outcomes[0].is_builtin);
    assert_eq!(
        outcomes[0].title,
        outcome_service::BUILTIN_SHOW_CONTENT_TITLE
    );
}

#[tokio::test]
async fn create_second_version_carries_forward_outcomes_and_components() {
    let (_db, state) = setup_with_feature().await;
    let pool = &state.pool;

    // v1 starts with the builtin; add a second (non-builtin) outcome + component.
    let v1 = version_service::create_version(
        pool,
        FID,
        VersionCreate {
            description: None,
            ..Default::default()
        },
        &state.node_manifest,
    )
    .await
    .expect("create v1");

    let paywall = outcome_service::create(
        pool,
        v1.id,
        OutcomeCreate {
            title: "Paywall".to_string(),
            description: Some("hard paywall".to_string()),
        },
    )
    .await
    .unwrap();

    outcome_service::add_component(
        pool,
        paywall.id,
        ComponentCreate {
            slug: "promo".to_string(),
            r#type: "html_injection".to_string(),
            config: ComponentConfig::HtmlInjection {
                target_selector: ".article".to_string(),
                placement_mode: HtmlPlacementMode::Append,
                html_body: "<p>subscribe</p>".to_string(),
                theme: None,
            },
            placement: Placement::Inline,
            order_index: None,
        },
    )
    .await
    .unwrap();

    // v2 carries forward all of v1's outcomes + components.
    let v2 = version_service::create_version(
        pool,
        FID,
        VersionCreate {
            description: None,
            ..Default::default()
        },
        &state.node_manifest,
    )
    .await
    .expect("create v2");
    assert_eq!(v2.version_number, 2);
    assert_eq!(v2.status, VersionStatus::Draft);
    assert_ne!(v2.id, v1.id);

    let v1_outcomes = outcome_service::list(pool, v1.id).await.unwrap();
    let v2_outcomes = outcome_service::list(pool, v2.id).await.unwrap();

    // Same count + titles + is_builtin + ordering, but fresh outcome ids.
    assert_eq!(v2_outcomes.len(), v1_outcomes.len());
    assert_eq!(v2_outcomes.len(), 2);
    for (src, dst) in v1_outcomes.iter().zip(v2_outcomes.iter()) {
        assert_eq!(dst.title, src.title);
        assert_eq!(dst.is_builtin, src.is_builtin);
        assert_eq!(dst.order_index, src.order_index);
        assert_ne!(dst.id, src.id, "carried-forward outcome gets a new id");
    }

    // Exactly one builtin survives (no double-seed alongside carry-forward).
    let builtin_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM rre.outcomes WHERE version_id = $1 AND is_builtin = true",
    )
    .bind(v2.id)
    .fetch_one(pool)
    .await
    .unwrap();
    assert_eq!(
        builtin_count, 1,
        "builtin is carried forward, not duplicated"
    );

    // The Paywall outcome's component is deep-copied with a fresh id.
    let carried_paywall = v2_outcomes
        .iter()
        .find(|o| o.title == "Paywall")
        .expect("paywall carried forward");
    assert_eq!(carried_paywall.components.len(), 1);
    let carried_component = &carried_paywall.components[0];
    assert_eq!(carried_component.slug, "promo");
    assert_eq!(carried_component.config["type"], "html_injection");
    assert_eq!(carried_component.placement, Placement::Inline);

    // Carried component belongs to the new outcome, not the old one.
    let v1_paywall = v1_outcomes.iter().find(|o| o.title == "Paywall").unwrap();
    assert_ne!(carried_component.id, v1_paywall.components[0].id);
}

#[tokio::test]
async fn create_next_version_carries_forward_from_live_source() {
    let (_db, state) = setup_with_feature().await;
    let pool = &state.pool;

    // v1 (will be published live) gets a custom outcome.
    let v1 = version_service::create_version(
        pool,
        FID,
        VersionCreate {
            description: None,
            ..Default::default()
        },
        &state.node_manifest,
    )
    .await
    .expect("create v1");
    outcome_service::create(
        pool,
        v1.id,
        OutcomeCreate {
            title: "From Live".to_string(),
            description: None,
        },
    )
    .await
    .unwrap();
    version_service::publish(pool, FID, v1.version_number, PublishEnvironment::Live)
        .await
        .unwrap();

    // v2 is created while v1 is live and inherits v1's outcomes.
    let v2 = version_service::create_version(
        pool,
        FID,
        VersionCreate {
            description: None,
            ..Default::default()
        },
        &state.node_manifest,
    )
    .await
    .expect("create v2");

    let v2_outcomes = outcome_service::list(pool, v2.id).await.unwrap();
    assert_eq!(v2_outcomes.len(), 2);
    assert!(v2_outcomes.iter().any(|o| o.title == "From Live"));
}

#[tokio::test]
async fn create_on_missing_feature_is_404() {
    let db = common::setup().await;
    let body = VersionCreate {
        description: None,
        ..Default::default()
    };
    let err =
        version_service::create_version(&db.state.pool, "nope", body, &db.state.node_manifest)
            .await
            .unwrap_err();
    assert!(matches!(err, AppError::FeatureNotFound(_)));
}

#[tokio::test]
async fn publish_live_promotes_and_demotes() {
    let (_db, state) = setup_with_feature().await;
    let pool = &state.pool;
    let v1 = make_version(&state).await;
    let v2 = make_version(&state).await;

    let published = version_service::publish(pool, FID, v1, PublishEnvironment::Live)
        .await
        .unwrap();
    assert_eq!(published.status, VersionStatus::Live);

    let (_, live_id) = live_pointers(pool).await;
    assert_eq!(live_id, Some(published.id));

    // Publish v2 live → v1 demoted to PREV.
    version_service::publish(pool, FID, v2, PublishEnvironment::Live)
        .await
        .unwrap();
    let v1_after = version_service::get(pool, FID, v1).await.unwrap();
    assert_eq!(v1_after.status, VersionStatus::Prev);
}

#[tokio::test]
async fn publish_live_when_already_live_is_conflict() {
    let (_db, state) = setup_with_feature().await;
    let pool = &state.pool;
    let v1 = make_version(&state).await;
    version_service::publish(pool, FID, v1, PublishEnvironment::Live)
        .await
        .unwrap();
    let err = version_service::publish(pool, FID, v1, PublishEnvironment::Live)
        .await
        .unwrap_err();
    assert!(matches!(err, AppError::InvalidStatusTransition(_)));
}

#[tokio::test]
async fn unpublish_live_clears_pointer_and_sets_prev() {
    let (_db, state) = setup_with_feature().await;
    let pool = &state.pool;
    let v1 = make_version(&state).await;
    version_service::publish(pool, FID, v1, PublishEnvironment::Live)
        .await
        .unwrap();

    let un = version_service::unpublish(pool, FID, v1, PublishEnvironment::Live)
        .await
        .unwrap();
    assert_eq!(un.status, VersionStatus::Prev);

    let (_, live_id) = live_pointers(pool).await;
    assert_eq!(live_id, None);
}

#[tokio::test]
async fn unpublish_non_live_is_conflict() {
    let (_db, state) = setup_with_feature().await;
    let pool = &state.pool;
    let v1 = make_version(&state).await;
    let err = version_service::unpublish(pool, FID, v1, PublishEnvironment::Live)
        .await
        .unwrap_err();
    assert!(matches!(err, AppError::InvalidStatusTransition(_)));
}

#[tokio::test]
async fn delete_draft_succeeds_but_live_is_blocked() {
    let (_db, state) = setup_with_feature().await;
    let pool = &state.pool;
    let v1 = make_version(&state).await;
    let v2 = make_version(&state).await;

    // Draft delete works.
    version_service::delete(pool, FID, v1).await.unwrap();

    // Live delete blocked.
    version_service::publish(pool, FID, v2, PublishEnvironment::Live)
        .await
        .unwrap();
    let err = version_service::delete(pool, FID, v2).await.unwrap_err();
    assert!(matches!(err, AppError::InvalidStatusTransition(_)));
}

#[tokio::test]
async fn update_description_allowed_on_any_status() {
    let (_db, state) = setup_with_feature().await;
    let pool = &state.pool;
    let v1 = make_version(&state).await;
    version_service::publish(pool, FID, v1, PublishEnvironment::Live)
        .await
        .unwrap();

    let body = VersionUpdate {
        description: Some("edited".to_string()),
        rule_graph: None,
        applicability: None,
    };
    let updated = version_service::update(pool, FID, v1, body, &state.node_manifest)
        .await
        .unwrap();
    assert_eq!(updated.description.as_deref(), Some("edited"));
    assert_eq!(updated.status, VersionStatus::Live);
}

#[tokio::test]
async fn update_rule_graph_on_non_draft_is_edit_locked() {
    let (_db, state) = setup_with_feature().await;
    let pool = &state.pool;
    let v1 = make_version(&state).await;
    version_service::publish(pool, FID, v1, PublishEnvironment::Live)
        .await
        .unwrap();

    let body = VersionUpdate {
        description: None,
        rule_graph: Some(Default::default()),
        applicability: None,
    };
    let err = version_service::update(pool, FID, v1, body, &state.node_manifest)
        .await
        .unwrap_err();
    assert!(matches!(err, AppError::VersionEditLocked(_)));
}

#[tokio::test]
async fn get_unknown_version_is_404() {
    let (_db, state) = setup_with_feature().await;
    let err = version_service::get(&state.pool, FID, 999)
        .await
        .unwrap_err();
    assert!(matches!(err, AppError::VersionNotFound(_)));
}

#[tokio::test]
async fn active_version_no_live_is_404() {
    let (_db, state) = setup_with_feature().await;
    make_version(&state).await;
    let err = version_service::active_version(&state.pool, FID, PublishEnvironment::Live)
        .await
        .unwrap_err();
    assert!(matches!(err, AppError::NoLiveVersion(_)));
}

/// "Save as New Version": creating v2 with a rule_graph whose outcome nodes
/// reference the SOURCE version's outcome ids must succeed (NOT 422) — the
/// service remaps those references onto the carried-forward outcome ids before
/// validating, and the returned graph carries the NEW ids.
#[tokio::test]
async fn create_version_with_rule_graph_remaps_outcome_refs() {
    let (_db, state) = setup_with_feature().await;
    let pool = &state.pool;

    // v1: builtin + a Paywall outcome. These ids are what the editor's graph
    // (built against v1) references.
    let v1 = version_service::create_version(
        pool,
        FID,
        VersionCreate {
            description: None,
            ..Default::default()
        },
        &state.node_manifest,
    )
    .await
    .expect("create v1");
    outcome_service::create(
        pool,
        v1.id,
        OutcomeCreate {
            title: "Paywall".to_string(),
            description: None,
        },
    )
    .await
    .unwrap();

    let v1_outcomes = outcome_service::list(pool, v1.id).await.unwrap();
    let v1_outcome_ids: Vec<Uuid> = v1_outcomes.iter().map(|o| o.id).collect();
    assert_eq!(v1_outcomes.len(), 2);

    // A graph whose two outcome nodes reference v1's outcome ids (one per node).
    let pos = Position { x: 0.0, y: 0.0 };
    let graph = RuleGraph {
        anonymous: CanvasGraph {
            nodes: vec![
                Node::Outcome {
                    id: "o-builtin".to_string(),
                    outcome_id: v1_outcome_ids[0],
                    position: pos,
                },
                Node::Outcome {
                    id: "o-paywall".to_string(),
                    outcome_id: v1_outcome_ids[1],
                    position: pos,
                },
            ],
            edges: vec![],
            root_node_id: None,
        },
        ..Default::default()
    };

    // (a) Create v2 with the source-referencing graph — must NOT be a 422.
    let v2 = version_service::create_version(
        pool,
        FID,
        VersionCreate {
            description: Some("save as new".to_string()),
            rule_graph: Some(graph),
            applicability: None,
        },
        &state.node_manifest,
    )
    .await
    .expect("create v2 with rule_graph must not 422");
    assert_eq!(v2.version_number, 2);
    assert_eq!(v2.status, VersionStatus::Draft);

    // (c) Carried-forward outcome count + titles are preserved.
    let v2_outcomes = outcome_service::list(pool, v2.id).await.unwrap();
    assert_eq!(v2_outcomes.len(), v1_outcomes.len());
    for (src, dst) in v1_outcomes.iter().zip(v2_outcomes.iter()) {
        assert_eq!(dst.title, src.title);
        assert_eq!(dst.is_builtin, src.is_builtin);
        assert_ne!(dst.id, src.id, "carried-forward outcome gets a new id");
    }

    // (b) The returned graph's outcome nodes reference the NEW version's outcome
    // ids — ids that exist in v2's outcomes and are NONE of the source ids.
    let v2_outcome_ids: HashSet<Uuid> = v2_outcomes.iter().map(|o| o.id).collect();
    let v1_outcome_id_set: HashSet<Uuid> = v1_outcome_ids.iter().copied().collect();
    let referenced: Vec<Uuid> = v2
        .rule_graph
        .anonymous
        .nodes
        .iter()
        .filter_map(|n| match n {
            Node::Outcome { outcome_id, .. } => Some(*outcome_id),
            _ => None,
        })
        .collect();
    assert_eq!(referenced.len(), 2);
    for oid in referenced {
        assert!(
            v2_outcome_ids.contains(&oid),
            "graph references an id that exists in v2's outcomes"
        );
        assert!(
            !v1_outcome_id_set.contains(&oid),
            "graph no longer references any source (v1) outcome id"
        );
    }
}
