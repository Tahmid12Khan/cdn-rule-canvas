//! Demo seeder (Task 20). Idempotent: creates the `dn-article` demo feature with
//! a LIVE version 1 whose anonymous canvas mirrors the worked example in the
//! expression-node flow (start → paywall meta-tag → device-type →
//! apply_outcome(regwall/paywall/show-content) → end), plus an editable DRAFT
//! version 2 cloned from it for the rule-builder UI. Also seeds the
//! `dn-json-article` (type `json`) demo feature whose anonymous canvas trims the
//! body and injects a paywall marker when `$.api == "dn-article"`.
//!
//! Run with:  `cargo run -p rre-backend --bin seed_demo`
//! (reads `DATABASE_URL` from the environment / `.env`).
//!
//! Idempotency: keyed on the fixed feature id + UUIDs below. Re-running performs
//! `ON CONFLICT DO NOTHING` inserts / `INSERT ... WHERE NOT EXISTS`, so the demo
//! state converges and never errors on a populated database. The JSON feature's
//! version graph is UPSERTED (rule_graph re-applied) so it converges even if the
//! row already exists.

#![forbid(unsafe_code)]

use anyhow::Context;
use rre_backend::models::enums::Placement;
use rre_backend::{config::Settings, db};
use serde_json::json;
use sqlx::PgPool;
use uuid::Uuid;

// --- Stable demo identifiers (fixed so re-seeding is idempotent) ---------------
const FEATURE_ID: &str = "dn-article";
const JSON_FEATURE_ID: &str = "dn-json-article";

// dn-json-article version 1 (LIVE) and its builtin Show Content outcome.
const JSON_V1_ID: Uuid = Uuid::from_u128(0xB0000000_0000_0000_0000_000000000001);
const JSON_O_BUILTIN: Uuid = Uuid::from_u128(0xB1111111_1111_1111_1111_111111111111);

// version 1 (LIVE — consumed by the proxy at env=live)
const V1_ID: Uuid = Uuid::from_u128(0xA0000000_0000_0000_0000_000000000001);
// version 2 (DRAFT — editable in the rule-builder)
const V2_ID: Uuid = Uuid::from_u128(0xA0000000_0000_0000_0000_000000000002);

// outcome ids — must match the rule_graph `outcome_id` references below.
const O_REGWALL: Uuid = Uuid::from_u128(0x11111111_1111_1111_1111_111111111111);
const O_PAYWALL: Uuid = Uuid::from_u128(0x22222222_2222_2222_2222_222222222222);
const O_CONTENT: Uuid = Uuid::from_u128(0x33333333_3333_3333_3333_333333333333);
// the builtin "Show Content" outcome on the editable draft
const O_DRAFT_BUILTIN: Uuid = Uuid::from_u128(0x44444444_4444_4444_4444_444444444444);

// component ids
const C_REGWALL_HTML: Uuid = Uuid::from_u128(0x51111111_1111_1111_1111_111111111111);
const C_PAYWALL_HTML: Uuid = Uuid::from_u128(0x52222222_2222_2222_2222_222222222222);
const C_PAYWALL_TRUNC: Uuid = Uuid::from_u128(0x53333333_3333_3333_3333_333333333333);

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let settings = Settings::load().context("failed to load settings")?;
    let pool = db::connect(&settings.database_url, 5)
        .await
        .context("failed to connect to database")?;
    db::run_migrations(&pool)
        .await
        .context("failed to run migrations")?;

    seed(&pool).await.context("seeding demo data")?;

    println!(
        "demo seed complete: feature `{FEATURE_ID}` (LIVE v1 + DRAFT v2), \
         feature `{JSON_FEATURE_ID}` (LIVE v1)"
    );
    Ok(())
}

/// Idempotently seed the demo feature, versions, outcomes and components.
pub async fn seed(pool: &PgPool) -> anyhow::Result<()> {
    let mut tx = pool.begin().await?;

    // --- feature ---------------------------------------------------------------
    sqlx::query(
        r#"
        INSERT INTO rre.features (id, name, "type")
        VALUES ($1, $2, 'html')
        ON CONFLICT (id) DO NOTHING
        "#,
    )
    .bind(FEATURE_ID)
    .bind("DN Article Paywall")
    .execute(&mut *tx)
    .await?;

    // --- version 1 (LIVE) with the worked-example anonymous canvas -------------
    let rule_graph = anonymous_rule_graph();

    sqlx::query(
        r#"
        INSERT INTO rre.versions
            (id, feature_id, version_number, description, status, rule_graph,
             created_by, last_updated_by)
        VALUES ($1, $2, 1, $3, 'live', $4, 'seed', 'seed')
        ON CONFLICT (id) DO NOTHING
        "#,
    )
    .bind(V1_ID)
    .bind(FEATURE_ID)
    .bind("Live demo: paywall + device-type routing")
    .bind(&rule_graph)
    .execute(&mut *tx)
    .await?;

    // point the feature's live slot at v1 ONLY when it has none yet — never
    // clobber a live version the user has since chosen (e.g. after editing the
    // canvas in the UI), so re-running the seed is non-destructive.
    sqlx::query(
        r#"
        UPDATE rre.features
        SET live_version_id = $1, updated_at = now()
        WHERE id = $2 AND live_version_id IS NULL
        "#,
    )
    .bind(V1_ID)
    .bind(FEATURE_ID)
    .execute(&mut *tx)
    .await?;

    // --- v1 outcomes -----------------------------------------------------------
    upsert_outcome(
        &mut tx,
        O_REGWALL,
        V1_ID,
        "Registration Wall",
        Some("Prompt anonymous mobile readers to register."),
        false,
        0,
    )
    .await?;
    upsert_outcome(
        &mut tx,
        O_PAYWALL,
        V1_ID,
        "Paywall",
        Some("Truncate the article and show a subscription prompt."),
        false,
        1,
    )
    .await?;
    upsert_outcome(
        &mut tx,
        O_CONTENT,
        V1_ID,
        "Show Content",
        Some("Serve the article untouched."),
        true,
        2,
    )
    .await?;

    // --- v1 components ---------------------------------------------------------
    // Registration wall: inject a registration prompt after the article body.
    upsert_component(
        &mut tx,
        C_REGWALL_HTML,
        O_REGWALL,
        "regwall-prompt",
        "html_injection",
        json!({
            "type": "html_injection",
            "target_selector": "#article-body",
            "placement_mode": "after",
            "html_body": "<section class=\"rre-regwall\"><h2>Create a free account to keep reading</h2><p>Register to read the rest of this article.</p></section>",
            "theme": "light"
        }),
        Placement::Inline,
        0,
    )
    .await?;

    // Paywall: truncate the article, then inject a subscribe prompt.
    upsert_component(
        &mut tx,
        C_PAYWALL_TRUNC,
        O_PAYWALL,
        "paywall-truncate",
        "content_truncation",
        json!({
            "type": "content_truncation",
            "target_selector": "#article-body",
            "word_count": 40,
            "fade_out": true
        }),
        Placement::Inline,
        0,
    )
    .await?;
    upsert_component(
        &mut tx,
        C_PAYWALL_HTML,
        O_PAYWALL,
        "paywall-prompt",
        "html_injection",
        json!({
            "type": "html_injection",
            "target_selector": "#article-body",
            "placement_mode": "after",
            "html_body": "<section class=\"rre-paywall\"><h2>Subscribe to continue</h2><p>This article is for subscribers only.</p></section>",
            "theme": "light"
        }),
        Placement::StickyFooter,
        1,
    )
    .await?;

    // --- version 2 (DRAFT) — editable clone for the rule builder ---------------
    sqlx::query(
        r#"
        INSERT INTO rre.versions
            (id, feature_id, version_number, description, status, rule_graph,
             created_by, last_updated_by)
        VALUES ($1, $2, 2, $3, 'draft',
                '{"anonymous":{"nodes":[],"edges":[],"root_node_id":null},
                  "registered":{"nodes":[],"edges":[],"root_node_id":null},
                  "customer":{"nodes":[],"edges":[],"root_node_id":null}}'::jsonb,
                'seed', 'seed')
        ON CONFLICT (id) DO NOTHING
        "#,
    )
    .bind(V2_ID)
    .bind(FEATURE_ID)
    .bind("Editable draft (rule-builder demo)")
    .execute(&mut *tx)
    .await?;

    // builtin Show Content outcome on the draft so the editor has a target.
    upsert_outcome(
        &mut tx,
        O_DRAFT_BUILTIN,
        V2_ID,
        "Show Content",
        Some("Serve the article untouched."),
        true,
        0,
    )
    .await?;

    seed_json_feature(&mut tx).await?;

    tx.commit().await?;
    Ok(())
}

/// Idempotently seed the `dn-json-article` JSON feature with one LIVE version
/// whose anonymous canvas trims `$.body` and injects `$.paywall_show` when
/// `$.api == "dn-article"`. The version row is UPSERTED on its rule_graph so a
/// pre-existing row converges to this canvas rather than duplicating.
async fn seed_json_feature(tx: &mut sqlx::Transaction<'_, sqlx::Postgres>) -> anyhow::Result<()> {
    sqlx::query(
        r#"
        INSERT INTO rre.features (id, name, "type")
        VALUES ($1, $2, 'json')
        ON CONFLICT (id) DO NOTHING
        "#,
    )
    .bind(JSON_FEATURE_ID)
    .bind("DN JSON Article Paywall")
    .execute(&mut **tx)
    .await?;

    // Orphan-proofing: a prior partial seed (predating the stable-id scheme) may
    // have left a version row at (dn-json-article, 1) under a NON-canonical id.
    // That collides with JSON_V1_ID on the (feature_id, version_number) unique
    // constraint, which the `ON CONFLICT (id)` upsert below does NOT catch — the
    // insert would error and abort the whole seed. Clear this feature's versions
    // first (outcomes/components cascade) so the canonical insert is always clean.
    // Scoped to JSON_FEATURE_ID only — never touches dn-article.
    sqlx::query(
        "UPDATE rre.features SET live_version_id = NULL, staging_version_id = NULL WHERE id = $1",
    )
    .bind(JSON_FEATURE_ID)
    .execute(&mut **tx)
    .await?;
    sqlx::query("DELETE FROM rre.versions WHERE feature_id = $1")
        .bind(JSON_FEATURE_ID)
        .execute(&mut **tx)
        .await?;

    let rule_graph = json_anonymous_rule_graph();

    // Upsert by the stable version id: insert on first run, otherwise refresh the
    // rule_graph so the seeded canvas converges to the latest shape.
    sqlx::query(
        r#"
        INSERT INTO rre.versions
            (id, feature_id, version_number, description, status, rule_graph,
             created_by, last_updated_by)
        VALUES ($1, $2, 1, $3, 'live', $4, 'seed', 'seed')
        ON CONFLICT (id) DO UPDATE
        SET rule_graph = EXCLUDED.rule_graph,
            last_updated_by = 'seed',
            last_updated_at = now()
        "#,
    )
    .bind(JSON_V1_ID)
    .bind(JSON_FEATURE_ID)
    .bind("Live demo: JSON trim + paywall marker")
    .bind(&rule_graph)
    .execute(&mut **tx)
    .await?;

    // Point the feature's live slot at v1 (idempotent — set only if unset/other).
    sqlx::query(
        r#"
        UPDATE rre.features
        SET live_version_id = $1, updated_at = now()
        WHERE id = $2 AND (live_version_id IS DISTINCT FROM $1)
        "#,
    )
    .bind(JSON_V1_ID)
    .bind(JSON_FEATURE_ID)
    .execute(&mut **tx)
    .await?;

    // A builtin Show Content outcome so the version is well-formed (the canvas
    // itself references no outcome — the JSON actions are self-contained).
    upsert_outcome(
        tx,
        JSON_O_BUILTIN,
        JSON_V1_ID,
        "Show Content",
        Some("Serve the JSON untouched."),
        true,
        0,
    )
    .await?;

    Ok(())
}

/// The anonymous canvas worked example in the new expression-node taxonomy:
/// `start → paywall meta-tag → device-type → apply_outcome(regwall/paywall/
/// show-content) → end`. `registered`/`customer` are empty canvases.
fn anonymous_rule_graph() -> serde_json::Value {
    json!({
        "anonymous": {
            "root_node_id": "start",
            "nodes": [
                { "kind": "start", "id": "start", "position": { "x": 40, "y": 200 } },
                { "kind": "decision", "id": "n_meta",
                  "processor": { "type": "meta_tags", "tag_name": "paywall", "operator": "contains", "value": "true" },
                  "position": { "x": 200, "y": 200 } },
                { "kind": "decision", "id": "n_dev",
                  "processor": { "type": "device_type", "operator": "equals", "value": "mobile" },
                  "position": { "x": 420, "y": 120 } },
                { "kind": "expression", "id": "a_regwall",
                  "action": { "type": "apply_outcome", "outcome_id": O_REGWALL.to_string() },
                  "position": { "x": 660, "y": 60 } },
                { "kind": "expression", "id": "a_paywall",
                  "action": { "type": "apply_outcome", "outcome_id": O_PAYWALL.to_string() },
                  "position": { "x": 660, "y": 200 } },
                { "kind": "expression", "id": "a_content",
                  "action": { "type": "apply_outcome", "outcome_id": O_CONTENT.to_string() },
                  "position": { "x": 420, "y": 320 } },
                { "kind": "end", "id": "end", "position": { "x": 900, "y": 200 } }
            ],
            "edges": [
                { "id": "e_start", "source_node_id": "start",     "target_node_id": "n_meta",    "branch": "yes" },
                { "id": "e1", "source_node_id": "n_meta",    "target_node_id": "n_dev",     "branch": "yes" },
                { "id": "e2", "source_node_id": "n_meta",    "target_node_id": "a_content", "branch": "no"  },
                { "id": "e3", "source_node_id": "n_dev",     "target_node_id": "a_regwall", "branch": "yes" },
                { "id": "e4", "source_node_id": "n_dev",     "target_node_id": "a_paywall", "branch": "no"  },
                { "id": "e_end_regwall", "source_node_id": "a_regwall", "target_node_id": "end", "branch": "yes" },
                { "id": "e_end_paywall", "source_node_id": "a_paywall", "target_node_id": "end", "branch": "yes" },
                { "id": "e_end_content", "source_node_id": "a_content", "target_node_id": "end", "branch": "yes" }
            ]
        },
        "registered": { "root_node_id": null, "nodes": [], "edges": [] },
        "customer":   { "root_node_id": null, "nodes": [], "edges": [] }
    })
}

/// The anonymous canvas for the `dn-json-article` JSON feature:
/// `start → json_expression($.api == "dn-article") → [yes] trim_json($.body, 0)
/// → add_attribute($.paywall_show, "<html>paywall_showed</html>") → end ;
/// [no] → end`.
fn json_anonymous_rule_graph() -> serde_json::Value {
    json!({
        "anonymous": {
            "root_node_id": "start",
            "nodes": [
                { "kind": "start", "id": "start", "position": { "x": 40, "y": 160 } },
                { "kind": "decision", "id": "d_api",
                  "processor": { "type": "json_expression", "json_path": "$.api", "operator": "equals", "value": "dn-article" },
                  "position": { "x": 220, "y": 160 } },
                { "kind": "expression", "id": "t_body",
                  "action": { "type": "trim_json", "json_path": "$.body", "length": 0 },
                  "position": { "x": 460, "y": 80 } },
                { "kind": "expression", "id": "a_pw",
                  "action": { "type": "add_attribute", "json_path": "$.paywall_show", "value": "<html>paywall_showed</html>" },
                  "position": { "x": 700, "y": 80 } },
                { "kind": "end", "id": "end", "position": { "x": 940, "y": 160 } }
            ],
            "edges": [
                { "id": "e_start", "source_node_id": "start",  "target_node_id": "d_api",  "branch": "yes" },
                { "id": "e_yes",   "source_node_id": "d_api",  "target_node_id": "t_body", "branch": "yes" },
                { "id": "e_no",    "source_node_id": "d_api",  "target_node_id": "end",    "branch": "no"  },
                { "id": "e_trim",  "source_node_id": "t_body", "target_node_id": "a_pw",   "branch": "yes" },
                { "id": "e_attr",  "source_node_id": "a_pw",   "target_node_id": "end",    "branch": "yes" }
            ]
        },
        "registered": { "root_node_id": null, "nodes": [], "edges": [] },
        "customer":   { "root_node_id": null, "nodes": [], "edges": [] }
    })
}

/// Idempotent outcome upsert (insert if the fixed id is absent).
async fn upsert_outcome(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    id: Uuid,
    version_id: Uuid,
    title: &str,
    description: Option<&str>,
    is_builtin: bool,
    order_index: i32,
) -> anyhow::Result<()> {
    sqlx::query(
        r#"
        INSERT INTO rre.outcomes (id, version_id, title, description, is_builtin, order_index)
        VALUES ($1, $2, $3, $4, $5, $6)
        ON CONFLICT (id) DO NOTHING
        "#,
    )
    .bind(id)
    .bind(version_id)
    .bind(title)
    .bind(description)
    .bind(is_builtin)
    .bind(order_index)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

/// Idempotent component upsert (insert if the fixed id is absent).
#[allow(clippy::too_many_arguments)]
async fn upsert_component(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    id: Uuid,
    outcome_id: Uuid,
    slug: &str,
    r#type: &str,
    config: serde_json::Value,
    placement: Placement,
    order_index: i32,
) -> anyhow::Result<()> {
    sqlx::query(
        r#"
        INSERT INTO rre.components (id, outcome_id, slug, "type", config, placement, order_index)
        VALUES ($1, $2, $3, $4, $5, $6, $7)
        ON CONFLICT (id) DO NOTHING
        "#,
    )
    .bind(id)
    .bind(outcome_id)
    .bind(slug)
    .bind(r#type)
    .bind(config)
    .bind(placement)
    .bind(order_index)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use rre_backend::schemas::node_type::LoadedManifest;
    use rre_backend::schemas::rule_graph::RuleGraph;
    use rre_backend::services::rule_graph_service;
    use std::collections::HashSet;

    /// The seeded `dn-article` anonymous canvas parses and passes ALL validation
    /// rules, including `all_paths_reach_end`: `root_node_id` is the start node
    /// and every reachable node reaches an end. Guards against a seed the API
    /// would reject.
    #[test]
    fn seed_graph_validates() {
        let graph: RuleGraph =
            serde_json::from_value(anonymous_rule_graph()).expect("seed graph parses");

        let valid_outcome_ids: HashSet<Uuid> = HashSet::from([O_REGWALL, O_PAYWALL, O_CONTENT]);
        let manifest = LoadedManifest::load("config/node_types.json")
            .expect("load manifest")
            .typed;

        rule_graph_service::validate(&graph, &valid_outcome_ids, &manifest)
            .expect("seed graph must validate");

        assert_eq!(
            graph.anonymous.root_node_id.as_deref(),
            Some("start"),
            "seed must anchor the canvas root at the start node"
        );
    }

    /// The seeded `dn-json-article` anonymous canvas parses and passes ALL
    /// validation rules. It references no outcomes (JSON actions are
    /// self-contained), so an empty valid-outcome set suffices.
    #[test]
    fn json_seed_graph_validates() {
        let graph: RuleGraph =
            serde_json::from_value(json_anonymous_rule_graph()).expect("json seed graph parses");

        let manifest = LoadedManifest::load("config/node_types.json")
            .expect("load manifest")
            .typed;

        rule_graph_service::validate(&graph, &HashSet::new(), &manifest)
            .expect("json seed graph must validate");

        assert_eq!(
            graph.anonymous.root_node_id.as_deref(),
            Some("start"),
            "json seed must anchor the canvas root at the start node"
        );
    }
}
