//! Demo seeder (Task 20). Idempotent: creates the `dn-article` demo feature with
//! a LIVE version 1 whose anonymous canvas mirrors the BACKEND CONTRACT §6 worked
//! example (paywall meta-tag → device-type → regwall/paywall/show-content), plus
//! an editable DRAFT version 2 cloned from it for the rule-builder UI.
//!
//! Run with:  `cargo run -p rre-backend --bin seed_demo`
//! (reads `DATABASE_URL` from the environment / `.env`).
//!
//! Idempotency: keyed on the fixed feature id + UUIDs below. Re-running performs
//! `ON CONFLICT DO NOTHING` inserts / `INSERT ... WHERE NOT EXISTS`, so the demo
//! state converges and never errors on a populated database.

#![forbid(unsafe_code)]

use anyhow::Context;
use rre_backend::models::enums::Placement;
use rre_backend::{config::Settings, db};
use serde_json::json;
use sqlx::PgPool;
use uuid::Uuid;

// --- Stable demo identifiers (fixed so re-seeding is idempotent) ---------------
const FEATURE_ID: &str = "dn-article";

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

    println!("demo seed complete: feature `{FEATURE_ID}` (LIVE v1 + DRAFT v2)");
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

    // point the feature's live slot at v1 (idempotent — set only if unset/other)
    sqlx::query(
        r#"
        UPDATE rre.features
        SET live_version_id = $1, updated_at = now()
        WHERE id = $2 AND (live_version_id IS DISTINCT FROM $1)
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

    tx.commit().await?;
    Ok(())
}

/// The anonymous canvas from BACKEND CONTRACT §6 (paywall → device → outcomes).
/// `registered`/`customer` are empty canvases.
fn anonymous_rule_graph() -> serde_json::Value {
    json!({
        "anonymous": {
            "root_node_id": "n_meta",
            "nodes": [
                { "kind": "decision", "id": "n_meta",
                  "processor": { "type": "meta_tags", "tag_name": "paywall", "operator": "contains", "value": "true" },
                  "position": { "x": 80, "y": 200 } },
                { "kind": "decision", "id": "n_dev",
                  "processor": { "type": "device_type", "operator": "equals", "value": "mobile" },
                  "position": { "x": 360, "y": 120 } },
                { "kind": "outcome", "id": "n_regwall", "outcome_id": O_REGWALL.to_string(),
                  "position": { "x": 640, "y": 60 } },
                { "kind": "outcome", "id": "n_paywall", "outcome_id": O_PAYWALL.to_string(),
                  "position": { "x": 640, "y": 200 } },
                { "kind": "outcome", "id": "n_content", "outcome_id": O_CONTENT.to_string(),
                  "position": { "x": 360, "y": 320 } }
            ],
            "edges": [
                { "id": "e1", "source_node_id": "n_meta", "target_node_id": "n_dev",     "branch": "yes" },
                { "id": "e2", "source_node_id": "n_meta", "target_node_id": "n_content", "branch": "no"  },
                { "id": "e3", "source_node_id": "n_dev",  "target_node_id": "n_regwall", "branch": "yes" },
                { "id": "e4", "source_node_id": "n_dev",  "target_node_id": "n_paywall", "branch": "no"  }
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
