use rre_proxy::domain::graph::CanvasGraph;
use rre_proxy::domain::translator::to_decision_content;
use zen_engine::model::DecisionContent;

/// The worked-example anonymous canvas from the contract (§6). Built from a raw
/// JSON string (not the `json!` macro) so rustfmt leaves the literal untouched.
const ANONYMOUS_CANVAS_JSON: &str = r#"{
  "root_node_id": "n_meta",
  "nodes": [
    { "kind": "decision", "id": "n_meta",
      "processor": { "type": "meta_tags", "tag_name": "paywall", "operator": "contains", "value": "true" },
      "position": { "x": 80.0, "y": 200.0 } },
    { "kind": "decision", "id": "n_dev",
      "processor": { "type": "device_type", "operator": "equals", "value": "mobile" },
      "position": { "x": 360.0, "y": 120.0 } },
    { "kind": "outcome", "id": "n_regwall", "outcome_id": "11111111-1111-1111-1111-111111111111",
      "position": { "x": 640.0, "y": 60.0 } },
    { "kind": "outcome", "id": "n_paywall", "outcome_id": "22222222-2222-2222-2222-222222222222",
      "position": { "x": 640.0, "y": 200.0 } },
    { "kind": "outcome", "id": "n_content", "outcome_id": "33333333-3333-3333-3333-333333333333",
      "position": { "x": 360.0, "y": 320.0 } }
  ],
  "edges": [
    { "id": "e1", "source_node_id": "n_meta", "target_node_id": "n_dev",     "branch": "yes" },
    { "id": "e2", "source_node_id": "n_meta", "target_node_id": "n_content", "branch": "no"  },
    { "id": "e3", "source_node_id": "n_dev",  "target_node_id": "n_regwall", "branch": "yes" },
    { "id": "e4", "source_node_id": "n_dev",  "target_node_id": "n_paywall", "branch": "no"  }
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

    // input + (2 decision nodes * 2) + (3 outcome nodes * 2) = 1 + 4 + 6 = 11 nodes.
    assert_eq!(back.nodes.len(), 11);

    // An input node exists.
    assert!(back.nodes.iter().any(|n| n.id.as_ref() == "input"));
    // The decision processor + switch nodes exist.
    assert!(back.nodes.iter().any(|n| n.id.as_ref() == "n_meta__proc"));
    assert!(back.nodes.iter().any(|n| n.id.as_ref() == "n_meta__switch"));
    // Outcome expr + out nodes exist.
    assert!(back
        .nodes
        .iter()
        .any(|n| n.id.as_ref() == "n_content__expr"));
    assert!(back.nodes.iter().any(|n| n.id.as_ref() == "n_content__out"));
}

#[test]
fn edges_carry_branch_source_handles() {
    let canvas = anonymous_canvas();
    let dc = to_decision_content(&canvas);

    let e1 = dc.edges.iter().find(|e| e.id.as_ref() == "e1").unwrap();
    assert_eq!(e1.source_id.as_ref(), "n_meta__switch");
    assert_eq!(e1.target_id.as_ref(), "n_dev__proc");
    assert_eq!(e1.source_handle.as_deref(), Some("n_meta:yes"));

    let e2 = dc.edges.iter().find(|e| e.id.as_ref() == "e2").unwrap();
    assert_eq!(e2.target_id.as_ref(), "n_content__expr");
    assert_eq!(e2.source_handle.as_deref(), Some("n_meta:no"));

    // Input is wired to the root entry node.
    let input_edge = dc
        .edges
        .iter()
        .find(|e| e.id.as_ref() == "input__e")
        .unwrap();
    assert_eq!(input_edge.target_id.as_ref(), "n_meta__proc");
}

/// §3.8: `json_expression` is a generic open-config processor — the translator
/// emits a CustomNode whose `kind` == the registry key, with no graph.rs change.
#[test]
fn json_expression_translates_to_custom_node() {
    let canvas_json = r#"{
      "root_node_id": "n_json",
      "nodes": [
        { "kind": "decision", "id": "n_json",
          "processor": { "type": "json_expression", "json_path": "$.type", "operator": "equals", "value": "premium" },
          "position": { "x": 0.0, "y": 0.0 } },
        { "kind": "outcome", "id": "n_out", "outcome_id": "22222222-2222-2222-2222-222222222222",
          "position": { "x": 200.0, "y": 0.0 } }
      ],
      "edges": [
        { "id": "e1", "source_node_id": "n_json", "target_node_id": "n_out", "branch": "yes" }
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
