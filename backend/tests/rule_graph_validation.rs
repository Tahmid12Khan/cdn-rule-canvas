//! Integration coverage for rule-graph validation (BACKEND CONTRACT §6 +
//! expression-node flow spec §3).
//!
//! These tests drive the public `rule_graph_service::validate` API and the
//! `RuleGraph` serde shape from outside the crate (no DB needed — the validator
//! is a pure function of the graph, the version's valid outcome-id set, and the
//! node-type manifest). A non-empty canvas is a `start → … → end` pipeline.

use std::collections::HashSet;

use rre_backend::{
    error::AppError,
    schemas::{
        node_type::{LoadedManifest, NodeManifest},
        rule_graph::{Branch, CanvasGraph, Edge, Node, Position, ProcessorConfig, RuleGraph},
    },
    services::rule_graph_service,
};
use serde_json::json;
use uuid::Uuid;

/// The committed backend manifest (ported decision kinds + expression kinds).
fn manifest() -> NodeManifest {
    let loaded =
        LoadedManifest::load("config/node_types.json").expect("load node manifest for tests");
    (*loaded.typed).clone()
}

/// Build a generic processor/action (`type` + flat field map) from a JSON object.
fn processor(value: serde_json::Value) -> ProcessorConfig {
    match value {
        serde_json::Value::Object(mut map) => {
            let r#type = match map.remove("type") {
                Some(serde_json::Value::String(s)) => s,
                other => panic!("processor needs a string `type`, got {other:?}"),
            };
            ProcessorConfig {
                r#type,
                fields: map,
            }
        }
        other => panic!("processor must be a JSON object, got {other:?}"),
    }
}

fn pos() -> Position {
    Position { x: 1.0, y: 2.0 }
}

fn start(id: &str) -> Node {
    Node::Start {
        id: id.to_string(),
        position: pos(),
    }
}

fn end(id: &str) -> Node {
    Node::End {
        id: id.to_string(),
        position: pos(),
    }
}

fn decision(id: &str) -> Node {
    Node::Decision {
        id: id.to_string(),
        processor: processor(
            json!({"type": "device_type", "operator": "equals", "value": "mobile"}),
        ),
        position: pos(),
    }
}

fn expression_apply(id: &str, outcome_id: Uuid) -> Node {
    Node::Expression {
        id: id.to_string(),
        action: processor(json!({"type": "apply_outcome", "outcome_id": outcome_id.to_string()})),
        custom_label: None,
        position: pos(),
    }
}

/// A `trim_json` expression node carrying an optional `custom_label`.
fn expression_trim_labeled(id: &str, custom_label: Option<&str>) -> Node {
    Node::Expression {
        id: id.to_string(),
        action: processor(json!({"type": "trim_json", "json_path": "$.body", "length": 0})),
        custom_label: custom_label.map(str::to_string),
        position: pos(),
    }
}

fn edge(id: &str, src: &str, tgt: &str, branch: Branch) -> Edge {
    Edge {
        id: id.to_string(),
        source_node_id: src.to_string(),
        target_node_id: tgt.to_string(),
        branch,
    }
}

fn anon(canvas: CanvasGraph) -> RuleGraph {
    RuleGraph { canvas }
}

fn ids(list: &[Uuid]) -> HashSet<Uuid> {
    list.iter().copied().collect()
}

fn details(res: Result<(), AppError>) -> Vec<rre_backend::error::ValidationDetail> {
    match res.expect_err("expected validation failure") {
        AppError::Validation { details } => details,
        other => panic!("expected Validation error, got {other:?}"),
    }
}

fn has_rule(ds: &[rre_backend::error::ValidationDetail], rule_id: &str) -> bool {
    ds.iter().any(|d| d.rule_id == rule_id)
}

// 1.
#[test]
fn empty_graph_passes() {
    assert!(rule_graph_service::validate(
        &RuleGraph::default(),
        &HashSet::new(),
        &HashSet::new(),
        &HashSet::new(),
        &manifest()
    )
    .is_ok());
}

// 2.
#[test]
fn complete_canvas_passes() {
    let oid = Uuid::new_v4();
    let canvas = CanvasGraph {
        nodes: vec![
            start("s"),
            decision("d"),
            expression_apply("a", oid),
            end("e"),
        ],
        edges: vec![
            edge("e0", "s", "d", Branch::Yes),
            edge("e1", "d", "a", Branch::Yes),
            edge("e2", "d", "e", Branch::No),
            edge("e3", "a", "e", Branch::Yes),
        ],
        root_node_id: Some("s".to_string()),
    };
    assert!(rule_graph_service::validate(
        &anon(canvas),
        &ids(&[oid]),
        &HashSet::new(),
        &HashSet::new(),
        &manifest()
    )
    .is_ok());
}

// 3.
#[test]
fn missing_edge_target_fails() {
    let canvas = CanvasGraph {
        nodes: vec![start("s"), decision("d"), end("e")],
        edges: vec![
            edge("e0", "s", "d", Branch::Yes),
            edge("e1", "d", "missing", Branch::Yes),
        ],
        root_node_id: None,
    };
    let ds = details(rule_graph_service::validate(
        &anon(canvas),
        &HashSet::new(),
        &HashSet::new(),
        &HashSet::new(),
        &manifest(),
    ));
    assert!(has_rule(&ds, "edge_endpoint_exists"));
}

// 4.
#[test]
fn missing_edge_source_fails() {
    let canvas = CanvasGraph {
        nodes: vec![start("s"), end("e")],
        edges: vec![
            edge("e0", "s", "e", Branch::Yes),
            edge("e1", "missing", "e", Branch::Yes),
        ],
        root_node_id: None,
    };
    let ds = details(rule_graph_service::validate(
        &anon(canvas),
        &HashSet::new(),
        &HashSet::new(),
        &HashSet::new(),
        &manifest(),
    ));
    assert!(has_rule(&ds, "edge_endpoint_exists"));
}

// 5.
#[test]
fn duplicate_branch_fails() {
    let canvas = CanvasGraph {
        nodes: vec![start("s"), decision("d"), end("a"), end("b")],
        edges: vec![
            edge("e0", "s", "d", Branch::Yes),
            edge("e1", "d", "a", Branch::No),
            edge("e2", "d", "b", Branch::No),
        ],
        root_node_id: None,
    };
    let ds = details(rule_graph_service::validate(
        &anon(canvas),
        &HashSet::new(),
        &HashSet::new(),
        &HashSet::new(),
        &manifest(),
    ));
    assert!(has_rule(&ds, "branch_unique"));
}

// 6.
#[test]
fn self_loop_cycle_fails() {
    let canvas = CanvasGraph {
        nodes: vec![start("s"), decision("d"), end("e")],
        edges: vec![
            edge("e0", "s", "d", Branch::Yes),
            edge("e1", "d", "d", Branch::Yes),
        ],
        root_node_id: None,
    };
    let ds = details(rule_graph_service::validate(
        &anon(canvas),
        &HashSet::new(),
        &HashSet::new(),
        &HashSet::new(),
        &manifest(),
    ));
    assert!(has_rule(&ds, "no_cycles"));
}

// 7.
#[test]
fn three_node_cycle_fails() {
    let canvas = CanvasGraph {
        nodes: vec![
            start("s"),
            decision("a"),
            decision("b"),
            decision("c"),
            end("e"),
        ],
        edges: vec![
            edge("e0", "s", "a", Branch::Yes),
            edge("e1", "a", "b", Branch::Yes),
            edge("e2", "b", "c", Branch::Yes),
            edge("e3", "c", "a", Branch::Yes),
        ],
        root_node_id: None,
    };
    let ds = details(rule_graph_service::validate(
        &anon(canvas),
        &HashSet::new(),
        &HashSet::new(),
        &HashSet::new(),
        &manifest(),
    ));
    assert!(has_rule(&ds, "no_cycles"));
}

// 8.
#[test]
fn end_with_outgoing_edge_fails() {
    let canvas = CanvasGraph {
        nodes: vec![start("s"), end("e"), decision("d")],
        edges: vec![
            edge("e0", "s", "e", Branch::Yes),
            edge("e1", "e", "d", Branch::Yes),
        ],
        root_node_id: None,
    };
    let ds = details(rule_graph_service::validate(
        &anon(canvas),
        &HashSet::new(),
        &HashSet::new(),
        &HashSet::new(),
        &manifest(),
    ));
    assert!(has_rule(&ds, "end_terminal"));
    assert!(has_rule(&ds, "edge_source_kind"));
}

// 9.
#[test]
fn dangling_apply_outcome_ref_fails() {
    let oid = Uuid::new_v4();
    let canvas = CanvasGraph {
        nodes: vec![start("s"), expression_apply("a", oid), end("e")],
        edges: vec![
            edge("e0", "s", "a", Branch::Yes),
            edge("e1", "a", "e", Branch::Yes),
        ],
        root_node_id: None,
    };
    let ds = details(rule_graph_service::validate(
        &anon(canvas),
        &HashSet::new(),
        &HashSet::new(),
        &HashSet::new(),
        &manifest(),
    ));
    assert!(has_rule(&ds, "apply_outcome_ref_exists"));
}

// 10.
#[test]
fn root_not_in_nodes_fails() {
    let canvas = CanvasGraph {
        nodes: vec![start("s"), end("e")],
        edges: vec![edge("e0", "s", "e", Branch::Yes)],
        root_node_id: Some("nope".to_string()),
    };
    let ds = details(rule_graph_service::validate(
        &anon(canvas),
        &HashSet::new(),
        &HashSet::new(),
        &HashSet::new(),
        &manifest(),
    ));
    assert!(has_rule(&ds, "root_in_nodes"));
    assert!(ds.iter().any(|d| d.loc == "rule_graph.canvas.root_node_id"));
}

// 11.
#[test]
fn loc_index_points_at_offending_edge() {
    let canvas = CanvasGraph {
        nodes: vec![start("s"), decision("d"), end("o")],
        edges: vec![
            edge("e0", "s", "d", Branch::Yes),
            edge("e1", "d", "o", Branch::Yes),
            edge("e2", "d", "ghost", Branch::No),
        ],
        root_node_id: None,
    };
    let ds = details(rule_graph_service::validate(
        &anon(canvas),
        &HashSet::new(),
        &HashSet::new(),
        &HashSet::new(),
        &manifest(),
    ));
    assert!(ds
        .iter()
        .any(|d| d.loc == "rule_graph.canvas.edges[2]" && d.rule_id == "edge_endpoint_exists"));
}

// 12. Error `loc`s point at the offending node's actual index within the
//     single canvas (not just the canvas prefix).
#[test]
fn errors_target_correct_node_index() {
    let canvas = CanvasGraph {
        nodes: vec![start("s1"), start("s2"), end("e")],
        edges: vec![
            edge("e0", "s1", "e", Branch::Yes),
            edge("e1", "s2", "e", Branch::Yes),
        ],
        root_node_id: None,
    };
    let ds = details(rule_graph_service::validate(
        &anon(canvas),
        &HashSet::new(),
        &HashSet::new(),
        &HashSet::new(),
        &manifest(),
    ));
    assert!(ds
        .iter()
        .any(|d| d.loc == "rule_graph.canvas.nodes[1]" && d.rule_id == "start_present"));
}

// 13. The canonical worked example (expression-node flow) must deserialize and
//     validate cleanly once its outcome ids are registered.
#[test]
fn worked_example_round_trips_and_validates() {
    let json = serde_json::json!({
      "canvas": {
        "root_node_id": "start",
        "nodes": [
          { "kind": "start", "id": "start", "position": { "x": 0, "y": 200 } },
          { "kind": "decision", "id": "n_meta",
            "processor": { "type": "meta_tags", "tag_name": "paywall", "operator": "contains", "value": "true" },
            "position": { "x": 80, "y": 200 } },
          { "kind": "decision", "id": "n_dev",
            "processor": { "type": "device_type", "operator": "equals", "value": "mobile" },
            "position": { "x": 360, "y": 120 } },
          { "kind": "expression", "id": "a_regwall",
            "action": { "type": "apply_outcome", "outcome_id": "11111111-1111-1111-1111-111111111111" },
            "position": { "x": 640, "y": 60 } },
          { "kind": "expression", "id": "a_paywall",
            "action": { "type": "apply_outcome", "outcome_id": "22222222-2222-2222-2222-222222222222" },
            "position": { "x": 640, "y": 200 } },
          { "kind": "expression", "id": "a_content",
            "action": { "type": "apply_outcome", "outcome_id": "33333333-3333-3333-3333-333333333333" },
            "position": { "x": 360, "y": 320 } },
          { "kind": "end", "id": "end", "position": { "x": 900, "y": 200 } }
        ],
        "edges": [
          { "id": "e_start", "source_node_id": "start", "target_node_id": "n_meta", "branch": "yes" },
          { "id": "e1", "source_node_id": "n_meta", "target_node_id": "n_dev",     "branch": "yes" },
          { "id": "e2", "source_node_id": "n_meta", "target_node_id": "a_content", "branch": "no"  },
          { "id": "e3", "source_node_id": "n_dev",  "target_node_id": "a_regwall", "branch": "yes" },
          { "id": "e4", "source_node_id": "n_dev",  "target_node_id": "a_paywall", "branch": "no"  },
          { "id": "e_end_regwall", "source_node_id": "a_regwall", "target_node_id": "end", "branch": "yes" },
          { "id": "e_end_paywall", "source_node_id": "a_paywall", "target_node_id": "end", "branch": "yes" },
          { "id": "e_end_content", "source_node_id": "a_content", "target_node_id": "end", "branch": "yes" }
        ]
      },
    });

    let graph: RuleGraph =
        serde_json::from_value(json.clone()).expect("deserialize worked example");

    // Serde round-trip identity.
    let reparsed: RuleGraph =
        serde_json::from_value(serde_json::to_value(&graph).unwrap()).unwrap();
    assert_eq!(graph, reparsed);

    let valid = ids(&[
        Uuid::parse_str("11111111-1111-1111-1111-111111111111").unwrap(),
        Uuid::parse_str("22222222-2222-2222-2222-222222222222").unwrap(),
        Uuid::parse_str("33333333-3333-3333-3333-333333333333").unwrap(),
    ]);
    assert!(rule_graph_service::validate(
        &graph,
        &valid,
        &HashSet::new(),
        &HashSet::new(),
        &manifest()
    )
    .is_ok());
}

// 14. The same worked example, but with the outcome ids NOT registered, fails
//     apply_outcome_ref_exists for each apply_outcome node.
#[test]
fn worked_example_with_unknown_outcomes_fails() {
    let json = serde_json::json!({
      "canvas": {
        "root_node_id": "start",
        "nodes": [
          { "kind": "start", "id": "start", "position": { "x": 0, "y": 200 } },
          { "kind": "decision", "id": "n_meta",
            "processor": { "type": "meta_tags", "tag_name": "paywall", "operator": "exists" },
            "position": { "x": 80, "y": 200 } },
          { "kind": "expression", "id": "a_content",
            "action": { "type": "apply_outcome", "outcome_id": "33333333-3333-3333-3333-333333333333" },
            "position": { "x": 360, "y": 320 } },
          { "kind": "end", "id": "end", "position": { "x": 600, "y": 200 } }
        ],
        "edges": [
          { "id": "e_start", "source_node_id": "start", "target_node_id": "n_meta", "branch": "yes" },
          { "id": "e2", "source_node_id": "n_meta", "target_node_id": "a_content", "branch": "no" },
          { "id": "e1", "source_node_id": "n_meta", "target_node_id": "end", "branch": "yes" },
          { "id": "e_end_content", "source_node_id": "a_content", "target_node_id": "end", "branch": "yes" }
        ]
      },
    });
    let graph: RuleGraph = serde_json::from_value(json).unwrap();
    let ds = details(rule_graph_service::validate(
        &graph,
        &HashSet::new(),
        &HashSet::new(),
        &HashSet::new(),
        &manifest(),
    ));
    assert!(has_rule(&ds, "apply_outcome_ref_exists"));
}

// 15. meta_tags with `exists` (no value) deserializes and validates.
#[test]
fn meta_tags_exists_operator_valid() {
    let canvas = CanvasGraph {
        nodes: vec![
            start("s"),
            Node::Decision {
                id: "m".to_string(),
                processor: processor(
                    json!({"type": "meta_tags", "tag_name": "robots", "operator": "exists"}),
                ),
                position: pos(),
            },
            end("e"),
        ],
        edges: vec![
            edge("e0", "s", "m", Branch::Yes),
            edge("e1", "m", "e", Branch::Yes),
        ],
        root_node_id: Some("s".to_string()),
    };
    assert!(rule_graph_service::validate(
        &anon(canvas),
        &HashSet::new(),
        &HashSet::new(),
        &HashSet::new(),
        &manifest()
    )
    .is_ok());
}

// 16. processor_kind_known: an unknown processor type fails.
#[test]
fn unknown_processor_kind_fails() {
    let canvas = CanvasGraph {
        nodes: vec![Node::Decision {
            id: "x".to_string(),
            processor: processor(json!({"type": "made_up", "foo": 1})),
            position: pos(),
        }],
        edges: vec![],
        root_node_id: None,
    };
    let ds = details(rule_graph_service::validate(
        &anon(canvas),
        &HashSet::new(),
        &HashSet::new(),
        &HashSet::new(),
        &manifest(),
    ));
    assert!(has_rule(&ds, "processor_kind_known"));
}

// 17. processor_field_option: a select value outside its options fails.
#[test]
fn select_value_outside_options_fails() {
    let canvas = CanvasGraph {
        nodes: vec![Node::Decision {
            id: "d".to_string(),
            processor: processor(
                json!({"type": "device_type", "operator": "equals", "value": "smartwatch"}),
            ),
            position: pos(),
        }],
        edges: vec![],
        root_node_id: None,
    };
    let ds = details(rule_graph_service::validate(
        &anon(canvas),
        &HashSet::new(),
        &HashSet::new(),
        &HashSet::new(),
        &manifest(),
    ));
    assert!(has_rule(&ds, "processor_field_option"));
}

// 18. processor_field_required: meta_tags `value` required when operator != exists.
#[test]
fn meta_tags_value_required_unless_exists() {
    let canvas = CanvasGraph {
        nodes: vec![Node::Decision {
            id: "m".to_string(),
            processor: processor(
                json!({"type": "meta_tags", "tag_name": "paywall", "operator": "contains"}),
            ),
            position: pos(),
        }],
        edges: vec![],
        root_node_id: None,
    };
    let ds = details(rule_graph_service::validate(
        &anon(canvas),
        &HashSet::new(),
        &HashSet::new(),
        &HashSet::new(),
        &manifest(),
    ));
    assert!(has_rule(&ds, "processor_field_required"));
}

// 19. Unknown extra fields on the processor are ignored (forward-compatible).
#[test]
fn unknown_extra_processor_fields_ignored() {
    let canvas = CanvasGraph {
        nodes: vec![
            start("s"),
            Node::Decision {
                id: "d".to_string(),
                processor: processor(
                    json!({"type": "device_type", "operator": "equals", "value": "mobile", "future": true}),
                ),
                position: pos(),
            },
            end("e"),
        ],
        edges: vec![
            edge("e0", "s", "d", Branch::Yes),
            edge("e1", "d", "e", Branch::Yes),
        ],
        root_node_id: Some("s".to_string()),
    };
    assert!(rule_graph_service::validate(
        &anon(canvas),
        &HashSet::new(),
        &HashSet::new(),
        &HashSet::new(),
        &manifest()
    )
    .is_ok());
}

// 20. A JSON action pipeline (trim_json -> add_attribute) validates.
#[test]
fn json_action_pipeline_validates() {
    let canvas = CanvasGraph {
        nodes: vec![
            start("s"),
            Node::Decision {
                id: "d".to_string(),
                processor: processor(
                    json!({"type": "json_expression", "json_path": "$.api", "operator": "equals", "value": "demo-article"}),
                ),
                position: pos(),
            },
            Node::Expression {
                id: "t".to_string(),
                action: processor(json!({"type": "trim_json", "json_path": "$.body", "length": 0})),
                custom_label: None,
                position: pos(),
            },
            Node::Expression {
                id: "a".to_string(),
                action: processor(
                    json!({"type": "add_attribute", "json_path": "$.paywall_show", "value": "<html>paywall_showed</html>"}),
                ),
                custom_label: None,
                position: pos(),
            },
            end("e"),
        ],
        edges: vec![
            edge("e0", "s", "d", Branch::Yes),
            edge("e1", "d", "t", Branch::Yes),
            edge("e2", "d", "e", Branch::No),
            edge("e3", "t", "a", Branch::Yes),
            edge("e4", "a", "e", Branch::Yes),
        ],
        root_node_id: Some("s".to_string()),
    };
    assert!(rule_graph_service::validate(
        &anon(canvas),
        &HashSet::new(),
        &HashSet::new(),
        &HashSet::new(),
        &manifest()
    )
    .is_ok());
}

// 21. §6 empty-canvas normalization: an all-empty RuleGraph → start + end +
//     e_start_end on the canvas; root_node_id == "start"; normalized graph
//     passes validation.
#[test]
fn empty_canvas_normalizes_to_start_end() {
    let mut graph = RuleGraph::default();
    rule_graph_service::normalize(&mut graph);

    let canvas = &graph.canvas;
    assert_eq!(canvas.nodes.len(), 2, "expected 2 nodes");
    assert_eq!(canvas.edges.len(), 1, "expected 1 edge");
    assert_eq!(
        canvas.root_node_id.as_deref(),
        Some("start"),
        "root_node_id must be 'start'"
    );

    let start_node = canvas.nodes.iter().find(|n| n.id() == "start");
    let end_node = canvas.nodes.iter().find(|n| n.id() == "end");
    assert!(
        start_node.map(|n| n.is_start()).unwrap_or(false),
        "missing start node"
    );
    assert!(
        end_node.map(|n| n.is_end()).unwrap_or(false),
        "missing end node"
    );

    let e = &canvas.edges[0];
    assert_eq!(e.id, "e_start_end", "edge id");
    assert_eq!(e.source_node_id, "start", "edge source");
    assert_eq!(e.target_node_id, "end", "edge target");
    assert_eq!(e.branch, Branch::Yes, "edge branch");

    // Normalized graph must pass end-to-end validation.
    assert!(
        rule_graph_service::validate(
            &graph,
            &HashSet::new(),
            &HashSet::new(),
            &HashSet::new(),
            &manifest()
        )
        .is_ok(),
        "normalized graph must pass validation"
    );
}

// 22. §6 empty-canvas normalization: a non-empty canvas is left untouched.
#[test]
fn non_empty_canvas_not_touched_by_normalize() {
    let mut graph = RuleGraph {
        canvas: CanvasGraph {
            nodes: vec![
                Node::Start {
                    id: "s".to_string(),
                    position: Position { x: 0.0, y: 0.0 },
                },
                Node::End {
                    id: "e".to_string(),
                    position: Position { x: 1.0, y: 0.0 },
                },
            ],
            edges: vec![Edge {
                id: "custom_edge".to_string(),
                source_node_id: "s".to_string(),
                target_node_id: "e".to_string(),
                branch: Branch::Yes,
            }],
            root_node_id: Some("s".to_string()),
        },
    };
    rule_graph_service::normalize(&mut graph);

    // The canvas was non-empty: its edge must remain unchanged.
    assert_eq!(graph.canvas.edges[0].id, "custom_edge");
}

/// A `start → expression(trim_json, custom_label) → end` canvas; the expression's
/// `custom_label` is the value under test.
fn canvas_with_custom_label(custom_label: Option<&str>) -> CanvasGraph {
    CanvasGraph {
        nodes: vec![
            start("s"),
            expression_trim_labeled("t", custom_label),
            end("e"),
        ],
        edges: vec![
            edge("e0", "s", "t", Branch::Yes),
            edge("e1", "t", "e", Branch::Yes),
        ],
        root_node_id: Some("s".to_string()),
    }
}

// 23. v2.3 expression_custom_label_invalid: a valid snake_case custom_label passes.
#[test]
fn custom_label_snake_case_valid() {
    let canvas = canvas_with_custom_label(Some("my_custom_name_2"));
    assert!(rule_graph_service::validate(
        &anon(canvas),
        &HashSet::new(),
        &HashSet::new(),
        &HashSet::new(),
        &manifest()
    )
    .is_ok());
}

// 24. v2.3 expression_custom_label_invalid: empty string and absent both pass.
#[test]
fn custom_label_empty_and_absent_valid() {
    let empty = canvas_with_custom_label(Some(""));
    assert!(rule_graph_service::validate(
        &anon(empty),
        &HashSet::new(),
        &HashSet::new(),
        &HashSet::new(),
        &manifest()
    )
    .is_ok());
    let absent = canvas_with_custom_label(None);
    assert!(rule_graph_service::validate(
        &anon(absent),
        &HashSet::new(),
        &HashSet::new(),
        &HashSet::new(),
        &manifest()
    )
    .is_ok());
}

// 25. v2.3 expression_custom_label_invalid: uppercase, spaces, and
//     leading/trailing/double underscore each fail with the stable rule_id.
#[test]
fn custom_label_invalid_forms_fail() {
    for bad in ["MyName", "my name", "_lead", "trail_", "double__under"] {
        let canvas = canvas_with_custom_label(Some(bad));
        let ds = details(rule_graph_service::validate(
            &anon(canvas),
            &HashSet::new(),
            &HashSet::new(),
            &HashSet::new(),
            &manifest(),
        ));
        assert!(
            has_rule(&ds, "expression_custom_label_invalid"),
            "custom_label {bad:?} should fail expression_custom_label_invalid"
        );
        assert!(
            ds.iter().any(|d| d.loc == "canvas.nodes[t].custom_label"),
            "custom_label {bad:?} should report loc canvas.nodes[t].custom_label"
        );
    }
}
