//! Rule-graph validation (BACKEND CONTRACT §6).
//!
//! [`validate`] checks a [`RuleGraph`] against the seven stable rules below,
//! per canvas (anonymous / registered / customer). Every failure produces a
//! [`ValidationDetail`] whose `loc` is `rule_graph.<canvas>.<field>[idx]` and
//! whose `rule_id` is one of the STABLE identifiers in the table. Any failure
//! yields an [`AppError::Validation`] (HTTP 422).
//!
//! | `rule_id` | Rule |
//! |---|---|
//! | `edge_endpoint_exists` | every edge endpoint exists in `nodes` |
//! | `branch_unique` | a Decision node has at most one outgoing edge per branch |
//! | `no_cycles` | the graph is acyclic |
//! | `outcome_terminal` | Outcome nodes have zero outgoing edges |
//! | `outcome_ref_exists` | every Outcome node's `outcome_id` exists for the version |
//! | `root_in_nodes` | a set `root_node_id` exists in `nodes` |
//! | `outcome_branch_forbidden` | edges may only originate from Decision nodes |
//!
//! This module is DB-agnostic: the set of valid outcome ids for the version is
//! supplied by the caller (the version service reads `rre.outcomes`). That keeps
//! the validator a pure function, fully unit-testable without a database.

use std::collections::{HashMap, HashSet};

use uuid::Uuid;

use crate::{
    error::{AppError, AppResult, ValidationDetail},
    schemas::rule_graph::{Branch, CanvasGraph, Node, RuleGraph},
};

/// Validate a full [`RuleGraph`] across all three canvases.
///
/// `valid_outcome_ids` is the set of `rre.outcomes.id` values that belong to the
/// version being edited; it backs the `outcome_ref_exists` rule. Callers (the
/// version service) fetch this set before invoking validation.
///
/// Returns `Ok(())` when every canvas passes; otherwise an
/// [`AppError::Validation`] carrying one [`ValidationDetail`] per violation.
pub fn validate(graph: &RuleGraph, valid_outcome_ids: &HashSet<Uuid>) -> AppResult<()> {
    let mut details: Vec<ValidationDetail> = Vec::new();

    for (canvas_name, canvas) in graph.canvases() {
        validate_canvas(canvas_name, canvas, valid_outcome_ids, &mut details);
    }

    if details.is_empty() {
        Ok(())
    } else {
        Err(AppError::validation(details))
    }
}

/// Validate a single canvas, pushing any violations into `details`.
fn validate_canvas(
    canvas: &'static str,
    graph: &CanvasGraph,
    valid_outcome_ids: &HashSet<Uuid>,
    details: &mut Vec<ValidationDetail>,
) {
    // Index nodes by id; flag duplicate ids defensively (last write wins, but a
    // duplicate id makes endpoint resolution ambiguous so we surface it).
    let mut node_by_id: HashMap<&str, &Node> = HashMap::with_capacity(graph.nodes.len());
    for (idx, node) in graph.nodes.iter().enumerate() {
        if node_by_id.insert(node.id(), node).is_some() {
            details.push(ValidationDetail::new(
                format!("rule_graph.{canvas}.nodes[{idx}]"),
                format!("duplicate node id '{}'", node.id()),
                "edge_endpoint_exists",
            ));
        }
    }

    // root_in_nodes
    if let Some(root) = graph.root_node_id.as_deref() {
        if !node_by_id.contains_key(root) {
            details.push(ValidationDetail::new(
                format!("rule_graph.{canvas}.root_node_id"),
                format!("root_node_id '{root}' not found in nodes"),
                "root_in_nodes",
            ));
        }
    }

    // outcome_ref_exists
    for (idx, node) in graph.nodes.iter().enumerate() {
        if let Node::Outcome { outcome_id, .. } = node {
            if !valid_outcome_ids.contains(outcome_id) {
                details.push(ValidationDetail::new(
                    format!("rule_graph.{canvas}.nodes[{idx}]"),
                    format!("outcome_id '{outcome_id}' not found in outcomes for this version"),
                    "outcome_ref_exists",
                ));
            }
        }
    }

    // Per-source branch tracking for branch_unique, plus edge-level rules.
    // Key: (source_node_id, branch) -> already seen.
    let mut seen_branch: HashSet<(&str, Branch)> = HashSet::new();

    for (idx, edge) in graph.edges.iter().enumerate() {
        let loc = format!("rule_graph.{canvas}.edges[{idx}]");

        let source = node_by_id.get(edge.source_node_id.as_str());
        let target_exists = node_by_id.contains_key(edge.target_node_id.as_str());

        // edge_endpoint_exists (source)
        if source.is_none() {
            details.push(ValidationDetail::new(
                loc.clone(),
                format!("edge source '{}' not found in nodes", edge.source_node_id),
                "edge_endpoint_exists",
            ));
        }
        // edge_endpoint_exists (target)
        if !target_exists {
            details.push(ValidationDetail::new(
                loc.clone(),
                format!("edge target '{}' not found in nodes", edge.target_node_id),
                "edge_endpoint_exists",
            ));
        }

        // Source-origin rules only apply when the source node resolves.
        if let Some(src_node) = source {
            // outcome_terminal / outcome_branch_forbidden: outcome nodes may not
            // originate edges.
            if src_node.is_outcome() {
                details.push(ValidationDetail::new(
                    loc.clone(),
                    format!(
                        "outcome node '{}' must be terminal (no outgoing edges)",
                        edge.source_node_id
                    ),
                    "outcome_terminal",
                ));
                details.push(ValidationDetail::new(
                    loc.clone(),
                    format!(
                        "edge may only originate from a decision node, not outcome '{}'",
                        edge.source_node_id
                    ),
                    "outcome_branch_forbidden",
                ));
            } else {
                // branch_unique: a decision node has at most one edge per branch.
                if !seen_branch.insert((src_node.id(), edge.branch)) {
                    details.push(ValidationDetail::new(
                        loc.clone(),
                        format!(
                            "decision node '{}' has more than one '{}' branch edge",
                            src_node.id(),
                            branch_str(edge.branch)
                        ),
                        "branch_unique",
                    ));
                }
            }
        }
    }

    // no_cycles: DFS over the (decision-sourced) adjacency. We include every
    // edge whose endpoints both resolve; a cycle through any resolvable edges is
    // reported once.
    if has_cycle(graph, &node_by_id) {
        details.push(ValidationDetail::new(
            format!("rule_graph.{canvas}.edges"),
            "graph contains a cycle".to_string(),
            "no_cycles",
        ));
    }
}

/// Human label for a branch, used in messages.
fn branch_str(branch: Branch) -> &'static str {
    match branch {
        Branch::Yes => "yes",
        Branch::No => "no",
    }
}

/// Iterative DFS cycle detection over edges whose endpoints both exist.
fn has_cycle(graph: &CanvasGraph, node_by_id: &HashMap<&str, &Node>) -> bool {
    // Adjacency over resolvable edges only (unresolved endpoints are reported by
    // edge_endpoint_exists and excluded here so they don't mask a real cycle).
    let mut adj: HashMap<&str, Vec<&str>> = HashMap::new();
    for edge in &graph.edges {
        let s = edge.source_node_id.as_str();
        let t = edge.target_node_id.as_str();
        if node_by_id.contains_key(s) && node_by_id.contains_key(t) {
            adj.entry(s).or_default().push(t);
        }
    }

    // Three-colour DFS: White (unvisited), Gray (on stack), Black (done).
    #[derive(Clone, Copy, PartialEq)]
    enum Color {
        White,
        Gray,
        Black,
    }
    let mut color: HashMap<&str, Color> = node_by_id.keys().map(|&k| (k, Color::White)).collect();

    // Explicit stack of (node, child-cursor) to avoid recursion blowups.
    for &start in node_by_id.keys() {
        if color[start] != Color::White {
            continue;
        }
        let mut stack: Vec<(&str, usize)> = vec![(start, 0)];
        color.insert(start, Color::Gray);

        while let Some(&(node, cursor)) = stack.last() {
            let neighbours = adj.get(node).map(Vec::as_slice).unwrap_or(&[]);
            if cursor < neighbours.len() {
                // Advance this frame's cursor.
                if let Some(frame) = stack.last_mut() {
                    frame.1 += 1;
                }
                let next = neighbours[cursor];
                match color[next] {
                    Color::White => {
                        color.insert(next, Color::Gray);
                        stack.push((next, 0));
                    }
                    Color::Gray => return true, // back-edge => cycle
                    Color::Black => {}
                }
            } else {
                color.insert(node, Color::Black);
                stack.pop();
            }
        }
    }

    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::schemas::rule_graph::{
        DeviceOperator, DeviceValue, Edge, MetaTagsOperator, Position, ProcessorConfig,
    };

    fn pos() -> Position {
        Position { x: 0.0, y: 0.0 }
    }

    fn decision(id: &str) -> Node {
        Node::Decision {
            id: id.to_string(),
            processor: ProcessorConfig::DeviceType {
                operator: DeviceOperator::Equals,
                value: DeviceValue::Mobile,
            },
            position: pos(),
        }
    }

    fn outcome(id: &str, outcome_id: Uuid) -> Node {
        Node::Outcome {
            id: id.to_string(),
            outcome_id,
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

    /// Wrap a single canvas as the `anonymous` canvas of an otherwise-empty graph.
    fn graph_with_anonymous(canvas: CanvasGraph) -> RuleGraph {
        RuleGraph {
            anonymous: canvas,
            ..Default::default()
        }
    }

    /// Extract the validation details, asserting an error was produced.
    fn details_of(res: AppResult<()>) -> Vec<ValidationDetail> {
        match res.expect_err("expected validation error") {
            AppError::Validation { details } => details,
            other => panic!("expected Validation error, got {other:?}"),
        }
    }

    fn rule_ids(details: &[ValidationDetail]) -> Vec<&str> {
        details.iter().map(|d| d.rule_id.as_str()).collect()
    }

    // 1. Empty graph is valid.
    #[test]
    fn empty_graph_is_valid() {
        let res = validate(&RuleGraph::default(), &HashSet::new());
        assert!(res.is_ok());
    }

    // 2. A well-formed canvas passes.
    #[test]
    fn well_formed_canvas_is_valid() {
        let oid = Uuid::new_v4();
        let canvas = CanvasGraph {
            nodes: vec![decision("d1"), outcome("o1", oid), outcome("o2", oid)],
            edges: vec![
                edge("e1", "d1", "o1", Branch::Yes),
                edge("e2", "d1", "o2", Branch::No),
            ],
            root_node_id: Some("d1".to_string()),
        };
        let mut ids = HashSet::new();
        ids.insert(oid);
        let res = validate(&graph_with_anonymous(canvas), &ids);
        assert!(res.is_ok(), "expected ok, got {res:?}");
    }

    // 3. edge_endpoint_exists: missing target.
    #[test]
    fn edge_target_missing() {
        let canvas = CanvasGraph {
            nodes: vec![decision("d1")],
            edges: vec![edge("e1", "d1", "ghost", Branch::Yes)],
            root_node_id: None,
        };
        let details = details_of(validate(&graph_with_anonymous(canvas), &HashSet::new()));
        assert!(rule_ids(&details).contains(&"edge_endpoint_exists"));
        assert_eq!(details[0].loc, "rule_graph.anonymous.edges[0]");
    }

    // 4. edge_endpoint_exists: missing source.
    #[test]
    fn edge_source_missing() {
        let oid = Uuid::new_v4();
        let canvas = CanvasGraph {
            nodes: vec![outcome("o1", oid)],
            edges: vec![edge("e1", "ghost", "o1", Branch::Yes)],
            root_node_id: None,
        };
        let mut ids = HashSet::new();
        ids.insert(oid);
        let details = details_of(validate(&graph_with_anonymous(canvas), &ids));
        assert!(rule_ids(&details).contains(&"edge_endpoint_exists"));
    }

    // 5. branch_unique: two `yes` edges from one decision node.
    #[test]
    fn duplicate_yes_branch() {
        let oid = Uuid::new_v4();
        let canvas = CanvasGraph {
            nodes: vec![decision("d1"), outcome("o1", oid), outcome("o2", oid)],
            edges: vec![
                edge("e1", "d1", "o1", Branch::Yes),
                edge("e2", "d1", "o2", Branch::Yes),
            ],
            root_node_id: None,
        };
        let mut ids = HashSet::new();
        ids.insert(oid);
        let details = details_of(validate(&graph_with_anonymous(canvas), &ids));
        assert!(rule_ids(&details).contains(&"branch_unique"));
    }

    // 6. branch_unique allows one yes + one no from the same node.
    #[test]
    fn yes_and_no_from_same_node_ok() {
        let oid = Uuid::new_v4();
        let canvas = CanvasGraph {
            nodes: vec![decision("d1"), outcome("o1", oid), outcome("o2", oid)],
            edges: vec![
                edge("e1", "d1", "o1", Branch::Yes),
                edge("e2", "d1", "o2", Branch::No),
            ],
            root_node_id: None,
        };
        let mut ids = HashSet::new();
        ids.insert(oid);
        assert!(validate(&graph_with_anonymous(canvas), &ids).is_ok());
    }

    // 7. no_cycles: a self-loop is a cycle.
    #[test]
    fn self_loop_is_cycle() {
        let canvas = CanvasGraph {
            nodes: vec![decision("d1")],
            edges: vec![edge("e1", "d1", "d1", Branch::Yes)],
            root_node_id: None,
        };
        let details = details_of(validate(&graph_with_anonymous(canvas), &HashSet::new()));
        assert!(rule_ids(&details).contains(&"no_cycles"));
    }

    // 8. no_cycles: a multi-node cycle d1 -> d2 -> d1.
    #[test]
    fn multi_node_cycle() {
        let canvas = CanvasGraph {
            nodes: vec![decision("d1"), decision("d2")],
            edges: vec![
                edge("e1", "d1", "d2", Branch::Yes),
                edge("e2", "d2", "d1", Branch::Yes),
            ],
            root_node_id: None,
        };
        let details = details_of(validate(&graph_with_anonymous(canvas), &HashSet::new()));
        assert!(rule_ids(&details).contains(&"no_cycles"));
    }

    // 9. A diamond (shared target, no back-edge) is acyclic.
    #[test]
    fn diamond_is_acyclic() {
        let oid = Uuid::new_v4();
        let canvas = CanvasGraph {
            nodes: vec![
                decision("d1"),
                decision("d2"),
                decision("d3"),
                outcome("o1", oid),
            ],
            edges: vec![
                edge("e1", "d1", "d2", Branch::Yes),
                edge("e2", "d1", "d3", Branch::No),
                edge("e3", "d2", "o1", Branch::Yes),
                edge("e4", "d3", "o1", Branch::Yes),
            ],
            root_node_id: Some("d1".to_string()),
        };
        let mut ids = HashSet::new();
        ids.insert(oid);
        assert!(validate(&graph_with_anonymous(canvas), &ids).is_ok());
    }

    // 10. outcome_terminal + outcome_branch_forbidden: outcome node with an outgoing edge.
    #[test]
    fn outcome_node_with_outgoing_edge() {
        let oid = Uuid::new_v4();
        let canvas = CanvasGraph {
            nodes: vec![outcome("o1", oid), decision("d1")],
            edges: vec![edge("e1", "o1", "d1", Branch::Yes)],
            root_node_id: None,
        };
        let mut ids = HashSet::new();
        ids.insert(oid);
        let details = details_of(validate(&graph_with_anonymous(canvas), &ids));
        let ids_seen = rule_ids(&details);
        assert!(ids_seen.contains(&"outcome_terminal"));
        assert!(ids_seen.contains(&"outcome_branch_forbidden"));
    }

    // 11. outcome_ref_exists: outcome_id not in the version's outcomes.
    #[test]
    fn outcome_ref_missing() {
        let oid = Uuid::new_v4();
        let canvas = CanvasGraph {
            nodes: vec![outcome("o1", oid)],
            edges: vec![],
            root_node_id: None,
        };
        // empty valid-id set => the reference is dangling.
        let details = details_of(validate(&graph_with_anonymous(canvas), &HashSet::new()));
        assert!(rule_ids(&details).contains(&"outcome_ref_exists"));
    }

    // 12. root_in_nodes: root_node_id points at a non-existent node.
    #[test]
    fn root_not_in_nodes() {
        let canvas = CanvasGraph {
            nodes: vec![decision("d1")],
            edges: vec![],
            root_node_id: Some("ghost".to_string()),
        };
        let details = details_of(validate(&graph_with_anonymous(canvas), &HashSet::new()));
        assert!(rule_ids(&details).contains(&"root_in_nodes"));
        assert_eq!(details[0].loc, "rule_graph.anonymous.root_node_id");
    }

    // 13. Failures are reported on the correct canvas (registered, not anonymous).
    #[test]
    fn loc_reflects_canvas() {
        let canvas = CanvasGraph {
            nodes: vec![decision("d1")],
            edges: vec![edge("e1", "d1", "ghost", Branch::Yes)],
            root_node_id: None,
        };
        let graph = RuleGraph {
            registered: canvas,
            ..Default::default()
        };
        let details = details_of(validate(&graph, &HashSet::new()));
        assert!(details
            .iter()
            .any(|d| d.loc.starts_with("rule_graph.registered.")));
    }

    // 14. Multiple violations across canvases accumulate.
    #[test]
    fn violations_accumulate_across_canvases() {
        let bad = || CanvasGraph {
            nodes: vec![decision("d1")],
            edges: vec![edge("e1", "d1", "ghost", Branch::Yes)],
            root_node_id: Some("missing".to_string()),
        };
        let graph = RuleGraph {
            anonymous: bad(),
            registered: bad(),
            customer: CanvasGraph::default(),
        };
        let details = details_of(validate(&graph, &HashSet::new()));
        // Two canvases each contribute an endpoint error + a root error => >= 4.
        assert!(details.len() >= 4, "got {} details", details.len());
        assert!(rule_ids(&details).contains(&"edge_endpoint_exists"));
        assert!(rule_ids(&details).contains(&"root_in_nodes"));
    }

    // 15. A meta_tags decision node round-trips through validation.
    #[test]
    fn meta_tags_decision_valid() {
        let oid = Uuid::new_v4();
        let canvas = CanvasGraph {
            nodes: vec![
                Node::Decision {
                    id: "m1".to_string(),
                    processor: ProcessorConfig::MetaTags {
                        tag_name: "paywall".to_string(),
                        operator: MetaTagsOperator::Contains,
                        value: Some("true".to_string()),
                    },
                    position: pos(),
                },
                outcome("o1", oid),
            ],
            edges: vec![edge("e1", "m1", "o1", Branch::Yes)],
            root_node_id: Some("m1".to_string()),
        };
        let mut ids = HashSet::new();
        ids.insert(oid);
        assert!(validate(&graph_with_anonymous(canvas), &ids).is_ok());
    }
}
