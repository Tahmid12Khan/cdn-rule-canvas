//! Canvas `CanvasGraph` -> zen JDM `DecisionContent`. Pure. See BACKEND CONTRACT
//! §8.2 for the exact mapping.
//!
//! - Decision node `<id>` -> CustomNode `<id>__proc` + SwitchNode `<id>__switch`
//!   + internal edge `<id>__proc -> <id>__switch`.
//! - Outcome node `<id>` -> ExpressionNode `<id>__expr` (emits `outcomeId`) +
//!   OutputNode `<id>__out` + internal edge.
//! - Canvas edge (decision->X, branch) -> DecisionEdge from `<src>__switch` with
//!   `source_handle = "<src>:yes" | "<src>:no"` to the target's entry node.
//! - One InputNode `input` -> root node entry seeds the context.

use std::sync::Arc;

use zen_engine::model::{
    DecisionContent, DecisionEdge, DecisionNode, DecisionNodeKind, Expression,
    ExpressionNodeContent, InputNodeContent, OutputNodeContent, SwitchNodeContent, SwitchStatement,
    SwitchStatementHitPolicy, TransformAttributes,
};

use crate::domain::graph::{CanvasGraph, Node};

/// Entry JDM node id for a canvas node: decisions enter at `__proc`, outcomes at `__expr`.
fn entry_id(node: &Node) -> String {
    match node {
        Node::Decision { id, .. } => format!("{id}__proc"),
        Node::Outcome { id, .. } => format!("{id}__expr"),
    }
}

/// Translate one canvas into a JDM `DecisionContent`. Pure function.
pub fn to_decision_content(canvas: &CanvasGraph) -> DecisionContent {
    let mut nodes: Vec<Arc<DecisionNode>> = Vec::new();
    let mut edges: Vec<Arc<DecisionEdge>> = Vec::new();

    // 1. Input node (seeds the evaluation context to the root).
    nodes.push(Arc::new(DecisionNode {
        id: Arc::from("input"),
        name: Arc::from("input"),
        kind: DecisionNodeKind::InputNode {
            content: InputNodeContent { schema: None },
        },
    }));

    // 2. Per-node translation.
    for node in &canvas.nodes {
        match node {
            Node::Decision { id, processor, .. } => {
                let proc_id = format!("{id}__proc");
                let switch_id = format!("{id}__switch");

                // CustomNode (processor).
                nodes.push(Arc::new(DecisionNode {
                    id: Arc::from(proc_id.as_str()),
                    name: Arc::from(id.as_str()),
                    kind: DecisionNodeKind::CustomNode {
                        content: zen_engine::model::CustomNodeContent {
                            kind: Arc::from(processor.kind_key()),
                            config: Arc::new(processor.to_config_value()),
                        },
                    },
                }));

                // SwitchNode routing on the processor's `branch` output. zen
                // switch conditions reference input fields directly (e.g.
                // `branch == 'yes'`), NOT JSONPath (`$.branch`).
                let statements = vec![
                    SwitchStatement {
                        id: Arc::from(format!("{id}:yes").as_str()),
                        condition: Arc::from("branch == 'yes'"),
                    },
                    SwitchStatement {
                        id: Arc::from(format!("{id}:no").as_str()),
                        condition: Arc::from("branch == 'no'"),
                    },
                ];
                nodes.push(Arc::new(DecisionNode {
                    id: Arc::from(switch_id.as_str()),
                    name: Arc::from(format!("{id}-switch").as_str()),
                    kind: DecisionNodeKind::SwitchNode {
                        content: SwitchNodeContent {
                            hit_policy: SwitchStatementHitPolicy::First,
                            statements: Arc::new(statements),
                        },
                    },
                }));

                // Internal edge: proc -> switch.
                edges.push(Arc::new(DecisionEdge {
                    id: Arc::from(format!("{id}__e").as_str()),
                    source_id: Arc::from(proc_id.as_str()),
                    target_id: Arc::from(switch_id.as_str()),
                    source_handle: None,
                }));
            }
            Node::Outcome { id, outcome_id, .. } => {
                let expr_id = format!("{id}__expr");
                let out_id = format!("{id}__out");

                // ExpressionNode emitting { outcomeId: '<uuid>' }.
                let expressions = vec![Expression {
                    id: Arc::from("x"),
                    key: Arc::from("outcomeId"),
                    value: Arc::from(format!("'{outcome_id}'").as_str()),
                }];
                nodes.push(Arc::new(DecisionNode {
                    id: Arc::from(expr_id.as_str()),
                    name: Arc::from(id.as_str()),
                    kind: DecisionNodeKind::ExpressionNode {
                        content: ExpressionNodeContent {
                            expressions: Arc::new(expressions),
                            transform_attributes: TransformAttributes::default(),
                        },
                    },
                }));

                // OutputNode (terminal).
                nodes.push(Arc::new(DecisionNode {
                    id: Arc::from(out_id.as_str()),
                    name: Arc::from(format!("{id}-out").as_str()),
                    kind: DecisionNodeKind::OutputNode {
                        content: OutputNodeContent { schema: None },
                    },
                }));

                // Internal edge: expr -> out.
                edges.push(Arc::new(DecisionEdge {
                    id: Arc::from(format!("{id}__oe").as_str()),
                    source_id: Arc::from(expr_id.as_str()),
                    target_id: Arc::from(out_id.as_str()),
                    source_handle: None,
                }));
            }
        }
    }

    // 3. Canvas edges (decision -> X). Source is the decision's SwitchNode; the
    //    source_handle MUST equal the matching SwitchStatement id.
    for edge in &canvas.edges {
        let Some(target) = canvas.nodes.iter().find(|n| n.id() == edge.target_node_id) else {
            // Dangling edge target — skip (backend validation should prevent this).
            continue;
        };
        let branch = match edge.branch {
            crate::domain::graph::Branch::Yes => "yes",
            crate::domain::graph::Branch::No => "no",
        };
        edges.push(Arc::new(DecisionEdge {
            id: Arc::from(edge.id.as_str()),
            source_id: Arc::from(format!("{}__switch", edge.source_node_id).as_str()),
            target_id: Arc::from(entry_id(target).as_str()),
            source_handle: Some(Arc::from(
                format!("{}:{}", edge.source_node_id, branch).as_str(),
            )),
        }));
    }

    // 4. Input -> root node entry.
    if let Some(root) = root_node(canvas) {
        edges.push(Arc::new(DecisionEdge {
            id: Arc::from("input__e"),
            source_id: Arc::from("input"),
            target_id: Arc::from(entry_id(root).as_str()),
            source_handle: None,
        }));
    }

    DecisionContent {
        nodes,
        edges,
        compiled_cache: None,
    }
}

/// Resolve the canvas root node: the explicit `root_node_id` if present,
/// otherwise the first node with no incoming canvas edge.
fn root_node(canvas: &CanvasGraph) -> Option<&Node> {
    if let Some(root_id) = &canvas.root_node_id {
        if let Some(n) = canvas.nodes.iter().find(|n| n.id() == root_id) {
            return Some(n);
        }
    }
    canvas
        .nodes
        .iter()
        .find(|n| !canvas.edges.iter().any(|e| e.target_node_id == n.id()))
}
