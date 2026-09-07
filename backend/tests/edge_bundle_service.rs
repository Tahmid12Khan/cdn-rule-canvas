//! Integration tests for `edge_bundle_service` against a real Postgres
//! (testcontainers).
//!
//! The bundle is what a Fastly Compute guest evaluates with NO further I/O, so
//! these tests pin the two properties the edge cannot recover on its own:
//! feature ORDER, and every component / saved-outcome reference resolved INLINE.

mod common;

use rre_backend::{
    error::AppError,
    models::enums::Placement,
    schemas::{
        component::{ComponentConfig, ComponentCreate, HtmlPlacementMode},
        component_template::ComponentTemplateCreate,
        rule_graph::{Branch, CanvasGraph, Edge, Node, Position, ProcessorConfig, RuleGraph},
        saved_outcome::SavedOutcomeCreate,
        site::SiteCreate,
        version::{PublishEnvironment, VersionCreate},
    },
    services::{
        component_template_service, edge_bundle_service, outcome_service, saved_outcome_service,
        site_service, version_service,
    },
    state::AppState,
};
use serde_json::json;
use sqlx::PgPool;
use uuid::Uuid;

const SITE: &str = "intrafish-com";
const HOST: &str = "test.intrafish.com";

fn pos() -> Position {
    Position { x: 0.0, y: 0.0 }
}

async fn seed_site(state: &AppState, slug: &str, source_host: &str) {
    site_service::create(
        &state.pool,
        SiteCreate {
            slug: slug.to_string(),
            name: format!("Site {slug}"),
            source_protocol: "https".to_string(),
            source_host: source_host.to_string(),
            source_port: 443,
            dest_protocol: "https".to_string(),
            dest_host: "origin.example.com".to_string(),
            dest_port: 443,
            headers: None,
        },
    )
    .await
    .expect("seed site");
}

async fn seed_feature(pool: &PgPool, id: &str, kind: &str, execution_order: i32) {
    sqlx::query(
        r#"INSERT INTO rre.features (id, name, "type", execution_order)
           VALUES ($1, $2, $3::rre.feature_type, $4)"#,
    )
    .bind(id)
    .bind(format!("Feature {id}"))
    .bind(kind)
    .bind(execution_order)
    .execute(pool)
    .await
    .expect("seed feature");
}

/// A minimal valid canvas: `start -> expression(action) -> end`.
fn canvas_with_action(action: ProcessorConfig) -> RuleGraph {
    RuleGraph {
        canvas: CanvasGraph {
            nodes: vec![
                Node::Start {
                    id: "s".into(),
                    position: pos(),
                },
                Node::Expression {
                    id: "e".into(),
                    action,
                    custom_label: None,
                    position: pos(),
                },
                Node::End {
                    id: "n".into(),
                    position: pos(),
                },
            ],
            edges: vec![
                Edge {
                    id: "e1".into(),
                    source_node_id: "s".into(),
                    target_node_id: "e".into(),
                    branch: Branch::Yes,
                },
                Edge {
                    id: "e2".into(),
                    source_node_id: "e".into(),
                    target_node_id: "n".into(),
                    branch: Branch::Yes,
                },
            ],
            root_node_id: Some("s".into()),
        },
    }
}

fn processor(kind: &str, fields: serde_json::Value) -> ProcessorConfig {
    ProcessorConfig {
        r#type: kind.to_string(),
        fields: fields.as_object().expect("object fields").clone(),
    }
}

/// Create v1 for `fid`, optionally set its canvas, and publish it LIVE.
async fn publish_version(state: &AppState, fid: &str, rule_graph: Option<RuleGraph>) {
    let v = version_service::create_version(
        &state.pool,
        fid,
        VersionCreate {
            description: None,
            ..Default::default()
        },
        &state.node_manifest,
    )
    .await
    .expect("create version");

    if let Some(graph) = rule_graph {
        version_service::update_rule_graph(
            &state.pool,
            fid,
            v.version_number,
            graph,
            &state.node_manifest,
        )
        .await
        .expect("set rule graph");
    }

    version_service::publish(&state.pool, fid, v.version_number, PublishEnvironment::Live)
        .await
        .expect("publish live");
}

/// Create a draft version for `fid` WITHOUT publishing it.
async fn draft_version(state: &AppState, fid: &str) {
    version_service::create_version(
        &state.pool,
        fid,
        VersionCreate {
            description: None,
            ..Default::default()
        },
        &state.node_manifest,
    )
    .await
    .expect("create version");
}

async fn seed_component_template(state: &AppState, slug: &str, html_body: &str) -> Uuid {
    component_template_service::create(
        &state.pool,
        ComponentTemplateCreate {
            slug: slug.to_string(),
            name: format!("Template {slug}"),
            description: None,
            html_body: Some(html_body.to_string()),
            variables: None,
        },
    )
    .await
    .expect("seed component template")
    .id
}

/// The bundle must list features in `(type, execution_order)` order and carry
/// each one's live version. Ordering is the contract the edge relies on: it runs
/// `features` in array order and never re-sorts.
#[tokio::test]
async fn build_orders_features_and_embeds_live_versions() {
    let db = common::setup().await;
    let state = db.state.clone();
    seed_site(&state, SITE, HOST).await;

    seed_feature(&state.pool, "b-html", "html", 2).await;
    seed_feature(&state.pool, "a-html", "html", 1).await;
    seed_feature(&state.pool, "c-json", "json", 1).await;
    for fid in ["b-html", "a-html", "c-json"] {
        publish_version(&state, fid, None).await;
    }

    let bundle = edge_bundle_service::build(&state.pool, SITE, PublishEnvironment::Live)
        .await
        .expect("build bundle");

    assert_eq!(bundle.schema_version, rre_core::edge::SCHEMA_VERSION);
    assert_eq!(bundle.site.slug, SITE);
    assert_eq!(bundle.site.source_host, HOST);
    assert_eq!(bundle.environment, "live");

    let ids: Vec<&str> = bundle.features.iter().map(|f| f.id.as_str()).collect();
    assert_eq!(ids, vec!["a-html", "b-html", "c-json"]);

    let kinds: Vec<&str> = bundle.features.iter().map(|f| f.r#type.as_str()).collect();
    assert_eq!(kinds, vec!["html", "html", "json"]);
    assert!(bundle.features.iter().all(|f| f.version_number == 1));
}

/// A feature with no published version for the environment is omitted entirely
/// rather than exported with an empty canvas.
#[tokio::test]
async fn build_omits_unpublished_features() {
    let db = common::setup().await;
    let state = db.state.clone();
    seed_site(&state, SITE, HOST).await;

    seed_feature(&state.pool, "draft-only", "json", 1).await;
    draft_version(&state, "draft-only").await;

    let bundle = edge_bundle_service::build(&state.pool, SITE, PublishEnvironment::Live)
        .await
        .expect("build bundle");

    assert!(
        bundle.features.is_empty(),
        "unpublished features must not be exported: {:?}",
        bundle.features.iter().map(|f| &f.id).collect::<Vec<_>>()
    );
}

/// A feature that has never had a version at all is skipped too, not a 404.
#[tokio::test]
async fn build_omits_features_with_no_versions() {
    let db = common::setup().await;
    let state = db.state.clone();
    seed_site(&state, SITE, HOST).await;

    seed_feature(&state.pool, "never-versioned", "html", 1).await;

    let bundle = edge_bundle_service::build(&state.pool, SITE, PublishEnvironment::Live)
        .await
        .expect("build bundle");

    assert!(bundle.features.is_empty());
}

/// Every `apply_component` reference the canvas can reach must be resolved into
/// `resolved_components`, because the edge cannot fetch one.
#[tokio::test]
async fn build_resolves_component_refs_inline() {
    let db = common::setup().await;
    let state = db.state.clone();
    seed_site(&state, SITE, HOST).await;

    let cid = seed_component_template(&state, "paywall-box", "<div>{{title}}</div>").await;
    seed_feature(&state.pool, "paywall", "html", 1).await;
    publish_version(
        &state,
        "paywall",
        Some(canvas_with_action(processor(
            "apply_component",
            json!({
                "component_id": cid.to_string(),
                "version": "default",
                "target_selector": "main",
                "placement_mode": "append",
            }),
        ))),
    )
    .await;

    let bundle = edge_bundle_service::build(&state.pool, SITE, PublishEnvironment::Live)
        .await
        .expect("build bundle");

    let f = &bundle.features[0];
    let key = rre_core::edge::component_key(cid, &rre_core::bundle::VersionSelector::Default);
    assert_eq!(key, format!("{cid}|default"));
    assert!(
        f.resolved_components.contains_key(&key),
        "keys: {:?}",
        f.resolved_components.keys().collect::<Vec<_>>()
    );
    assert!(f.resolved_components[&key].html_body.contains("{{title}}"));
    assert_eq!(f.resolved_components[&key].version_number, 1);
}

/// A `component_ref` living inside an OUTCOME's components is resolved too. The
/// walk is static: it does not matter that no route reaches the outcome.
#[tokio::test]
async fn build_resolves_component_refs_inside_outcomes() {
    let db = common::setup().await;
    let state = db.state.clone();
    seed_site(&state, SITE, HOST).await;

    let cid = seed_component_template(&state, "outcome-box", "<aside>{{cta}}</aside>").await;
    seed_feature(&state.pool, "outcome-feature", "html", 1).await;

    let v = version_service::create_version(
        &state.pool,
        "outcome-feature",
        VersionCreate {
            description: None,
            ..Default::default()
        },
        &state.node_manifest,
    )
    .await
    .expect("create version");

    let outcome = outcome_service::create(
        &state.pool,
        v.id,
        rre_backend::schemas::outcome::OutcomeCreate {
            title: "Paywall".to_string(),
            description: None,
        },
    )
    .await
    .expect("create outcome");

    outcome_service::add_component(
        &state.pool,
        outcome.id,
        ComponentCreate {
            slug: "boxed".to_string(),
            r#type: "component_ref".to_string(),
            config: ComponentConfig::ComponentRef {
                component_id: cid,
                version: json!(1),
                variables: serde_json::Map::new(),
                target_selector: "main".to_string(),
                placement_mode: HtmlPlacementMode::Append,
            },
            placement: Placement::Inline,
            order_index: None,
        },
    )
    .await
    .expect("add component");

    version_service::publish(
        &state.pool,
        "outcome-feature",
        v.version_number,
        PublishEnvironment::Live,
    )
    .await
    .expect("publish");

    let bundle = edge_bundle_service::build(&state.pool, SITE, PublishEnvironment::Live)
        .await
        .expect("build bundle");

    let f = &bundle.features[0];
    let key = rre_core::edge::component_key(cid, &rre_core::bundle::VersionSelector::Version(1));
    assert!(
        f.resolved_components.contains_key(&key),
        "pinned outcome component must resolve; keys: {:?}",
        f.resolved_components.keys().collect::<Vec<_>>()
    );
    assert!(f.resolved_components[&key].html_body.contains("{{cta}}"));
}

/// An `apply_saved_outcome` action must carry its ALREADY-RENDERED HTML in
/// `saved_outcomes`, keyed by the saved outcome's id.
#[tokio::test]
async fn build_resolves_saved_outcome_refs_inline() {
    let db = common::setup().await;
    let state = db.state.clone();
    seed_site(&state, SITE, HOST).await;

    let cid = seed_component_template(&state, "regwall-box", "<p>{{headline}}</p>").await;
    let saved = saved_outcome_service::create(
        &state.pool,
        SavedOutcomeCreate {
            slug: "regwall".to_string(),
            name: "Regwall".to_string(),
            component_id: cid,
            version_number: None,
            variables: [("headline".to_string(), "Subscribe".to_string())]
                .into_iter()
                .collect(),
        },
    )
    .await
    .expect("create saved outcome");

    seed_feature(&state.pool, "regwall-feature", "html", 1).await;
    publish_version(
        &state,
        "regwall-feature",
        Some(canvas_with_action(processor(
            "apply_saved_outcome",
            json!({
                "saved_outcome_id": saved.id.to_string(),
                "target_selector": "main",
                "placement_mode": "append",
            }),
        ))),
    )
    .await;

    let bundle = edge_bundle_service::build(&state.pool, SITE, PublishEnvironment::Live)
        .await
        .expect("build bundle");

    let f = &bundle.features[0];
    assert!(
        f.saved_outcomes.contains_key(&saved.id),
        "keys: {:?}",
        f.saved_outcomes.keys().collect::<Vec<_>>()
    );
    assert_eq!(f.saved_outcomes[&saved.id].html_body, "<p>Subscribe</p>");
}

/// An unknown site is a 404, not an empty bundle — an operator exporting a
/// typo'd slug must find out at export time, not at the edge.
#[tokio::test]
async fn build_rejects_an_unknown_site() {
    let db = common::setup().await;
    let state = db.state.clone();

    let err = edge_bundle_service::build(&state.pool, "nope", PublishEnvironment::Live)
        .await
        .expect_err("unknown site must not build");

    assert!(matches!(err, AppError::SiteNotFound(_)), "got {err:?}");
    assert_eq!(err.code(), "SITE_NOT_FOUND");
}

/// The bundle must round-trip through `rre_core::edge::EdgeBundle`, which is the
/// type the proxy and the Fastly guest deserialize. A field the exporter renames
/// or drops fails HERE rather than at the POP.
#[tokio::test]
async fn build_output_round_trips_as_an_rre_core_bundle() {
    let db = common::setup().await;
    let state = db.state.clone();
    seed_site(&state, SITE, HOST).await;
    seed_feature(&state.pool, "round-trip", "json", 1).await;
    publish_version(
        &state,
        "round-trip",
        Some(canvas_with_action(processor(
            "trim_json",
            json!({ "json_path": "$.body", "length": 3 }),
        ))),
    )
    .await;

    let bundle = edge_bundle_service::build(&state.pool, SITE, PublishEnvironment::Live)
        .await
        .expect("build bundle");

    let wire = serde_json::to_string(&bundle).expect("serialize");
    let back: rre_core::edge::EdgeBundle = serde_json::from_str(&wire).expect("round-trip");

    assert_eq!(back.features.len(), 1);
    assert_eq!(back.features[0].id, "round-trip");
    assert_eq!(back.features[0].canvas().nodes.len(), 3);
    // `generated_at` must be RFC 3339 so a consumer can parse it.
    assert!(
        chrono::DateTime::parse_from_rfc3339(&back.generated_at).is_ok(),
        "generated_at not RFC 3339: {}",
        back.generated_at
    );
}
