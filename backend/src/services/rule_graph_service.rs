//! Rule-graph validation (BACKEND CONTRACT §6).
//!
//! [`validate`] checks a [`RuleGraph`] against the stable rules below, per canvas
//! (anonymous / registered / customer). Every failure produces a
//! [`ValidationDetail`] whose `loc` is `rule_graph.<canvas>.<field>[idx]` and
//! whose `rule_id` is one of the STABLE identifiers in the table. Any failure
//! yields an [`AppError::Validation`] (HTTP 422).
//!
//! A non-empty canvas is an in-graph action pipeline:
//! `Start → (decisions route) → expression/action nodes → End`.
//!
//! | `rule_id` | Rule |
//! |---|---|
//! | `edge_endpoint_exists` | every edge endpoint exists in `nodes` |
//! | `branch_unique` | a Decision node has at most one outgoing edge per branch |
//! | `no_cycles` | the graph is acyclic |
//! | `root_in_nodes` | a set `root_node_id` exists in `nodes` |
//! | `start_present` | a non-empty canvas has exactly ONE `start` node |
//! | `end_present` | a non-empty canvas has ≥1 `end` node |
//! | `start_no_incoming` | no edge targets a `start` node |
//! | `start_single_out` | a `start` node has exactly one outgoing edge |
//! | `expression_single_out` | an `expression` node has exactly one outgoing edge |
//! | `end_terminal` | an `end` node has zero outgoing edges |
//! | `edge_source_kind` | edges originate only from start/decision/expression, never `end` |
//! | `all_paths_reach_end` | every node reachable from the Start node can reach an `end` |
//! | `apply_outcome_ref_exists` | an `apply_outcome` action's `outcome_id` exists for the version |
//! | `apply_component_ref_exists` | an `apply_component`/`apply_component_json` action's `component_id` is a UUID present in `rre.component_templates` |
//! | `apply_component_version_valid` | such an action's `version` is the string `"default"` or a positive integer (well-formedness only; drift is fail-open at the proxy) |
//! | `processor_kind_known` | a Decision/Expression node's `type` is a manifest `kind` |
//! | `processor_field_required` | each required field (incl. unsatisfied `required_unless`) is present and non-empty |
//! | `processor_field_option` | a `select` field's value is one of its `options[].value` |
//!
//! The structural rules are DB-agnostic: the set of valid outcome ids for the
//! version is supplied by the caller (the version service reads `rre.outcomes`).
//! The processor rules are manifest-driven: the typed processor enum is gone, so
//! `validate` takes a [`NodeManifest`] and checks each Decision node's processor
//! and each Expression node's `action` against the matched spec. That keeps the
//! validator a pure function, fully unit-testable without a database.

use std::collections::{HashMap, HashSet};

use serde_json::Value;
use uuid::Uuid;

use crate::{
    error::{AppError, AppResult, ValidationDetail},
    schemas::{
        node_type::{Control, Field, NodeManifest, NodeTypeSpec},
        rule_graph::{Branch, CanvasGraph, Edge, Node, Position, ProcessorConfig, RuleGraph},
    },
};

/// Normalize a full [`RuleGraph`]: any canvas with zero nodes is replaced with
/// the default `start → end` graph (one start node, one end node, one edge).
/// Non-empty canvases are untouched.
pub fn normalize(graph: &mut RuleGraph) {
    normalize_canvas(&mut graph.anonymous);
    normalize_canvas(&mut graph.registered);
    normalize_canvas(&mut graph.customer);
}

/// Replace an empty [`CanvasGraph`] (zero nodes) with the default start → end
/// graph. Non-empty canvases are returned untouched.
fn normalize_canvas(canvas: &mut CanvasGraph) {
    if !canvas.nodes.is_empty() {
        return;
    }
    canvas.nodes = vec![
        Node::Start {
            id: "start".to_string(),
            position: Position { x: 40.0, y: 160.0 },
        },
        Node::End {
            id: "end".to_string(),
            position: Position { x: 940.0, y: 160.0 },
        },
    ];
    canvas.edges = vec![Edge {
        id: "e_start_end".to_string(),
        source_node_id: "start".to_string(),
        target_node_id: "end".to_string(),
        branch: Branch::Yes,
    }];
    canvas.root_node_id = Some("start".to_string());
}

/// Validate a full [`RuleGraph`] across all three canvases.
///
/// `valid_outcome_ids` is the set of `rre.outcomes.id` values that belong to the
/// version being edited; it backs the `apply_outcome_ref_exists` rule.
/// `valid_component_ids` is the set of `rre.component_templates.id` values that
/// exist; it backs the `apply_component_ref_exists` rule (threaded the same way).
/// `manifest` backs the processor rules (`processor_kind_known`,
/// `processor_field_required`, `processor_field_option`). Callers (the version
/// service) fetch both id sets and supply the manifest from `AppState` before
/// invoking validation.
///
/// Returns `Ok(())` when every canvas passes; otherwise an
/// [`AppError::Validation`] carrying one [`ValidationDetail`] per violation.
pub fn validate(
    graph: &RuleGraph,
    valid_outcome_ids: &HashSet<Uuid>,
    valid_component_ids: &HashSet<Uuid>,
    manifest: &NodeManifest,
) -> AppResult<()> {
    let mut details: Vec<ValidationDetail> = Vec::new();
    let spec_by_kind = manifest.index();

    for (canvas_name, canvas) in graph.canvases() {
        validate_canvas(
            canvas_name,
            canvas,
            valid_outcome_ids,
            valid_component_ids,
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
    valid_component_ids: &HashSet<Uuid>,
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

    // start_present / end_present: a non-empty canvas needs exactly one start
    // and at least one end. An empty canvas (zero nodes) stays valid.
    let start_indices: Vec<usize> = graph
        .nodes
        .iter()
        .enumerate()
        .filter(|(_, n)| n.is_start())
        .map(|(i, _)| i)
        .collect();
    let has_end = graph.nodes.iter().any(Node::is_end);

    if !graph.nodes.is_empty() {
        match start_indices.len() {
            0 => details.push(ValidationDetail::new(
                format!("rule_graph.{canvas}.nodes"),
                "canvas is missing a start node".to_string(),
                "start_present",
            )),
            1 => {}
            n => details.push(ValidationDetail::new(
                format!("rule_graph.{canvas}.nodes[{}]", start_indices[1]),
                format!("canvas has {n} start nodes; expected exactly one"),
                "start_present",
            )),
        }
        if !has_end {
            details.push(ValidationDetail::new(
                format!("rule_graph.{canvas}.nodes"),
                "canvas is missing an end node".to_string(),
                "end_present",
            ));
        }
    }

    // apply_outcome_ref_exists + processor_* (manifest-driven, per Decision and
    // Expression node).
    for (idx, node) in graph.nodes.iter().enumerate() {
        match node {
            Node::Decision { processor, .. } => {
                validate_processor(canvas, idx, processor, spec_by_kind, details);
            }
            Node::Expression {
                id,
                action,
                custom_label,
                ..
            } => {
                validate_processor(canvas, idx, action, spec_by_kind, details);
                validate_apply_outcome_ref(canvas, idx, action, valid_outcome_ids, details);
                validate_apply_component_ref(canvas, idx, action, valid_component_ids, details);
                validate_apply_component_version(canvas, idx, action, details);
                validate_custom_label(canvas, id, custom_label.as_deref(), details);
            }
            Node::Start { .. } | Node::End { .. } => {}
        }
    }

    // Per-source out-degree + branch tracking, plus edge-level rules.
    let mut seen_branch: HashSet<(&str, Branch)> = HashSet::new();
    let mut out_degree: HashMap<&str, usize> = HashMap::new();

    for (idx, edge) in graph.edges.iter().enumerate() {
        let loc = format!("rule_graph.{canvas}.edges[{idx}]");

        let source = node_by_id.get(edge.source_node_id.as_str());
        let target = node_by_id.get(edge.target_node_id.as_str());

        // edge_endpoint_exists (source)
        if source.is_none() {
            details.push(ValidationDetail::new(
                loc.clone(),
                format!("edge source '{}' not found in nodes", edge.source_node_id),
                "edge_endpoint_exists",
            ));
        }
        // edge_endpoint_exists (target)
        if target.is_none() {
            details.push(ValidationDetail::new(
                loc.clone(),
                format!("edge target '{}' not found in nodes", edge.target_node_id),
                "edge_endpoint_exists",
            ));
        }

        // start_no_incoming: no edge may target a start node.
        if let Some(tgt_node) = target {
            if tgt_node.is_start() {
                details.push(ValidationDetail::new(
                    loc.clone(),
                    format!(
                        "start node '{}' must not have incoming edges",
                        tgt_node.id()
                    ),
                    "start_no_incoming",
                ));
            }
        }

        // Source-origin rules only apply when the source node resolves.
        if let Some(src_node) = source {
            *out_degree.entry(src_node.id()).or_insert(0) += 1;

            match src_node {
                // end_terminal / edge_source_kind: end nodes may not originate edges.
                Node::End { .. } => {
                    details.push(ValidationDetail::new(
                        loc.clone(),
                        format!(
                            "end node '{}' must be terminal (no outgoing edges)",
                            src_node.id()
                        ),
                        "end_terminal",
                    ));
                    details.push(ValidationDetail::new(
                        loc.clone(),
                        format!(
                            "edge may only originate from start/decision/expression, not end '{}'",
                            src_node.id()
                        ),
                        "edge_source_kind",
                    ));
                }
                // branch_unique: a decision node has at most one edge per branch.
                Node::Decision { .. } => {
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
                // start/expression have exactly one outgoing edge (checked below
                // via out_degree); no per-edge rule here.
                Node::Start { .. } | Node::Expression { .. } => {}
            }
        }
    }

    // start_single_out / expression_single_out: each start/expression node has
    // exactly one outgoing edge.
    for (idx, node) in graph.nodes.iter().enumerate() {
        let degree = out_degree.get(node.id()).copied().unwrap_or(0);
        match node {
            Node::Start { .. } if degree != 1 => details.push(ValidationDetail::new(
                format!("rule_graph.{canvas}.nodes[{idx}]"),
                format!(
                    "start node '{}' must have exactly one outgoing edge (found {degree})",
                    node.id()
                ),
                "start_single_out",
            )),
            Node::Expression { .. } if degree != 1 => details.push(ValidationDetail::new(
                format!("rule_graph.{canvas}.nodes[{idx}]"),
                format!(
                    "expression node '{}' must have exactly one outgoing edge (found {degree})",
                    node.id()
                ),
                "expression_single_out",
            )),
            _ => {}
        }
    }

    // no_cycles: DFS over the (resolvable) adjacency.
    if has_cycle(graph, &node_by_id) {
        details.push(ValidationDetail::new(
            format!("rule_graph.{canvas}.edges"),
            "graph contains a cycle".to_string(),
            "no_cycles",
        ));
    }

    // all_paths_reach_end: every node reachable from the Start node must be able
    // to reach an end node (dead-ends are invalid).
    validate_all_paths_reach_end(canvas, graph, &node_by_id, &start_indices, details);
}

/// `all_paths_reach_end`: every node reachable from the unique Start node must be
/// able to reach at least one End node over resolvable edges.
///
/// Anchored at the unique `start` node (no more no-incoming heuristic). Skipped
/// when the canvas is empty or has no single start (`start_present` covers the
/// latter; an empty canvas stays valid).
fn validate_all_paths_reach_end(
    canvas: &'static str,
    graph: &CanvasGraph,
    node_by_id: &HashMap<&str, &Node>,
    start_indices: &[usize],
    details: &mut Vec<ValidationDetail>,
) {
    if graph.nodes.is_empty() {
        return;
    }
    // Anchor at the single start node; without exactly one, start_present has
    // already flagged the problem and there is no well-defined root.
    let [start_idx] = start_indices else {
        return;
    };
    let root: &str = graph.nodes[*start_idx].id();

    // Resolvable adjacency (forward) and its reverse, over edges whose endpoints
    // both exist (dangling edges are reported by edge_endpoint_exists).
    let mut forward: HashMap<&str, Vec<&str>> = HashMap::new();
    let mut reverse: HashMap<&str, Vec<&str>> = HashMap::new();
    for edge in &graph.edges {
        let s = edge.source_node_id.as_str();
        let t = edge.target_node_id.as_str();
        if node_by_id.contains_key(s) && node_by_id.contains_key(t) {
            forward.entry(s).or_default().push(t);
            reverse.entry(t).or_default().push(s);
        }
    }

    // can_reach_end: reverse-BFS from every end node.
    let mut can_reach: HashSet<&str> = HashSet::new();
    let mut queue: Vec<&str> = graph
        .nodes
        .iter()
        .filter(|n| n.is_end())
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

    // Any reachable node that cannot reach an end is a dead-end.
    for (idx, node) in graph.nodes.iter().enumerate() {
        let id = node.id();
        if reachable.contains(id) && !can_reach.contains(id) {
            details.push(ValidationDetail::new(
                format!("rule_graph.{canvas}.nodes[{idx}]"),
                format!("node '{id}' cannot reach an end"),
                "all_paths_reach_end",
            ));
        }
    }
}

/// `apply_outcome_ref_exists`: an expression node whose `action.type ==
/// "apply_outcome"` must carry an `outcome_id` present in `valid_outcome_ids`.
/// Other expression kinds are ignored here.
fn validate_apply_outcome_ref(
    canvas: &'static str,
    idx: usize,
    action: &ProcessorConfig,
    valid_outcome_ids: &HashSet<Uuid>,
    details: &mut Vec<ValidationDetail>,
) {
    if action.r#type != "apply_outcome" {
        return;
    }
    let loc = format!("rule_graph.{canvas}.nodes[{idx}]");

    // Accept either `action.outcome_id` directly or `action.fields["outcome_id"]`
    // (both flatten to the same map on the wire). Parse the string as a UUID.
    let raw = action.fields.get("outcome_id");
    let parsed: Option<Uuid> = match raw {
        Some(Value::String(s)) => Uuid::parse_str(s).ok(),
        _ => None,
    };
    match parsed {
        Some(id) if valid_outcome_ids.contains(&id) => {}
        _ => {
            let shown = match raw {
                Some(Value::String(s)) => s.clone(),
                Some(other) => other.to_string(),
                None => "<missing>".to_string(),
            };
            details.push(ValidationDetail::new(
                loc,
                format!("outcome_id '{shown}' not found in outcomes for this version"),
                "apply_outcome_ref_exists",
            ));
        }
    }
}

/// `apply_component_ref_exists`: an expression node whose `action.type` is
/// `apply_component` / `apply_component_json` must carry a `component_id` that is
/// a UUID present in `valid_component_ids`. Other expression kinds are ignored.
/// Mirrors [`validate_apply_outcome_ref`].
fn validate_apply_component_ref(
    canvas: &'static str,
    idx: usize,
    action: &ProcessorConfig,
    valid_component_ids: &HashSet<Uuid>,
    details: &mut Vec<ValidationDetail>,
) {
    if !is_apply_component(&action.r#type) {
        return;
    }
    let loc = format!("rule_graph.{canvas}.nodes[{idx}]");

    let raw = action.fields.get("component_id");
    let parsed: Option<Uuid> = match raw {
        Some(Value::String(s)) => Uuid::parse_str(s).ok(),
        _ => None,
    };
    match parsed {
        Some(id) if valid_component_ids.contains(&id) => {}
        _ => {
            let shown = match raw {
                Some(Value::String(s)) => s.clone(),
                Some(other) => other.to_string(),
                None => "<missing>".to_string(),
            };
            details.push(ValidationDetail::new(
                loc,
                format!("component_id '{shown}' not found in components"),
                "apply_component_ref_exists",
            ));
        }
    }
}

/// `apply_component_version_valid`: an `apply_component` / `apply_component_json`
/// action's `version` must be the string `"default"` or a positive integer
/// (well-formedness only). A pinned version number that no longer exists is NOT
/// a save-time error — version drift is handled fail-open at the proxy.
fn validate_apply_component_version(
    canvas: &'static str,
    idx: usize,
    action: &ProcessorConfig,
    details: &mut Vec<ValidationDetail>,
) {
    if !is_apply_component(&action.r#type) {
        return;
    }
    let loc = format!("rule_graph.{canvas}.nodes[{idx}]");

    let valid = match action.fields.get("version") {
        Some(Value::String(s)) => {
            let s = s.trim();
            s == "default" || s.parse::<i64>().is_ok_and(|n| n > 0)
        }
        Some(Value::Number(n)) => n.as_i64().is_some_and(|n| n > 0),
        _ => false,
    };
    if !valid {
        details.push(ValidationDetail::new(
            loc,
            "version must be 'default' or a positive integer".to_string(),
            "apply_component_version_valid",
        ));
    }
}

/// Whether an action `type` is one of the Component-apply kinds.
fn is_apply_component(action_type: &str) -> bool {
    action_type == "apply_component" || action_type == "apply_component_json"
}

/// `expression_custom_label_invalid`: an expression node's optional
/// `custom_label`, when present and non-empty (after trim), must be snake_case
/// (`^[a-z0-9]+(_[a-z0-9]+)*$`). An empty/absent value is valid (no custom name).
fn validate_custom_label(
    canvas: &'static str,
    node_id: &str,
    custom_label: Option<&str>,
    details: &mut Vec<ValidationDetail>,
) {
    let Some(label) = custom_label else { return };
    if label.trim().is_empty() {
        return;
    }
    if !is_snake_case(label) {
        details.push(ValidationDetail::new(
            format!("{canvas}.nodes[{node_id}].custom_label"),
            "Custom name must be snake_case (lowercase letters, digits, single underscores) or left empty.".to_string(),
            "expression_custom_label_invalid",
        ));
    }
}

/// `^[a-z0-9]+(_[a-z0-9]+)*$` without a regex crate: split on `_`; every segment
/// must be non-empty (rejects leading/trailing/double underscore) and contain
/// only `a-z`/`0-9` bytes.
fn is_snake_case(value: &str) -> bool {
    value.split('_').all(|seg| {
        !seg.is_empty()
            && seg
                .bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit())
    })
}

/// Validate one Decision processor / Expression action against the matched
/// manifest spec.
///
/// Emits, in order: `processor_kind_known` (when `type` is not a manifest
/// `kind` — and then no field checks are possible); per required field
/// `processor_field_required`; per `select` field with a non-empty value
/// `processor_field_option`. `outcome_select` controls skip the option check
/// (their options are dynamic; `apply_outcome_ref_exists` covers them) but still
/// honor `processor_field_required`. Unknown extra fields are ignored.
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
        // Applies to outcome_select too (the field must be non-empty).
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
        // of the field's options. `outcome_select` options are dynamic (covered
        // by apply_outcome_ref_exists), so skip it here.
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

    /// The real backend manifest (ported decision kinds + the three expression
    /// kinds), loaded from the committed file so processor rules are exercised
    /// end-to-end.
    fn manifest() -> NodeManifest {
        let loaded =
            LoadedManifest::load("config/node_types.json").expect("load node manifest for tests");
        (*loaded.typed).clone()
    }

    /// A generic processor/action: `type` + a flat field map (the wire shape).
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

    /// An `apply_outcome` expression node referencing `outcome_id`.
    fn expression_apply(id: &str, outcome_id: Uuid) -> Node {
        Node::Expression {
            id: id.to_string(),
            action: processor(
                json!({"type": "apply_outcome", "outcome_id": outcome_id.to_string()}),
            ),
            custom_label: None,
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
        let res = validate(
            &RuleGraph::default(),
            &HashSet::new(),
            &HashSet::new(),
            &manifest(),
        );
        assert!(res.is_ok());
    }

    // 2. A well-formed start -> decision -> expression/end canvas passes.
    #[test]
    fn well_formed_canvas_is_valid() {
        let oid = Uuid::new_v4();
        let canvas = CanvasGraph {
            nodes: vec![
                start("s"),
                decision("d1"),
                expression_apply("a1", oid),
                end("e"),
            ],
            edges: vec![
                edge("e0", "s", "d1", Branch::Yes),
                edge("e1", "d1", "a1", Branch::Yes),
                edge("e2", "d1", "e", Branch::No),
                edge("e3", "a1", "e", Branch::Yes),
            ],
            root_node_id: Some("s".to_string()),
        };
        let mut ids = HashSet::new();
        ids.insert(oid);
        let res = validate(
            &graph_with_anonymous(canvas),
            &ids,
            &HashSet::new(),
            &manifest(),
        );
        assert!(res.is_ok(), "expected ok, got {res:?}");
    }

    // 3. edge_endpoint_exists: missing target.
    #[test]
    fn edge_target_missing() {
        let canvas = CanvasGraph {
            nodes: vec![start("s"), decision("d1"), end("e")],
            edges: vec![
                edge("e0", "s", "d1", Branch::Yes),
                edge("e1", "d1", "ghost", Branch::Yes),
            ],
            root_node_id: None,
        };
        let details = details_of(validate(
            &graph_with_anonymous(canvas),
            &HashSet::new(),
            &HashSet::new(),
            &manifest(),
        ));
        assert!(rule_ids(&details).contains(&"edge_endpoint_exists"));
    }

    // 4. edge_endpoint_exists: missing source.
    #[test]
    fn edge_source_missing() {
        let canvas = CanvasGraph {
            nodes: vec![start("s"), end("e")],
            edges: vec![
                edge("e0", "s", "e", Branch::Yes),
                edge("e1", "ghost", "e", Branch::Yes),
            ],
            root_node_id: None,
        };
        let details = details_of(validate(
            &graph_with_anonymous(canvas),
            &HashSet::new(),
            &HashSet::new(),
            &manifest(),
        ));
        assert!(rule_ids(&details).contains(&"edge_endpoint_exists"));
    }

    // 5. branch_unique: two `yes` edges from one decision node.
    #[test]
    fn duplicate_yes_branch() {
        let canvas = CanvasGraph {
            nodes: vec![start("s"), decision("d1"), end("y"), end("n")],
            edges: vec![
                edge("e0", "s", "d1", Branch::Yes),
                edge("e1", "d1", "y", Branch::Yes),
                edge("e2", "d1", "n", Branch::Yes),
            ],
            root_node_id: None,
        };
        let details = details_of(validate(
            &graph_with_anonymous(canvas),
            &HashSet::new(),
            &HashSet::new(),
            &manifest(),
        ));
        assert!(rule_ids(&details).contains(&"branch_unique"));
    }

    // 6. branch_unique allows one yes + one no from the same node.
    #[test]
    fn yes_and_no_from_same_node_ok() {
        let canvas = CanvasGraph {
            nodes: vec![start("s"), decision("d1"), end("y"), end("n")],
            edges: vec![
                edge("e0", "s", "d1", Branch::Yes),
                edge("e1", "d1", "y", Branch::Yes),
                edge("e2", "d1", "n", Branch::No),
            ],
            root_node_id: None,
        };
        assert!(validate(
            &graph_with_anonymous(canvas),
            &HashSet::new(),
            &HashSet::new(),
            &manifest()
        )
        .is_ok());
    }

    // 7. no_cycles: a self-loop is a cycle.
    #[test]
    fn self_loop_is_cycle() {
        let canvas = CanvasGraph {
            nodes: vec![start("s"), decision("d1"), end("e")],
            edges: vec![
                edge("e0", "s", "d1", Branch::Yes),
                edge("e1", "d1", "d1", Branch::Yes),
            ],
            root_node_id: None,
        };
        let details = details_of(validate(
            &graph_with_anonymous(canvas),
            &HashSet::new(),
            &HashSet::new(),
            &manifest(),
        ));
        assert!(rule_ids(&details).contains(&"no_cycles"));
    }

    // 8. no_cycles: a multi-node cycle d1 -> d2 -> d1.
    #[test]
    fn multi_node_cycle() {
        let canvas = CanvasGraph {
            nodes: vec![start("s"), decision("d1"), decision("d2"), end("e")],
            edges: vec![
                edge("e0", "s", "d1", Branch::Yes),
                edge("e1", "d1", "d2", Branch::Yes),
                edge("e2", "d2", "d1", Branch::Yes),
            ],
            root_node_id: None,
        };
        let details = details_of(validate(
            &graph_with_anonymous(canvas),
            &HashSet::new(),
            &HashSet::new(),
            &manifest(),
        ));
        assert!(rule_ids(&details).contains(&"no_cycles"));
    }

    // 9. A diamond (shared end, no back-edge) is acyclic and valid.
    #[test]
    fn diamond_is_acyclic() {
        let canvas = CanvasGraph {
            nodes: vec![
                start("s"),
                decision("d1"),
                decision("d2"),
                decision("d3"),
                end("e"),
            ],
            edges: vec![
                edge("e0", "s", "d1", Branch::Yes),
                edge("e1", "d1", "d2", Branch::Yes),
                edge("e2", "d1", "d3", Branch::No),
                edge("e3", "d2", "e", Branch::Yes),
                edge("e4", "d3", "e", Branch::Yes),
            ],
            root_node_id: Some("s".to_string()),
        };
        assert!(validate(
            &graph_with_anonymous(canvas),
            &HashSet::new(),
            &HashSet::new(),
            &manifest()
        )
        .is_ok());
    }

    // 10. end_terminal + edge_source_kind: an end node with an outgoing edge.
    #[test]
    fn end_node_with_outgoing_edge() {
        let canvas = CanvasGraph {
            nodes: vec![start("s"), end("e"), decision("d1")],
            edges: vec![
                edge("e0", "s", "e", Branch::Yes),
                edge("e1", "e", "d1", Branch::Yes),
            ],
            root_node_id: None,
        };
        let details = details_of(validate(
            &graph_with_anonymous(canvas),
            &HashSet::new(),
            &HashSet::new(),
            &manifest(),
        ));
        let ids_seen = rule_ids(&details);
        assert!(ids_seen.contains(&"end_terminal"));
        assert!(ids_seen.contains(&"edge_source_kind"));
    }

    // 11. apply_outcome_ref_exists: outcome_id not in the version's outcomes.
    #[test]
    fn apply_outcome_ref_missing() {
        let oid = Uuid::new_v4();
        let canvas = CanvasGraph {
            nodes: vec![start("s"), expression_apply("a", oid), end("e")],
            edges: vec![
                edge("e0", "s", "a", Branch::Yes),
                edge("e1", "a", "e", Branch::Yes),
            ],
            root_node_id: None,
        };
        // empty valid-id set => the reference is dangling.
        let details = details_of(validate(
            &graph_with_anonymous(canvas),
            &HashSet::new(),
            &HashSet::new(),
            &manifest(),
        ));
        assert!(rule_ids(&details).contains(&"apply_outcome_ref_exists"));
    }

    // 12. root_in_nodes: root_node_id points at a non-existent node.
    #[test]
    fn root_not_in_nodes() {
        let canvas = CanvasGraph {
            nodes: vec![start("s"), end("e")],
            edges: vec![edge("e0", "s", "e", Branch::Yes)],
            root_node_id: Some("ghost".to_string()),
        };
        let details = details_of(validate(
            &graph_with_anonymous(canvas),
            &HashSet::new(),
            &HashSet::new(),
            &manifest(),
        ));
        assert!(rule_ids(&details).contains(&"root_in_nodes"));
        assert!(details
            .iter()
            .any(|d| d.loc == "rule_graph.anonymous.root_node_id"));
    }

    // 13. Failures are reported on the correct canvas (registered, not anonymous).
    #[test]
    fn loc_reflects_canvas() {
        let canvas = CanvasGraph {
            nodes: vec![start("s"), decision("d1"), end("e")],
            edges: vec![
                edge("e0", "s", "d1", Branch::Yes),
                edge("e1", "d1", "ghost", Branch::Yes),
            ],
            root_node_id: None,
        };
        let graph = RuleGraph {
            registered: canvas,
            ..Default::default()
        };
        let details = details_of(validate(
            &graph,
            &HashSet::new(),
            &HashSet::new(),
            &manifest(),
        ));
        assert!(details
            .iter()
            .any(|d| d.loc.starts_with("rule_graph.registered.")));
    }

    // 14. Multiple violations across canvases accumulate.
    #[test]
    fn violations_accumulate_across_canvases() {
        let bad = || CanvasGraph {
            nodes: vec![start("s"), decision("d1"), end("e")],
            edges: vec![
                edge("e0", "s", "d1", Branch::Yes),
                edge("e1", "d1", "ghost", Branch::Yes),
            ],
            root_node_id: Some("missing".to_string()),
        };
        let graph = RuleGraph {
            anonymous: bad(),
            registered: bad(),
            customer: CanvasGraph::default(),
        };
        let details = details_of(validate(
            &graph,
            &HashSet::new(),
            &HashSet::new(),
            &manifest(),
        ));
        // Two canvases each contribute an endpoint error + a root error => >= 4.
        assert!(details.len() >= 4, "got {} details", details.len());
        assert!(rule_ids(&details).contains(&"edge_endpoint_exists"));
        assert!(rule_ids(&details).contains(&"root_in_nodes"));
    }

    // 15. A meta_tags decision node round-trips through manifest-driven validation.
    #[test]
    fn meta_tags_decision_valid() {
        let canvas = CanvasGraph {
            nodes: vec![
                start("s"),
                Node::Decision {
                    id: "m1".to_string(),
                    processor: processor(
                        json!({"type": "meta_tags", "tag_name": "paywall", "operator": "contains", "value": "true"}),
                    ),
                    position: pos(),
                },
                end("e"),
            ],
            edges: vec![
                edge("e0", "s", "m1", Branch::Yes),
                edge("e1", "m1", "e", Branch::Yes),
            ],
            root_node_id: Some("s".to_string()),
        };
        assert!(validate(
            &graph_with_anonymous(canvas),
            &HashSet::new(),
            &HashSet::new(),
            &manifest()
        )
        .is_ok());
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
            &HashSet::new(),
            &manifest(),
        ));
        assert!(rule_ids(&details).contains(&"processor_kind_known"));
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
        assert!(validate(
            &graph_with_anonymous(canvas),
            &HashSet::new(),
            &HashSet::new(),
            &manifest()
        )
        .is_ok());
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
            &HashSet::new(),
            &manifest(),
        ));
        assert!(rule_ids(&details).contains(&"processor_field_option"));
    }

    // 21. Unknown extra fields are ignored (forward-compatible), not an error.
    #[test]
    fn unknown_extra_fields_ignored() {
        let canvas = CanvasGraph {
            nodes: vec![
                start("s"),
                Node::Decision {
                    id: "d".to_string(),
                    processor: processor(
                        json!({"type": "device_type", "operator": "equals", "value": "mobile", "future_field": 42}),
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
        assert!(validate(
            &graph_with_anonymous(canvas),
            &HashSet::new(),
            &HashSet::new(),
            &manifest()
        )
        .is_ok());
    }

    // 22. all_paths_reach_end: a reachable dead-end decision fails (no path to end).
    #[test]
    fn dead_end_reachable_fails() {
        let canvas = CanvasGraph {
            nodes: vec![start("s"), decision("d1"), end("e")],
            edges: vec![edge("e0", "s", "d1", Branch::Yes)],
            root_node_id: Some("s".to_string()),
        };
        let details = details_of(validate(
            &graph_with_anonymous(canvas),
            &HashSet::new(),
            &HashSet::new(),
            &manifest(),
        ));
        assert!(rule_ids(&details).contains(&"all_paths_reach_end"));
        assert!(details.iter().any(
            |d| d.rule_id == "all_paths_reach_end" && d.msg == "node 'd1' cannot reach an end"
        ));
    }

    // 23. all_paths_reach_end: start -> decision -> decision -> end chain passes.
    #[test]
    fn decision_chain_to_end_passes() {
        let canvas = CanvasGraph {
            nodes: vec![start("s"), decision("d1"), decision("d2"), end("e")],
            edges: vec![
                edge("e0", "s", "d1", Branch::Yes),
                edge("e1", "d1", "d2", Branch::Yes),
                edge("e2", "d2", "e", Branch::Yes),
            ],
            root_node_id: Some("s".to_string()),
        };
        assert!(validate(
            &graph_with_anonymous(canvas),
            &HashSet::new(),
            &HashSet::new(),
            &manifest()
        )
        .is_ok());
    }

    // 24. start -> expression(apply_outcome) -> end passes (expression action
    //     validated against the manifest + apply_outcome_ref_exists).
    #[test]
    fn start_expression_end_passes() {
        let oid = Uuid::new_v4();
        let canvas = CanvasGraph {
            nodes: vec![start("s"), expression_apply("a", oid), end("e")],
            edges: vec![
                edge("e0", "s", "a", Branch::Yes),
                edge("e1", "a", "e", Branch::Yes),
            ],
            root_node_id: Some("s".to_string()),
        };
        let mut ids = HashSet::new();
        ids.insert(oid);
        assert!(validate(
            &graph_with_anonymous(canvas),
            &ids,
            &HashSet::new(),
            &manifest()
        )
        .is_ok());
    }

    // 25. start_present: a non-empty canvas without a start node fails.
    #[test]
    fn missing_start_fails() {
        let canvas = CanvasGraph {
            nodes: vec![decision("d1"), end("e")],
            edges: vec![edge("e1", "d1", "e", Branch::Yes)],
            root_node_id: None,
        };
        let details = details_of(validate(
            &graph_with_anonymous(canvas),
            &HashSet::new(),
            &HashSet::new(),
            &manifest(),
        ));
        assert!(rule_ids(&details).contains(&"start_present"));
    }

    // 26. start_present: two start nodes fail.
    #[test]
    fn two_starts_fail() {
        let canvas = CanvasGraph {
            nodes: vec![start("s1"), start("s2"), end("e")],
            edges: vec![
                edge("e1", "s1", "e", Branch::Yes),
                edge("e2", "s2", "e", Branch::Yes),
            ],
            root_node_id: None,
        };
        let details = details_of(validate(
            &graph_with_anonymous(canvas),
            &HashSet::new(),
            &HashSet::new(),
            &manifest(),
        ));
        assert!(rule_ids(&details).contains(&"start_present"));
    }

    // 27. end_present: a non-empty canvas without an end node fails.
    #[test]
    fn missing_end_fails() {
        let canvas = CanvasGraph {
            nodes: vec![start("s"), decision("d1")],
            edges: vec![edge("e0", "s", "d1", Branch::Yes)],
            root_node_id: None,
        };
        let details = details_of(validate(
            &graph_with_anonymous(canvas),
            &HashSet::new(),
            &HashSet::new(),
            &manifest(),
        ));
        assert!(rule_ids(&details).contains(&"end_present"));
    }

    // 28. start_no_incoming: an edge targeting the start node fails.
    #[test]
    fn start_with_incoming_fails() {
        let canvas = CanvasGraph {
            nodes: vec![start("s"), decision("d1"), end("e")],
            edges: vec![
                edge("e0", "s", "d1", Branch::Yes),
                edge("e1", "d1", "e", Branch::Yes),
                edge("e2", "d1", "s", Branch::No),
            ],
            root_node_id: None,
        };
        let details = details_of(validate(
            &graph_with_anonymous(canvas),
            &HashSet::new(),
            &HashSet::new(),
            &manifest(),
        ));
        assert!(rule_ids(&details).contains(&"start_no_incoming"));
    }

    // 29. start_single_out: a start node with zero outgoing edges fails.
    #[test]
    fn start_no_out_fails() {
        let canvas = CanvasGraph {
            nodes: vec![start("s"), end("e")],
            edges: vec![],
            root_node_id: None,
        };
        let details = details_of(validate(
            &graph_with_anonymous(canvas),
            &HashSet::new(),
            &HashSet::new(),
            &manifest(),
        ));
        assert!(rule_ids(&details).contains(&"start_single_out"));
    }

    // 30. expression_single_out: an expression node with two outgoing edges fails.
    #[test]
    fn expression_two_out_fails() {
        let oid = Uuid::new_v4();
        let canvas = CanvasGraph {
            nodes: vec![start("s"), expression_apply("a", oid), end("e1"), end("e2")],
            edges: vec![
                edge("e0", "s", "a", Branch::Yes),
                edge("ea", "a", "e1", Branch::Yes),
                edge("eb", "a", "e2", Branch::No),
            ],
            root_node_id: None,
        };
        let mut ids = HashSet::new();
        ids.insert(oid);
        let details = details_of(validate(
            &graph_with_anonymous(canvas),
            &ids,
            &HashSet::new(),
            &manifest(),
        ));
        assert!(rule_ids(&details).contains(&"expression_single_out"));
    }

    // 31. processor checks run on expression actions: an unknown action type fails
    //     processor_kind_known.
    #[test]
    fn expression_unknown_action_kind_fails() {
        let canvas = CanvasGraph {
            nodes: vec![
                start("s"),
                Node::Expression {
                    id: "a".to_string(),
                    action: processor(json!({"type": "not_a_real_action"})),
                    custom_label: None,
                    position: pos(),
                },
                end("e"),
            ],
            edges: vec![
                edge("e0", "s", "a", Branch::Yes),
                edge("e1", "a", "e", Branch::Yes),
            ],
            root_node_id: None,
        };
        let details = details_of(validate(
            &graph_with_anonymous(canvas),
            &HashSet::new(),
            &HashSet::new(),
            &manifest(),
        ));
        assert!(rule_ids(&details).contains(&"processor_kind_known"));
    }

    // 32. processor_field_required on outcome_select: apply_outcome with an empty
    //     outcome_id fails processor_field_required (the field is required).
    #[test]
    fn apply_outcome_empty_outcome_id_fails_required() {
        let canvas = CanvasGraph {
            nodes: vec![
                start("s"),
                Node::Expression {
                    id: "a".to_string(),
                    action: processor(json!({"type": "apply_outcome", "outcome_id": ""})),
                    custom_label: None,
                    position: pos(),
                },
                end("e"),
            ],
            edges: vec![
                edge("e0", "s", "a", Branch::Yes),
                edge("e1", "a", "e", Branch::Yes),
            ],
            root_node_id: None,
        };
        let details = details_of(validate(
            &graph_with_anonymous(canvas),
            &HashSet::new(),
            &HashSet::new(),
            &manifest(),
        ));
        assert!(rule_ids(&details).contains(&"processor_field_required"));
    }

    // 33. trim_json expression: a valid trim_json action passes (number field).
    #[test]
    fn trim_json_expression_valid() {
        let canvas = CanvasGraph {
            nodes: vec![
                start("s"),
                Node::Expression {
                    id: "t".to_string(),
                    action: processor(
                        json!({"type": "trim_json", "json_path": "$.body", "length": 0}),
                    ),
                    custom_label: None,
                    position: pos(),
                },
                end("e"),
            ],
            edges: vec![
                edge("e0", "s", "t", Branch::Yes),
                edge("e1", "t", "e", Branch::Yes),
            ],
            root_node_id: Some("s".to_string()),
        };
        assert!(validate(
            &graph_with_anonymous(canvas),
            &HashSet::new(),
            &HashSet::new(),
            &manifest()
        )
        .is_ok());
    }
}
