//! Rule-graph validation (BACKEND CONTRACT §6).
//!
//! [`validate`] checks a [`RuleGraph`] against the stable rules below, per canvas
//! (anonymous / registered / customer). Every failure produces a
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
//! | `outcome_reachable` | every node reachable from the canvas root can reach an Outcome |
//! | `root_in_nodes` | a set `root_node_id` exists in `nodes` |
//! | `outcome_branch_forbidden` | edges may only originate from Decision nodes |
//! | `processor_kind_known` | a Decision node's processor `type` is a manifest `kind` |
//! | `processor_field_required` | each required field (incl. unsatisfied `required_unless`) is present and non-empty |
//! | `processor_field_option` | a `select` field's value is one of its `options[].value` |
//!
//! The structural rules are DB-agnostic: the set of valid outcome ids for the
//! version is supplied by the caller (the version service reads `rre.outcomes`).
//! The processor rules are manifest-driven: the typed processor enum is gone, so
//! `validate` takes a [`NodeManifest`] and checks each Decision node's processor
//! against the matched spec. That keeps the validator a pure function, fully
//! unit-testable without a database.

use std::collections::{HashMap, HashSet};

use serde_json::Value;
use uuid::Uuid;

use crate::{
    error::{AppError, AppResult, ValidationDetail},
    schemas::{
        node_type::{Control, Field, NodeManifest, NodeTypeSpec},
        rule_graph::{Branch, CanvasGraph, Node, ProcessorConfig, RuleGraph},
    },
};

/// Validate a full [`RuleGraph`] across all three canvases.
///
/// `valid_outcome_ids` is the set of `rre.outcomes.id` values that belong to the
/// version being edited; it backs the `outcome_ref_exists` rule. `manifest`
/// backs the processor rules (`processor_kind_known`, `processor_field_required`,
/// `processor_field_option`). Callers (the version service) fetch the outcome-id
/// set and supply the manifest from `AppState` before invoking validation.
///
/// Returns `Ok(())` when every canvas passes; otherwise an
/// [`AppError::Validation`] carrying one [`ValidationDetail`] per violation.
pub fn validate(
    graph: &RuleGraph,
    valid_outcome_ids: &HashSet<Uuid>,
    manifest: &NodeManifest,
) -> AppResult<()> {
    let mut details: Vec<ValidationDetail> = Vec::new();
    let spec_by_kind = manifest.index();

    for (canvas_name, canvas) in graph.canvases() {
        validate_canvas(
            canvas_name,
            canvas,
            valid_outcome_ids,
            &spec_by_kind,
            &mut details,
        );
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
    spec_by_kind: &HashMap<&str, &NodeTypeSpec>,
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

    // outcome_ref_exists + processor_* (manifest-driven, per Decision node).
    for (idx, node) in graph.nodes.iter().enumerate() {
        match node {
            Node::Outcome { outcome_id, .. } => {
                if !valid_outcome_ids.contains(outcome_id) {
                    details.push(ValidationDetail::new(
                        format!("rule_graph.{canvas}.nodes[{idx}]"),
                        format!("outcome_id '{outcome_id}' not found in outcomes for this version"),
                        "outcome_ref_exists",
                    ));
                }
            }
            Node::Decision { processor, .. } => {
                validate_processor(canvas, idx, processor, spec_by_kind, details);
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

    // outcome_reachable: every node reachable from the canvas root must be able
    // to reach an outcome (dead-ends are invalid; partial branches are OK).
    validate_outcome_reachable(canvas, graph, &node_by_id, details);
}

/// `outcome_reachable`: every node reachable from the canvas root must be able to
/// reach at least one Outcome node over resolvable edges.
///
/// Anchoring requires a defined start. The root is `root_node_id` when set, else
/// the UNIQUE node with no incoming (resolvable) edge. When neither yields a
/// single root — or the canvas is empty — the rule is SKIPPED (other rules cover
/// misconfiguration; an empty canvas stays valid).
fn validate_outcome_reachable(
    canvas: &'static str,
    graph: &CanvasGraph,
    node_by_id: &HashMap<&str, &Node>,
    details: &mut Vec<ValidationDetail>,
) {
    if graph.nodes.is_empty() {
        return;
    }

    // Resolvable adjacency (forward) and its reverse, over edges whose endpoints
    // both exist (dangling edges are reported by edge_endpoint_exists).
    let mut forward: HashMap<&str, Vec<&str>> = HashMap::new();
    let mut reverse: HashMap<&str, Vec<&str>> = HashMap::new();
    let mut has_incoming: HashSet<&str> = HashSet::new();
    for edge in &graph.edges {
        let s = edge.source_node_id.as_str();
        let t = edge.target_node_id.as_str();
        if node_by_id.contains_key(s) && node_by_id.contains_key(t) {
            forward.entry(s).or_default().push(t);
            reverse.entry(t).or_default().push(s);
            has_incoming.insert(t);
        }
    }

    // Determine the root: explicit root_node_id (when it resolves), else the
    // unique node with no incoming resolvable edge. Skip if ambiguous/missing.
    //
    // The no-incoming fallback only anchors when the canvas has at least one
    // outcome node — without one, "reaching an outcome" is inapplicable and a
    // half-built canvas (a lone decision node, an unresolved edge) stays valid;
    // an explicit root always anchors so a deliberate dead-end is still caught.
    let root: &str = match graph.root_node_id.as_deref() {
        Some(r) if node_by_id.contains_key(r) => r,
        Some(_) => return, // root_in_nodes already flagged it; can't anchor.
        None => {
            if !graph.nodes.iter().any(Node::is_outcome) {
                return;
            }
            let mut roots = graph
                .nodes
                .iter()
                .map(Node::id)
                .filter(|id| !has_incoming.contains(id));
            match (roots.next(), roots.next()) {
                (Some(only), None) => only,
                _ => return, // zero or multiple roots => skip (no single start).
            }
        }
    };

    // can_reach_outcome: reverse-BFS from every outcome node.
    let mut can_reach: HashSet<&str> = HashSet::new();
    let mut queue: Vec<&str> = graph
        .nodes
        .iter()
        .filter(|n| n.is_outcome())
        .map(Node::id)
        .collect();
    for &start in &queue {
        can_reach.insert(start);
    }
    while let Some(node) = queue.pop() {
        if let Some(preds) = reverse.get(node) {
            for &p in preds {
                if can_reach.insert(p) {
                    queue.push(p);
                }
            }
        }
    }

    // reachable: forward-BFS from the root.
    let mut reachable: HashSet<&str> = HashSet::new();
    reachable.insert(root);
    let mut stack = vec![root];
    while let Some(node) = stack.pop() {
        if let Some(succ) = forward.get(node) {
            for &t in succ {
                if reachable.insert(t) {
                    stack.push(t);
                }
            }
        }
    }

    // Any reachable node that cannot reach an outcome is a dead-end.
    for (idx, node) in graph.nodes.iter().enumerate() {
        let id = node.id();
        if reachable.contains(id) && !can_reach.contains(id) {
            details.push(ValidationDetail::new(
                format!("rule_graph.{canvas}.nodes[{idx}]"),
                format!("node '{id}' cannot reach an outcome"),
                "outcome_reachable",
            ));
        }
    }
}

/// Validate one Decision node's processor against the matched manifest spec.
///
/// Emits, in order: `processor_kind_known` (when `type` is not a manifest
/// `kind` — and then no field checks are possible); per required field
/// `processor_field_required`; per `select` field with a non-empty value
/// `processor_field_option`. Unknown extra fields on the processor are ignored.
fn validate_processor(
    canvas: &'static str,
    idx: usize,
    processor: &ProcessorConfig,
    spec_by_kind: &HashMap<&str, &NodeTypeSpec>,
    details: &mut Vec<ValidationDetail>,
) {
    let loc = format!("rule_graph.{canvas}.nodes[{idx}]");

    // processor_kind_known: a known manifest `kind` is required for any field
    // checks; without a spec we cannot validate further.
    let Some(spec) = spec_by_kind.get(processor.r#type.as_str()) else {
        details.push(ValidationDetail::new(
            loc,
            format!("unknown processor type '{}'", processor.r#type),
            "processor_kind_known",
        ));
        return;
    };

    for field in &spec.fields {
        let current = processor.fields.get(&field.name);

        // processor_field_required: required when `required` is true OR the
        // `required_unless` sibling value is not equal to the configured value.
        if is_required(field, &processor.fields) && !is_non_empty(current) {
            let msg = field.required_message.clone().unwrap_or_else(|| {
                format!("field '{}' is required and must not be empty", field.name)
            });
            details.push(ValidationDetail::new(
                loc.clone(),
                msg,
                "processor_field_required",
            ));
        }

        // processor_field_option: a `select` value (when non-empty) must be one
        // of the field's options. Emptiness is covered by the required rule.
        if field.control == Control::Select && is_non_empty(current) {
            let value = current.expect("non-empty value present");
            let allowed = field.options.iter().any(|o| &o.value == value);
            if !allowed {
                details.push(ValidationDetail::new(
                    loc.clone(),
                    format!(
                        "field '{}' value {} is not one of the allowed options",
                        field.name, value
                    ),
                    "processor_field_option",
                ));
            }
        }
    }
}

/// Whether `field` is required given the sibling values on the processor.
///
/// `required == true` always requires the field. A `required_unless { field,
/// value }` clause requires it UNLESS the named sibling's current value equals
/// `value` (when the sibling matches, the field is optional).
fn is_required(field: &Field, siblings: &serde_json::Map<String, Value>) -> bool {
    if field.required {
        return true;
    }
    if let Some(ru) = &field.required_unless {
        let sibling = siblings.get(&ru.field);
        // Required while the sibling does NOT equal the exempting value.
        return sibling != Some(&ru.value);
    }
    false
}

/// "Non-empty" per the contract: present, not JSON `null`, and (for strings) not
/// empty after trimming.
fn is_non_empty(value: Option<&Value>) -> bool {
    match value {
        None | Some(Value::Null) => false,
        Some(Value::String(s)) => !s.trim().is_empty(),
        Some(_) => true,
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
    use crate::schemas::node_type::LoadedManifest;
    use crate::schemas::rule_graph::{Edge, Position, ProcessorConfig};
    use serde_json::json;

    fn pos() -> Position {
        Position { x: 0.0, y: 0.0 }
    }

    /// The real backend manifest (ported `meta_tags`/`device_type`/`article_url`),
    /// loaded from the committed file so processor rules are exercised end-to-end.
    fn manifest() -> NodeManifest {
        let loaded =
            LoadedManifest::load("config/node_types.json").expect("load node manifest for tests");
        (*loaded.typed).clone()
    }

    /// A generic processor: `type` + a flat field map (the wire shape).
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

    /// A valid `device_type` decision node (used by structural-rule tests).
    fn decision(id: &str) -> Node {
        Node::Decision {
            id: id.to_string(),
            processor: processor(
                json!({"type": "device_type", "operator": "equals", "value": "mobile"}),
            ),
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
        let res = validate(&RuleGraph::default(), &HashSet::new(), &manifest());
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
        let res = validate(&graph_with_anonymous(canvas), &ids, &manifest());
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
        let details = details_of(validate(
            &graph_with_anonymous(canvas),
            &HashSet::new(),
            &manifest(),
        ));
        assert!(rule_ids(&details).contains(&"edge_endpoint_exists"));
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
        let details = details_of(validate(&graph_with_anonymous(canvas), &ids, &manifest()));
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
        let details = details_of(validate(&graph_with_anonymous(canvas), &ids, &manifest()));
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
        assert!(validate(&graph_with_anonymous(canvas), &ids, &manifest()).is_ok());
    }

    // 7. no_cycles: a self-loop is a cycle.
    #[test]
    fn self_loop_is_cycle() {
        let canvas = CanvasGraph {
            nodes: vec![decision("d1")],
            edges: vec![edge("e1", "d1", "d1", Branch::Yes)],
            root_node_id: None,
        };
        let details = details_of(validate(
            &graph_with_anonymous(canvas),
            &HashSet::new(),
            &manifest(),
        ));
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
        let details = details_of(validate(
            &graph_with_anonymous(canvas),
            &HashSet::new(),
            &manifest(),
        ));
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
        assert!(validate(&graph_with_anonymous(canvas), &ids, &manifest()).is_ok());
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
        let details = details_of(validate(&graph_with_anonymous(canvas), &ids, &manifest()));
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
        let details = details_of(validate(
            &graph_with_anonymous(canvas),
            &HashSet::new(),
            &manifest(),
        ));
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
        let details = details_of(validate(
            &graph_with_anonymous(canvas),
            &HashSet::new(),
            &manifest(),
        ));
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
        let details = details_of(validate(&graph, &HashSet::new(), &manifest()));
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
        let details = details_of(validate(&graph, &HashSet::new(), &manifest()));
        // Two canvases each contribute an endpoint error + a root error => >= 4.
        assert!(details.len() >= 4, "got {} details", details.len());
        assert!(rule_ids(&details).contains(&"edge_endpoint_exists"));
        assert!(rule_ids(&details).contains(&"root_in_nodes"));
    }

    // 15. A meta_tags decision node round-trips through manifest-driven validation.
    #[test]
    fn meta_tags_decision_valid() {
        let oid = Uuid::new_v4();
        let canvas = CanvasGraph {
            nodes: vec![
                Node::Decision {
                    id: "m1".to_string(),
                    processor: processor(
                        json!({"type": "meta_tags", "tag_name": "paywall", "operator": "contains", "value": "true"}),
                    ),
                    position: pos(),
                },
                outcome("o1", oid),
            ],
            edges: vec![edge("e1", "m1", "o1", Branch::Yes)],
            root_node_id: Some("m1".to_string()),
        };
        let mut ids = HashSet::new();
        ids.insert(oid);
        assert!(validate(&graph_with_anonymous(canvas), &ids, &manifest()).is_ok());
    }

    // 16. processor_kind_known: an unknown processor `type` fails (no field checks).
    #[test]
    fn unknown_processor_kind_fails() {
        let canvas = CanvasGraph {
            nodes: vec![Node::Decision {
                id: "x".to_string(),
                processor: processor(json!({"type": "not_a_real_kind", "foo": "bar"})),
                position: pos(),
            }],
            edges: vec![],
            root_node_id: None,
        };
        let details = details_of(validate(
            &graph_with_anonymous(canvas),
            &HashSet::new(),
            &manifest(),
        ));
        assert_eq!(rule_ids(&details), vec!["processor_kind_known"]);
    }

    // 17. processor_field_required: a required field missing/empty fails, and the
    //     manifest's `required_message` is used.
    #[test]
    fn required_field_missing_fails() {
        let canvas = CanvasGraph {
            nodes: vec![Node::Decision {
                id: "a".to_string(),
                // article_url requires `value` (non-empty); empty string fails.
                processor: processor(
                    json!({"type": "article_url", "operator": "contains", "value": "   "}),
                ),
                position: pos(),
            }],
            edges: vec![],
            root_node_id: None,
        };
        let details = details_of(validate(
            &graph_with_anonymous(canvas),
            &HashSet::new(),
            &manifest(),
        ));
        assert!(rule_ids(&details).contains(&"processor_field_required"));
        assert!(details
            .iter()
            .any(|d| d.msg == "Enter a value to compare against the URL"));
    }

    // 18. required_unless: meta_tags `value` is optional when operator == "exists".
    #[test]
    fn meta_tags_exists_omits_value_ok() {
        let oid = Uuid::new_v4();
        let canvas = CanvasGraph {
            nodes: vec![
                Node::Decision {
                    id: "m".to_string(),
                    processor: processor(
                        json!({"type": "meta_tags", "tag_name": "robots", "operator": "exists"}),
                    ),
                    position: pos(),
                },
                outcome("o", oid),
            ],
            edges: vec![edge("e", "m", "o", Branch::Yes)],
            root_node_id: Some("m".to_string()),
        };
        let mut ids = HashSet::new();
        ids.insert(oid);
        assert!(validate(&graph_with_anonymous(canvas), &ids, &manifest()).is_ok());
    }

    // 19. required_unless: meta_tags `value` is required when operator != "exists".
    #[test]
    fn meta_tags_contains_requires_value() {
        let canvas = CanvasGraph {
            nodes: vec![Node::Decision {
                id: "m".to_string(),
                // operator=contains, value omitted => required_unless triggers.
                processor: processor(
                    json!({"type": "meta_tags", "tag_name": "paywall", "operator": "contains"}),
                ),
                position: pos(),
            }],
            edges: vec![],
            root_node_id: None,
        };
        let details = details_of(validate(
            &graph_with_anonymous(canvas),
            &HashSet::new(),
            &manifest(),
        ));
        assert!(rule_ids(&details).contains(&"processor_field_required"));
    }

    // 20. processor_field_option: a `select` value outside its options fails.
    #[test]
    fn select_value_not_in_options_fails() {
        let canvas = CanvasGraph {
            nodes: vec![Node::Decision {
                id: "d".to_string(),
                processor: processor(
                    json!({"type": "device_type", "operator": "equals", "value": "watch"}),
                ),
                position: pos(),
            }],
            edges: vec![],
            root_node_id: None,
        };
        let details = details_of(validate(
            &graph_with_anonymous(canvas),
            &HashSet::new(),
            &manifest(),
        ));
        assert!(rule_ids(&details).contains(&"processor_field_option"));
    }

    // 21. Unknown extra fields are ignored (forward-compatible), not an error.
    #[test]
    fn unknown_extra_fields_ignored() {
        let canvas = CanvasGraph {
            nodes: vec![Node::Decision {
                id: "d".to_string(),
                processor: processor(
                    json!({"type": "device_type", "operator": "equals", "value": "mobile", "future_field": 42}),
                ),
                position: pos(),
            }],
            edges: vec![],
            root_node_id: None,
        };
        assert!(validate(&graph_with_anonymous(canvas), &HashSet::new(), &manifest()).is_ok());
    }

    // 22. outcome_reachable: a dead-end decision with a set root fails (the root
    //     node has no path to any outcome).
    #[test]
    fn dead_end_decision_with_root_fails() {
        let canvas = CanvasGraph {
            nodes: vec![decision("d1")],
            edges: vec![],
            root_node_id: Some("d1".to_string()),
        };
        let details = details_of(validate(
            &graph_with_anonymous(canvas),
            &HashSet::new(),
            &manifest(),
        ));
        assert!(rule_ids(&details).contains(&"outcome_reachable"));
        assert!(details
            .iter()
            .any(|d| d.rule_id == "outcome_reachable"
                && d.msg == "node 'd1' cannot reach an outcome"));
    }

    // 23. outcome_reachable: decision -> decision -> outcome chain passes.
    #[test]
    fn decision_chain_to_outcome_passes() {
        let oid = Uuid::new_v4();
        let canvas = CanvasGraph {
            nodes: vec![decision("d1"), decision("d2"), outcome("o1", oid)],
            edges: vec![
                edge("e1", "d1", "d2", Branch::Yes),
                edge("e2", "d2", "o1", Branch::Yes),
            ],
            root_node_id: Some("d1".to_string()),
        };
        let mut ids = HashSet::new();
        ids.insert(oid);
        assert!(validate(&graph_with_anonymous(canvas), &ids, &manifest()).is_ok());
    }

    // 24. outcome_reachable: an outcome-only canvas (the unique root IS an
    //     outcome) passes — the outcome trivially reaches itself.
    #[test]
    fn outcome_only_root_passes() {
        let oid = Uuid::new_v4();
        let canvas = CanvasGraph {
            nodes: vec![outcome("o1", oid)],
            edges: vec![],
            root_node_id: None,
        };
        let mut ids = HashSet::new();
        ids.insert(oid);
        assert!(validate(&graph_with_anonymous(canvas), &ids, &manifest()).is_ok());
    }
}
