//! Coverage for the 0013 single-canvas collapse migration (BACKEND CONTRACT §6).
//!
//! `sqlx::migrate!` (via `common::setup`) already applies every migration,
//! including 0013, up to head before any test runs — so there are no
//! pre-0013-shaped rows left to observe by the time a test starts. This test
//! instead seeds a row shaped like the OLD three-key default directly (as
//! 0013's own `UPDATE` would have found it, had it existed at migration time),
//! then replays that exact `UPDATE` statement from `0013_single_canvas.up.sql`
//! and confirms it leaves `rule_graph->'canvas'` present and equal to the old
//! `anonymous` canvas, with the old keys gone.

mod common;

use serde_json::{json, Value};
use sqlx::Row;
use uuid::Uuid;

#[tokio::test]
async fn migration_0013_collapses_to_single_canvas() {
    let db = common::setup().await;
    let pool = &db.state.pool;

    let old_graph = json!({
        "anonymous": {
            "root_node_id": "start",
            "nodes": [
                { "kind": "start", "id": "start", "position": { "x": 0, "y": 0 } },
                { "kind": "end", "id": "end", "position": { "x": 100, "y": 0 } }
            ],
            "edges": [
                { "id": "e0", "source_node_id": "start", "target_node_id": "end", "branch": "yes" }
            ]
        },
        "registered": { "root_node_id": null, "nodes": [], "edges": [] },
        "customer":   { "root_node_id": null, "nodes": [], "edges": [] }
    });

    sqlx::query(
        "INSERT INTO rre.features (id, name, type, execution_order) VALUES ('mig-0013', 'm', 'html', 1)",
    )
    .execute(pool)
    .await
    .expect("seed feature");
    let vid = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO rre.versions (id, feature_id, version_number, status, rule_graph, created_by, last_updated_by) \
         VALUES ($1, 'mig-0013', 1, 'draft', $2, 't', 't')",
    )
    .bind(vid)
    .bind(&old_graph)
    .execute(pool)
    .await
    .expect("seed version with old three-canvas shape");

    // Replay 0013's own UPDATE, scoped to this row.
    sqlx::query(
        "UPDATE rre.versions SET rule_graph = jsonb_build_object('canvas', rule_graph->'anonymous') WHERE id = $1",
    )
    .bind(vid)
    .execute(pool)
    .await
    .expect("run 0013 UPDATE");

    let migrated: Value = sqlx::query("SELECT rule_graph FROM rre.versions WHERE id = $1")
        .bind(vid)
        .fetch_one(pool)
        .await
        .expect("read migrated graph")
        .get(0);

    let obj = migrated.as_object().expect("rule_graph is an object");
    assert_eq!(
        obj.keys().collect::<Vec<_>>(),
        vec!["canvas"],
        "only the 'canvas' key remains"
    );
    assert_eq!(
        migrated["canvas"], old_graph["anonymous"],
        "canvas carries over the old anonymous canvas verbatim"
    );
}
