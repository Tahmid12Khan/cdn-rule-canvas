//! Coverage for the 0007 expression-node data migration (spec §6).
//!
//! The `0007_expression_nodes.up.sql` migration drops its transient transform
//! function on completion, so this test re-creates the same helper from the
//! committed SQL, seeds an OLD-shape (outcome-node) rule_graph row, runs the
//! per-canvas UPDATE, and asserts the result is the NEW Start/Expression/End
//! pipeline that the validator accepts. It also checks idempotency (running the
//! transform again is a no-op) and the best-effort down reverse.

mod common;

use std::collections::HashSet;

use rre_backend::schemas::node_type::LoadedManifest;
use rre_backend::schemas::rule_graph::RuleGraph;
use rre_backend::services::rule_graph_service;
use serde_json::{json, Value};
use sqlx::Row;
use uuid::Uuid;

/// Extract just the `CREATE FUNCTION … $func$;` block from a migration file so
/// the test can recreate the (otherwise-dropped) transform helper.
fn function_block(sql: &str) -> String {
    let start = sql
        .find("CREATE OR REPLACE FUNCTION")
        .expect("function start");
    // The block ends at the first `$func$;` after the body terminator.
    let body_end = sql.find("END;\n$func$;").expect("function body end");
    let end = body_end + "END;\n$func$;".len();
    sql[start..end].to_string()
}

#[tokio::test]
async fn migration_0007_transforms_outcomes_to_pipeline() {
    let db = common::setup().await;
    let pool = &db.state.pool;

    let up = include_str!("../migrations/0007_expression_nodes.up.sql");
    sqlx::query(sqlx::AssertSqlSafe(function_block(up)))
        .execute(pool)
        .await
        .expect("recreate up function");

    // Seed a feature + version whose anonymous canvas is the OLD outcome shape.
    let o1 = Uuid::new_v4();
    let o2 = Uuid::new_v4();
    let old_graph = json!({
        "anonymous": {
            "root_node_id": "n_meta",
            "nodes": [
                { "kind": "decision", "id": "n_meta",
                  "processor": { "type": "device_type", "operator": "equals", "value": "mobile" },
                  "position": { "x": 80, "y": 200 } },
                { "kind": "outcome", "id": "n_a", "outcome_id": o1.to_string(),
                  "position": { "x": 360, "y": 120 } },
                { "kind": "outcome", "id": "n_b", "outcome_id": o2.to_string(),
                  "position": { "x": 360, "y": 320 } }
            ],
            "edges": [
                { "id": "e1", "source_node_id": "n_meta", "target_node_id": "n_a", "branch": "yes" },
                { "id": "e2", "source_node_id": "n_meta", "target_node_id": "n_b", "branch": "no"  }
            ]
        },
        "registered": { "root_node_id": null, "nodes": [], "edges": [] },
        "customer":   { "root_node_id": null, "nodes": [], "edges": [] }
    });

    sqlx::query(
        "INSERT INTO rre.features (id, name, type, execution_order) VALUES ('mig-0007', 'm', 'html', 1)",
    )
        .execute(pool)
        .await
        .expect("seed feature");
    let vid = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO rre.versions (id, feature_id, version_number, status, rule_graph, created_by, last_updated_by) \
         VALUES ($1, 'mig-0007', 1, 'draft', $2, 't', 't')",
    )
    .bind(vid)
    .bind(&old_graph)
    .execute(pool)
    .await
    .expect("seed version");

    // Run the migration's per-canvas UPDATE.
    let update = r#"
        UPDATE rre.versions
        SET rule_graph = jsonb_build_object(
              'anonymous',  rre.migrate_canvas_0007(rule_graph->'anonymous'),
              'registered', rre.migrate_canvas_0007(rule_graph->'registered'),
              'customer',   rre.migrate_canvas_0007(rule_graph->'customer'))
        WHERE id = $1
    "#;
    sqlx::query(update)
        .bind(vid)
        .execute(pool)
        .await
        .expect("run migrate update");

    let migrated: Value = sqlx::query("SELECT rule_graph FROM rre.versions WHERE id = $1")
        .bind(vid)
        .fetch_one(pool)
        .await
        .expect("read migrated graph")
        .get(0);

    // Migration 0007 predates 0013 (the single-canvas collapse), so at this
    // point in migration history the row is still the 3-key
    // {anonymous, registered, customer} shape — index it as raw JSON rather
    // than parsing the top-level object into the (now single-canvas)
    // `RuleGraph`. The `anonymous` canvas object itself is still a
    // `CanvasGraph`, which hasn't changed shape, so parse that piece and wrap
    // it to keep validating against the current manifest-driven validator.
    let canvas_graph: rre_backend::schemas::rule_graph::CanvasGraph =
        serde_json::from_value(migrated["anonymous"].clone())
            .expect("migrated anonymous canvas parses to new shape");
    let graph = RuleGraph {
        canvas: canvas_graph,
    };
    assert_eq!(
        graph.canvas.root_node_id.as_deref(),
        Some("start"),
        "root anchored at the injected start node"
    );
    let kinds: Vec<&str> = migrated["anonymous"]["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .map(|n| n["kind"].as_str().unwrap())
        .collect();
    assert!(kinds.contains(&"start"));
    assert!(kinds.contains(&"end"));
    assert!(kinds.contains(&"expression"));
    assert!(!kinds.contains(&"outcome"), "no outcome nodes remain");

    let manifest = LoadedManifest::load("config/node_types.json")
        .expect("manifest")
        .typed;
    let valid: HashSet<Uuid> = HashSet::from([o1, o2]);
    rule_graph_service::validate(&graph, &valid, &manifest)
        .expect("migrated graph must validate under the new rules");

    // Idempotency: re-running the transform on the migrated canvas is a no-op.
    let again = sqlx::query("SELECT rre.migrate_canvas_0007($1)")
        .bind(&migrated["anonymous"])
        .fetch_one(pool)
        .await
        .expect("re-run transform")
        .get::<Value, _>(0);
    assert_eq!(
        again, migrated["anonymous"],
        "re-running the migration must be a no-op"
    );

    // Best-effort down reverse: apply_outcome expression -> outcome again.
    let down = include_str!("../migrations/0007_expression_nodes.down.sql");
    sqlx::query(sqlx::AssertSqlSafe(function_block(down)))
        .execute(pool)
        .await
        .expect("recreate down function");
    let reverted = sqlx::query("SELECT rre.revert_canvas_0007($1)")
        .bind(&migrated["anonymous"])
        .fetch_one(pool)
        .await
        .expect("run revert")
        .get::<Value, _>(0);
    let rev_kinds: Vec<&str> = reverted["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .map(|n| n["kind"].as_str().unwrap())
        .collect();
    assert!(rev_kinds.contains(&"outcome"), "reverse restores outcomes");
    assert!(!rev_kinds.contains(&"start"));
    assert!(!rev_kinds.contains(&"end"));
}
