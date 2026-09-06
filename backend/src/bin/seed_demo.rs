//! Demo seeder (Task 20). Idempotent: creates the `demo-article` demo feature
//! with a LIVE version 1 whose Rule Canvas mirrors the worked example in the
//! expression-node flow (start → paywall meta-tag → device-type →
//! apply_outcome(regwall/paywall/show-content) → end), plus an editable DRAFT
//! version 2 cloned from it for the rule-builder UI. Also seeds the
//! `demo-json-article` (type `json`) demo feature whose Rule Canvas trims
//! the body and injects a paywall marker when `$.api == "demo-article"`, and a
//! demo Site (`localhost:9000 → demo-upstream:8081`) the proxy routes through.
//!
//! Run with:  `cargo run -p rre-backend --bin seed_demo`
//! (reads `DATABASE_URL` from the environment / `.env`).
//!
//! The demo Site's destination host/port are overridable via the env vars
//! `DEMO_UPSTREAM_HOST` / `DEMO_UPSTREAM_PORT` (for native dev where
//! `demo-upstream` does not resolve).
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
const FEATURE_ID: &str = "demo-article";
const JSON_FEATURE_ID: &str = "demo-json-article";

// Demo Site: routes the proxy's inbound `localhost:9000` to the demo upstream.
const SITE_SLUG: &str = "demo-localhost";
const SITE_NAME: &str = "Demo (localhost:9000)";
const SITE_SOURCE_HOST: &str = "localhost";
const SITE_SOURCE_PORT: i32 = 9000;
const DEFAULT_DEST_HOST: &str = "demo-upstream";
const DEFAULT_DEST_PORT: i32 = 8081;

// demo-json-article version 1 (LIVE) and its builtin Show Content outcome.
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

// Component-template (Component Editor library) ids — a global "Promo Banner"
// component + its v1. Wired into the editable DRAFT v2 of demo-article via an
// `apply_component` expression node so the new flow is demonstrable end-to-end.
const CT_PROMO_BANNER: Uuid = Uuid::from_u128(0x60000000_0000_0000_0000_000000000001);
const CT_PROMO_BANNER_V1: Uuid = Uuid::from_u128(0x60000000_0000_0000_0000_0000000000A1);
const CT_PROMO_BANNER_SLUG: &str = "promo-banner";

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
        "demo seed complete: feature `{FEATURE_ID}` (LIVE v1 + DRAFT v2 with \
         apply_component), feature `{JSON_FEATURE_ID}` (LIVE v1), \
         component `{CT_PROMO_BANNER_SLUG}` (v1), site `{SITE_SLUG}`"
    );
    Ok(())
}

/// Idempotently seed the demo feature, versions, outcomes and components.
pub async fn seed(pool: &PgPool) -> anyhow::Result<()> {
    let mut tx = pool.begin().await?;

    // --- feature ---------------------------------------------------------------
    sqlx::query(
        r#"
        INSERT INTO rre.features (id, name, "type", execution_order)
        VALUES ($1, $2, 'html', 1)
        ON CONFLICT (id) DO NOTHING
        "#,
    )
    .bind(FEATURE_ID)
    .bind("Demo Article Paywall")
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

    // --- Component Editor library: the global "Promo Banner" component + v1.
    // Seeded BEFORE the draft v2 below so the `apply_component` node wired into
    // that draft passes `apply_component_ref_exists` validation.
    seed_promo_banner(&mut tx).await?;

    // --- version 2 (DRAFT) — editable clone for the rule builder ---------------
    // The anonymous canvas demonstrates the Component Editor flow end-to-end:
    // start → apply_component(Promo Banner, version="default") → end.
    let draft_graph = draft_rule_graph();
    sqlx::query(
        r#"
        INSERT INTO rre.versions
            (id, feature_id, version_number, description, status, rule_graph,
             created_by, last_updated_by)
        VALUES ($1, $2, 2, $3, 'draft', $4, 'seed', 'seed')
        ON CONFLICT (id) DO NOTHING
        "#,
    )
    .bind(V2_ID)
    .bind(FEATURE_ID)
    .bind("Editable draft (rule-builder demo)")
    .bind(&draft_graph)
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
    seed_site(&mut tx).await?;

    tx.commit().await?;
    Ok(())
}

/// Idempotently seed the demo Site (`localhost:9000 → demo-upstream:8081`). The
/// destination host/port are overridable via the env vars `DEMO_UPSTREAM_HOST` /
/// `DEMO_UPSTREAM_PORT` (native dev cannot resolve the `demo-upstream` hostname).
/// Existing rows are left untouched (`ON CONFLICT DO NOTHING`) so a user-edited
/// Site is never clobbered by a re-seed.
async fn seed_site(tx: &mut sqlx::Transaction<'_, sqlx::Postgres>) -> anyhow::Result<()> {
    let dest_host =
        std::env::var("DEMO_UPSTREAM_HOST").unwrap_or_else(|_| DEFAULT_DEST_HOST.into());
    let dest_port = std::env::var("DEMO_UPSTREAM_PORT")
        .ok()
        .and_then(|p| p.parse::<i32>().ok())
        .unwrap_or(DEFAULT_DEST_PORT);

    sqlx::query(
        r#"
        INSERT INTO rre.sites
            (slug, name, source_protocol, source_host, source_port,
             dest_protocol, dest_host, dest_port)
        VALUES ($1, $2, 'http', $3, $4, 'http', $5, $6)
        ON CONFLICT (slug) DO NOTHING
        "#,
    )
    .bind(SITE_SLUG)
    .bind(SITE_NAME)
    .bind(SITE_SOURCE_HOST)
    .bind(SITE_SOURCE_PORT)
    .bind(&dest_host)
    .bind(dest_port)
    .execute(&mut **tx)
    .await?;

    Ok(())
}

/// Idempotently seed the global "Promo Banner" Component Editor template + its
/// v1. The body is a small mustache snippet using `{{headline}}`, `{{cta_label}}`
/// and `{{cta_href}}` (rendered as a heading + anchor button); the variables
/// carry author `title` + `description` metadata for the rule-side population UI.
/// `default_mode = 'latest'` so a rule following `version = "default"` always
/// resolves to the newest version. Keyed on the fixed component + version ids
/// (`ON CONFLICT DO NOTHING`) so a re-seed converges and never clobbers an
/// author-edited component.
async fn seed_promo_banner(tx: &mut sqlx::Transaction<'_, sqlx::Postgres>) -> anyhow::Result<()> {
    // The component row. default_version_id is wired AFTER the v1 insert (the FK
    // is DEFERRABLE INITIALLY DEFERRED, so set-then-commit is fine in one tx).
    sqlx::query(
        r#"
        INSERT INTO rre.component_templates (id, slug, name, description, default_mode)
        VALUES ($1, $2, $3, $4, 'latest')
        ON CONFLICT (id) DO NOTHING
        "#,
    )
    .bind(CT_PROMO_BANNER)
    .bind(CT_PROMO_BANNER_SLUG)
    .bind("Promo Banner")
    .bind("Reusable call-to-action banner: a headline plus a button link.")
    .execute(&mut **tx)
    .await?;

    let html_body = "<section class=\"rre-promo-banner\">\n  \
        <h2>{{headline}}</h2>\n  \
        <a class=\"rre-promo-cta\" href=\"{{cta_href}}\">{{cta_label}}</a>\n\
        </section>";

    let variables = json!([
        {
            "name": "headline",
            "title": "Headline",
            "description": "The banner's main heading text."
        },
        {
            "name": "cta_label",
            "title": "Button label",
            "description": "The visible text of the call-to-action button."
        },
        {
            "name": "cta_href",
            "title": "Button link",
            "description": "The URL the call-to-action button points to."
        }
    ]);

    sqlx::query(
        r#"
        INSERT INTO rre.component_template_versions
            (id, component_id, version_number, description, html_body, variables)
        VALUES ($1, $2, 1, $3, $4, $5)
        ON CONFLICT (id) DO NOTHING
        "#,
    )
    .bind(CT_PROMO_BANNER_V1)
    .bind(CT_PROMO_BANNER)
    .bind("Initial promo banner")
    .bind(html_body)
    .bind(&variables)
    .execute(&mut **tx)
    .await?;

    // Point the default pointer at v1 only when unset, so a re-seed never
    // re-pins a default the author has since moved.
    sqlx::query(
        r#"
        UPDATE rre.component_templates
        SET default_version_id = $1, updated_at = now()
        WHERE id = $2 AND default_version_id IS NULL
        "#,
    )
    .bind(CT_PROMO_BANNER_V1)
    .bind(CT_PROMO_BANNER)
    .execute(&mut **tx)
    .await?;

    Ok(())
}

/// Idempotently seed the `demo-json-article` JSON feature with one LIVE version
/// whose Rule Canvas trims `$.body` and injects `$.paywall_show` when
/// `$.api == "demo-article"`. The version row is UPSERTED on its rule_graph so a
/// pre-existing row converges to this canvas rather than duplicating.
async fn seed_json_feature(tx: &mut sqlx::Transaction<'_, sqlx::Postgres>) -> anyhow::Result<()> {
    // execution_order is per-type, so the JSON feature is order 1 independently
    // of the HTML demo feature (which is also order 1).
    sqlx::query(
        r#"
        INSERT INTO rre.features (id, name, "type", execution_order)
        VALUES ($1, $2, 'json', 1)
        ON CONFLICT (id) DO NOTHING
        "#,
    )
    .bind(JSON_FEATURE_ID)
    .bind("Demo JSON Article Paywall")
    .execute(&mut **tx)
    .await?;

    // Orphan-proofing: a prior partial seed (predating the stable-id scheme) may
    // have left a version row at (demo-json-article, 1) under a NON-canonical id.
    // That collides with JSON_V1_ID on the (feature_id, version_number) unique
    // constraint, which the `ON CONFLICT (id)` upsert below does NOT catch — the
    // insert would error and abort the whole seed. Clear this feature's versions
    // first (outcomes/components cascade) so the canonical insert is always clean.
    // Scoped to JSON_FEATURE_ID only — never touches demo-article.
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

/// The Rule Canvas worked example in the new expression-node taxonomy:
/// `start → paywall meta-tag → device-type → apply_outcome(regwall/paywall/
/// show-content) → end`.
fn anonymous_rule_graph() -> serde_json::Value {
    json!({
        "canvas": {
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
        }
    })
}

/// The editable DRAFT v2 canvas, demonstrating the Component Editor
/// flow end-to-end: `start → apply_component(Promo Banner, version="default",
/// variables filled) → end`. `placement_mode = append` injects the rendered
/// banner at the `main` target selector. Passes `rule_graph_service::validate`
/// (incl. `apply_component_ref_exists`, since the Promo Banner component is seeded).
fn draft_rule_graph() -> serde_json::Value {
    json!({
        "canvas": {
            "root_node_id": "start",
            "nodes": [
                { "kind": "start", "id": "start", "position": { "x": 40, "y": 200 } },
                { "kind": "expression", "id": "a_promo",
                  "action": {
                      "type": "apply_component",
                      "component_id": CT_PROMO_BANNER.to_string(),
                      "version": "default",
                      "target_selector": "main",
                      "placement_mode": "append",
                      "variables": {
                          "headline": "Enjoying this article?",
                          "cta_label": "Subscribe now",
                          "cta_href": "/subscribe"
                      }
                  },
                  "position": { "x": 280, "y": 200 } },
                { "kind": "end", "id": "end", "position": { "x": 540, "y": 200 } }
            ],
            "edges": [
                { "id": "e_start", "source_node_id": "start",   "target_node_id": "a_promo", "branch": "yes" },
                { "id": "e_end",   "source_node_id": "a_promo", "target_node_id": "end",     "branch": "yes" }
            ]
        }
    })
}

/// The Rule Canvas for the `demo-json-article` JSON feature:
/// `start → json_expression($.api == "demo-article") → [yes] trim_json($.body, 0)
/// → add_attribute($.paywall_show, "<html>paywall_showed</html>") → end ;
/// [no] → end`.
fn json_anonymous_rule_graph() -> serde_json::Value {
    json!({
        "canvas": {
            "root_node_id": "start",
            "nodes": [
                { "kind": "start", "id": "start", "position": { "x": 40, "y": 160 } },
                { "kind": "decision", "id": "d_api",
                  "processor": { "type": "json_expression", "json_path": "$.api", "operator": "equals", "value": "demo-article" },
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
        }
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

    /// The seeded `demo-article` anonymous canvas parses and passes ALL
    /// validation rules, including `all_paths_reach_end`: `root_node_id` is the
    /// start node and every reachable node reaches an end. Guards against a seed
    /// the API would reject.
    #[test]
    fn seed_graph_validates() {
        let graph: RuleGraph =
            serde_json::from_value(anonymous_rule_graph()).expect("seed graph parses");

        let valid_outcome_ids: HashSet<Uuid> = HashSet::from([O_REGWALL, O_PAYWALL, O_CONTENT]);
        let manifest = LoadedManifest::load("config/node_types.json")
            .expect("load manifest")
            .typed;

        rule_graph_service::validate(
            &graph,
            &valid_outcome_ids,
            &HashSet::new(),
            &HashSet::new(),
            &manifest,
        )
        .expect("seed graph must validate");

        assert_eq!(
            graph.canvas.root_node_id.as_deref(),
            Some("start"),
            "seed must anchor the canvas root at the start node"
        );
    }

    /// The seeded editable DRAFT v2 anonymous canvas parses and passes ALL
    /// validation rules, including `apply_component_ref_exists`: the wired
    /// `apply_component` node references the seeded Promo Banner component id.
    /// Guards against a draft seed the API would reject.
    #[test]
    fn draft_graph_validates() {
        let graph: RuleGraph =
            serde_json::from_value(draft_rule_graph()).expect("draft graph parses");

        let valid_component_ids: HashSet<Uuid> = HashSet::from([CT_PROMO_BANNER]);
        let manifest = LoadedManifest::load("config/node_types.json")
            .expect("load manifest")
            .typed;

        rule_graph_service::validate(
            &graph,
            &HashSet::new(),
            &HashSet::new(),
            &valid_component_ids,
            &manifest,
        )
        .expect("draft graph must validate");

        assert_eq!(
            graph.canvas.root_node_id.as_deref(),
            Some("start"),
            "draft seed must anchor the canvas root at the start node"
        );
    }

    /// The draft graph's `apply_component` node references the seeded Promo
    /// Banner component id. With an EMPTY valid-component set, validation must
    /// FAIL on `apply_component_ref_exists` — proving the test above is real.
    #[test]
    fn draft_graph_rejects_unknown_component() {
        let graph: RuleGraph =
            serde_json::from_value(draft_rule_graph()).expect("draft graph parses");

        let manifest = LoadedManifest::load("config/node_types.json")
            .expect("load manifest")
            .typed;

        let err = rule_graph_service::validate(
            &graph,
            &HashSet::new(),
            &HashSet::new(),
            &HashSet::new(),
            &manifest,
        )
        .expect_err("unknown component must fail validation");
        let msg = format!("{err:?}");
        assert!(
            msg.contains("apply_component_ref_exists"),
            "expected apply_component_ref_exists failure, got: {msg}"
        );
    }

    /// The seeded `demo-json-article` anonymous canvas parses and passes ALL
    /// validation rules. It references no outcomes (JSON actions are
    /// self-contained), so an empty valid-outcome set suffices.
    #[test]
    fn json_seed_graph_validates() {
        let graph: RuleGraph =
            serde_json::from_value(json_anonymous_rule_graph()).expect("json seed graph parses");

        let manifest = LoadedManifest::load("config/node_types.json")
            .expect("load manifest")
            .typed;

        rule_graph_service::validate(
            &graph,
            &HashSet::new(),
            &HashSet::new(),
            &HashSet::new(),
            &manifest,
        )
        .expect("json seed graph must validate");

        assert_eq!(
            graph.canvas.root_node_id.as_deref(),
            Some("start"),
            "json seed must anchor the canvas root at the start node"
        );
    }
}
