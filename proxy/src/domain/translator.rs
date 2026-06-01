//! Canvas `CanvasGraph` -> zen JDM `DecisionContent`. Pure. See spec
//! `docs/expression-nodes-spec.md` §4 for the exact mapping.
//!
//! - **Root = the `start` node.** Wire `input -> entry(successor_of_start)`; the
//!   start node emits no JDM node, its single edge's target is the real entry.
//! - `Decision <id>` -> CustomNode `<id>__proc` + SwitchNode `<id>__switch`
//!   + internal edge `<id>__proc -> <id>__switch` (UNCHANGED).
//! - `Expression <id>` -> ExpressionNode `<id>__expr` emitting a trace marker
//!   `{ "exprNode": "'<id>'" }`; connects forward to its single successor.
//! - `End <id>` -> zen `OutputNode` `<id>__out` (terminal).
//! - Edge wiring by source kind: Decision source -> `<src>__switch` +
//!   `source_handle = "<src>:yes|no"`; Start/Expression source -> the source's
//!   exit JDM id (`input` is handled by the root wiring; expression exits at
//!   `<src>__expr`) with `source_handle = None`.

use std::sync::Arc;

use zen_engine::model::{
    DecisionContent, DecisionEdge, DecisionNode, DecisionNodeKind, Expression,
    ExpressionNodeContent, InputNodeContent, OutputNodeContent, SwitchNodeContent, SwitchStatement,
    SwitchStatementHitPolicy, TransformAttributes,
};

use crate::domain::graph::{CanvasGraph, Node};

/// Entry JDM node id for a canvas node: decisions enter at `__proc`, expressions
/// at `__expr`, ends at `__out`. Start nodes emit no JDM node — they should never
/// be the target of an edge, so they map to their own id as a defensive fallback.
fn entry_id(node: &Node) -> String {
    match node {
        Node::Decision { id, .. } => format!("{id}__proc"),
        Node::Expression { id, .. } => format!("{id}__expr"),
        Node::End { id, .. } => format!("{id}__out"),
        Node::Start { id, .. } => id.clone(),
    }
}

/// Exit JDM node id for a non-decision source: an expression exits at `<id>__expr`
/// (a start node's outgoing edge is replaced by the root wiring, never reached
/// here). Decisions are handled separately (they exit via their SwitchNode).
fn exit_id(node: &Node) -> String {
    match node {
        Node::Expression { id, .. } => format!("{id}__expr"),
        // Start/Decision/End are not valid non-decision exit sources reached here.
        other => other.id().to_string(),
    }
}

/// Translate one canvas into a JDM `DecisionContent`. Pure function.
pub fn to_decision_content(canvas: &CanvasGraph) -> DecisionContent {
    let mut nodes: Vec<Arc<DecisionNode>> = Vec::new();
    let mut edges: Vec<Arc<DecisionEdge>> = Vec::new();

    // 1. Input node (seeds the evaluation context to the entry node).
    nodes.push(Arc::new(DecisionNode {
        id: Arc::from("input"),
        name: Arc::from("input"),
        kind: DecisionNodeKind::InputNode {
            content: InputNodeContent { schema: None },
        },
    }));

    // 2. Per-node translation. Start nodes emit NO JDM node.
    for node in &canvas.nodes {
        match node {
            Node::Start { .. } => {
                // No JDM node: the start's outgoing edge becomes the input wiring.
            }
            Node::Decision { id, processor, .. } => {
                let proc_id = format!("{id}__proc");
                let switch_id = format!("{id}__switch");

                // CustomNode (processor). The processor's `type`/kind is used
                // DIRECTLY as the JDM CustomNode kind (== registry key); the
                // flat config map is passed through unchanged.
                nodes.push(Arc::new(DecisionNode {
                    id: Arc::from(proc_id.as_str()),
                    name: Arc::from(id.as_str()),
                    kind: DecisionNodeKind::CustomNode {
                        content: zen_engine::model::CustomNodeContent {
                            kind: Arc::from(processor.kind.as_str()),
                            config: Arc::new(processor.config.clone()),
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
            Node::Expression { id, .. } => {
                let expr_id = format!("{id}__expr");

                // ExpressionNode emitting a trace marker { exprNode: '<id>' } so
                // the evaluator can recover the matched expression nodes (in order)
                // from the trace. The actual action is applied by the forwarder.
                let expressions = vec![Expression {
                    id: Arc::from("x"),
                    key: Arc::from("exprNode"),
                    value: Arc::from(format!("'{id}'").as_str()),
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
            }
            Node::End { id, .. } => {
                let out_id = format!("{id}__out");

                // OutputNode (terminal).
                nodes.push(Arc::new(DecisionNode {
                    id: Arc::from(out_id.as_str()),
                    name: Arc::from(format!("{id}-out").as_str()),
                    kind: DecisionNodeKind::OutputNode {
                        content: OutputNodeContent { schema: None },
                    },
                }));
            }
        }
    }

    // 3. Canvas edges. The source JDM node depends on the source's KIND:
    //    - Decision source -> from `<src>__switch` with the matching switch handle.
    //    - Start source     -> handled by the root wiring (step 4); skipped here.
    //    - Expression source -> from `<src>__expr`, no source_handle.
    for edge in &canvas.edges {
        let Some(target) = canvas.nodes.iter().find(|n| n.id() == edge.target_node_id) else {
            // Dangling edge target — skip (backend validation should prevent this).
            continue;
        };
        let Some(source) = canvas.nodes.iter().find(|n| n.id() == edge.source_node_id) else {
            continue;
        };

        match source {
            Node::Decision { .. } => {
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
            Node::Expression { .. } => {
                edges.push(Arc::new(DecisionEdge {
                    id: Arc::from(edge.id.as_str()),
                    source_id: Arc::from(exit_id(source).as_str()),
                    target_id: Arc::from(entry_id(target).as_str()),
                    source_handle: None,
                }));
            }
            // Start -> root wiring (step 4). End never originates an edge.
            Node::Start { .. } | Node::End { .. } => {}
        }
    }

    // 4. Input -> the start node's successor entry (the real entry node).
    if let Some(entry) = start_successor(canvas) {
        edges.push(Arc::new(DecisionEdge {
            id: Arc::from("input__e"),
            source_id: Arc::from("input"),
            target_id: Arc::from(entry_id(entry).as_str()),
            source_handle: None,
        }));
    }

    DecisionContent {
        nodes,
        edges,
        compiled_cache: None,
    }
}

/// Resolve the canvas entry node: the target of the unique `start` node's single
/// outgoing edge. Falls back to the legacy root heuristic (explicit `root_node_id`
/// then first no-incoming-edge node) when no start node exists, so non-migrated
/// canvases still translate.
fn start_successor(canvas: &CanvasGraph) -> Option<&Node> {
    if let Some(start) = canvas.nodes.iter().find(|n| n.is_start()) {
        let start_id = start.id();
        if let Some(edge) = canvas.edges.iter().find(|e| e.source_node_id == start_id) {
            return canvas.nodes.iter().find(|n| n.id() == edge.target_node_id);
        }
        return None;
    }

    // Legacy fallback: explicit root, else first node with no incoming edge.
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
