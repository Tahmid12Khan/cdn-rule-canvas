use rre_proxy::domain::graph::CanvasGraph;
use rre_proxy::domain::translator::to_decision_content;
use zen_engine::model::DecisionContent;

/// The worked-example anonymous canvas in the new start->decision->expression->end
/// shape. Built from a raw JSON string (not the `json!` macro) so rustfmt leaves
/// the literal untouched.
const ANONYMOUS_CANVAS_JSON: &str = r#"{
  "root_node_id": "start",
  "nodes": [
    { "kind": "start", "id": "start", "position": { "x": -200.0, "y": 200.0 } },
    { "kind": "decision", "id": "n_meta",
      "processor": { "type": "meta_tags", "tag_name": "paywall", "operator": "contains", "value": "true" },
      "position": { "x": 80.0, "y": 200.0 } },
    { "kind": "decision", "id": "n_dev",
      "processor": { "type": "device_type", "operator": "equals", "value": "mobile" },
      "position": { "x": 360.0, "y": 120.0 } },
    { "kind": "expression", "id": "n_regwall",
      "action": { "type": "apply_outcome", "outcome_id": "11111111-1111-1111-1111-111111111111" },
      "position": { "x": 640.0, "y": 60.0 } },
    { "kind": "expression", "id": "n_paywall",
      "action": { "type": "apply_outcome", "outcome_id": "22222222-2222-2222-2222-222222222222" },
      "position": { "x": 640.0, "y": 200.0 } },
    { "kind": "expression", "id": "n_content",
      "action": { "type": "apply_outcome", "outcome_id": "33333333-3333-3333-3333-333333333333" },
      "position": { "x": 360.0, "y": 320.0 } },
    { "kind": "end", "id": "end", "position": { "x": 900.0, "y": 200.0 } }
  ],
  "edges": [
    { "id": "e0", "source_node_id": "start",     "target_node_id": "n_meta",    "branch": "yes" },
    { "id": "e1", "source_node_id": "n_meta",    "target_node_id": "n_dev",     "branch": "yes" },
    { "id": "e2", "source_node_id": "n_meta",    "target_node_id": "n_content", "branch": "no"  },
    { "id": "e3", "source_node_id": "n_dev",     "target_node_id": "n_regwall", "branch": "yes" },
    { "id": "e4", "source_node_id": "n_dev",     "target_node_id": "n_paywall", "branch": "no"  },
    { "id": "e5", "source_node_id": "n_regwall", "target_node_id": "end",       "branch": "yes" },
    { "id": "e6", "source_node_id": "n_paywall", "target_node_id": "end",       "branch": "yes" },
    { "id": "e7", "source_node_id": "n_content", "target_node_id": "end",       "branch": "yes" }
  ]
}"#;

fn anonymous_canvas() -> CanvasGraph {
    serde_json::from_str(ANONYMOUS_CANVAS_JSON).unwrap()
}

#[test]
fn translates_and_round_trips() {
    let canvas = anonymous_canvas();
    let dc = to_decision_content(&canvas);

    // Round-trip requirement: to_value then from_value must succeed.
    let as_value = serde_json::to_value(&dc).unwrap();
    let back: DecisionContent = serde_json::from_value(as_value).unwrap();

    // input + (2 decisions * 2) + (3 expressions * 1) + (1 end * 1)
    //   = 1 + 4 + 3 + 1 = 9 nodes. Start emits NO JDM node.
    assert_eq!(back.nodes.len(), 9);

    // An input node exists; the start node emits nothing.
    assert!(back.nodes.iter().any(|n| n.id.as_ref() == "input"));
    assert!(!back.nodes.iter().any(|n| n.id.as_ref() == "start"));

    // Decision proc + switch nodes exist.
    assert!(back.nodes.iter().any(|n| n.id.as_ref() == "n_meta__proc"));
    assert!(back.nodes.iter().any(|n| n.id.as_ref() == "n_meta__switch"));

    // Expression emits an `__expr` node, no `__out`.
    assert!(back
        .nodes
        .iter()
        .any(|n| n.id.as_ref() == "n_content__expr"));
    assert!(!back.nodes.iter().any(|n| n.id.as_ref() == "n_content__out"));

    // End emits an `__out` OutputNode.
    assert!(back.nodes.iter().any(|n| n.id.as_ref() == "end__out"));
}

#[test]
fn input_wires_to_start_successor() {
    let canvas = anonymous_canvas();
    let dc = to_decision_content(&canvas);

    // Input is wired to the start node's successor (the first decision's entry).
    let input_edge = dc
        .edges
        .iter()
        .find(|e| e.id.as_ref() == "input__e")
        .unwrap();
    assert_eq!(input_edge.source_id.as_ref(), "input");
    assert_eq!(input_edge.target_id.as_ref(), "n_meta__proc");
}

#[test]
fn decision_edges_carry_branch_source_handles() {
    let canvas = anonymous_canvas();
    let dc = to_decision_content(&canvas);

    // The start->n_meta edge (e0) is replaced by the input wiring; it must NOT be
    // re-emitted as a switch edge.
    assert!(dc.edges.iter().all(|e| e.id.as_ref() != "e0"));

    let e1 = dc.edges.iter().find(|e| e.id.as_ref() == "e1").unwrap();
    assert_eq!(e1.source_id.as_ref(), "n_meta__switch");
    assert_eq!(e1.target_id.as_ref(), "n_dev__proc");
    assert_eq!(e1.source_handle.as_deref(), Some("n_meta:yes"));

    let e2 = dc.edges.iter().find(|e| e.id.as_ref() == "e2").unwrap();
    assert_eq!(e2.target_id.as_ref(), "n_content__expr");
    assert_eq!(e2.source_handle.as_deref(), Some("n_meta:no"));
}

#[test]
fn expression_edges_exit_at_expr_no_handle() {
    let canvas = anonymous_canvas();
    let dc = to_decision_content(&canvas);

    // n_content -> end: source is the expression's `__expr` exit, no source_handle.
    let e7 = dc.edges.iter().find(|e| e.id.as_ref() == "e7").unwrap();
    assert_eq!(e7.source_id.as_ref(), "n_content__expr");
    assert_eq!(e7.target_id.as_ref(), "end__out");
    assert_eq!(e7.source_handle.as_deref(), None);
}

/// `json_expression` is a generic open-config processor — the translator emits a
/// CustomNode whose `kind` == the registry key, with no graph.rs change.
#[test]
fn json_expression_translates_to_custom_node() {
    let canvas_json = r#"{
      "root_node_id": "start",
      "nodes": [
        { "kind": "start", "id": "start", "position": { "x": -200.0, "y": 0.0 } },
        { "kind": "decision", "id": "n_json",
          "processor": { "type": "json_expression", "json_path": "$.type", "operator": "equals", "value": "premium" },
          "position": { "x": 0.0, "y": 0.0 } },
        { "kind": "expression", "id": "n_out",
          "action": { "type": "apply_outcome", "outcome_id": "22222222-2222-2222-2222-222222222222" },
          "position": { "x": 200.0, "y": 0.0 } },
        { "kind": "end", "id": "end", "position": { "x": 400.0, "y": 0.0 } }
      ],
      "edges": [
        { "id": "e0", "source_node_id": "start",  "target_node_id": "n_json", "branch": "yes" },
        { "id": "e1", "source_node_id": "n_json", "target_node_id": "n_out",  "branch": "yes" },
        { "id": "e2", "source_node_id": "n_out",  "target_node_id": "end",    "branch": "yes" }
      ]
    }"#;
    let canvas: CanvasGraph = serde_json::from_str(canvas_json).unwrap();
    let dc = to_decision_content(&canvas);

    // Find the CustomNode and assert its kind + config flow through verbatim.
    let proc = dc
        .nodes
        .iter()
        .find(|n| n.id.as_ref() == "n_json__proc")
        .expect("json_expression proc node");
    match &proc.kind {
        zen_engine::model::DecisionNodeKind::CustomNode { content } => {
            assert_eq!(content.kind.as_ref(), "json_expression");
            assert_eq!(content.config.get("json_path").unwrap(), "$.type");
            assert_eq!(content.config.get("operator").unwrap(), "equals");
            assert_eq!(content.config.get("value").unwrap(), "premium");
        }
        other => panic!("expected CustomNode, got {other:?}"),
    }

    // Round-trips through serde like any other node.
    let as_value = serde_json::to_value(&dc).unwrap();
    let _back: DecisionContent = serde_json::from_value(as_value).unwrap();
}
