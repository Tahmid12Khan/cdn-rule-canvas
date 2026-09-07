//! The `rre` telemetry block: per-feature expression timings, injected into the
//! response so the reader's own body carries the debug view (spec §2/v2.2).
//!
//! Lives HERE, not in a host, because both hosts must emit the SAME shape: the
//! proxy injects it from `forwarder.rs`, the Fastly guest gets it out of the box
//! from [`crate::edge::apply`]. A second copy of this shape would drift, and the
//! whole point of the block is that what you read in the app is what the POP
//! did.
//!
//! All times are STRINGS formatted `d.dd` (spec §3) so the trailing-zero format
//! survives JSON (`0.10`, not `0.1`).

use serde::Serialize;

use crate::graph::{CanvasGraph, Node};

/// One expression node's measured apply time (spec §4): canvas node id, display
/// label, optional custom label (spec v2.3), and its individual apply duration.
#[derive(Clone, Debug)]
pub struct NodeTiming {
    pub node_id: String,
    pub label: String,
    pub custom_label: Option<String>,
    pub time_ms: f64,
}

/// One expression node on the matched path (spec v2.2). Used for both the
/// `expressions` list (last-10, in order) and `expensive_nodes` (top-3 by time).
#[derive(Serialize, Clone, Debug)]
pub struct Expression {
    pub expression_id: String,
    pub expression_label: String,
    /// The node's `custom_label` (spec v2.3), `""` when unset.
    pub custom_expression_label: String,
    /// Per-node apply time, `d.dd`.
    pub expression_time_ms: String,
}

/// A single feature's `feature_expressions` entry (spec v2.2). Also the
/// `/__rre/eval` `summary` for the single canvas under test (spec §5).
#[derive(Serialize, Clone, Debug)]
pub struct FeatureEntry {
    /// The `version_number` of the active version used to evaluate this feature.
    /// `Some` for the production injection (the served version); `None` for the
    /// `/__rre/eval(-url)` test panel (a posted canvas has no saved version) →
    /// omitted from the JSON.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<i32>,
    /// Last-10 expression nodes the matched path traversed, in order.
    pub expressions: Vec<Expression>,
    /// Whole-feature time (`eval_ms + sum(per-node apply times)`), `d.dd`.
    pub time_took_ms: String,
    /// Expression nodes sorted by per-node apply time DESC, top 3.
    pub expensive_nodes: Vec<Expression>,
}

/// Format a millisecond value as the LOCKED `d.dd` string (spec §3).
pub fn fmt_ms(ms: f64) -> String {
    format!("{ms:.2}")
}

/// The expression node's display label and custom label, read off the canvas.
/// An id that is not an expression node falls back to the id itself.
pub fn expression_label(canvas: &CanvasGraph, node_id: &str) -> (String, Option<String>) {
    match canvas.nodes.iter().find(|n| n.id() == node_id) {
        Some(Node::Expression {
            action,
            custom_label,
            ..
        }) => (action.kind.clone(), custom_label.clone()),
        _ => (node_id.to_string(), None),
    }
}

/// Map a `NodeTiming` to the wire `Expression` shape (spec v2.2). The custom
/// label is trimmed; `""` when `None` or empty.
fn to_expression(t: &NodeTiming) -> Expression {
    Expression {
        expression_id: t.node_id.clone(),
        expression_label: t.label.clone(),
        custom_expression_label: t
            .custom_label
            .as_deref()
            .map(str::trim)
            .unwrap_or_default()
            .to_string(),
        expression_time_ms: fmt_ms(t.time_ms),
    }
}

/// Build a feature's entry from the per-node expression timings (in traversal
/// order), the feature's eval duration, and the served active `version` (`None`
/// for the test panel — a posted canvas has no saved version). Returns `None`
/// when no expression node was applied (the feature did NOT match — no entry).
pub fn build_entry(
    timings: &[NodeTiming],
    eval_ms: f64,
    version: Option<i32>,
) -> Option<FeatureEntry> {
    if timings.is_empty() {
        return None;
    }

    // expressions: traversal order, LAST 10.
    let start = timings.len().saturating_sub(10);
    let expressions: Vec<Expression> = timings[start..].iter().map(to_expression).collect();

    // time_took_ms = eval_ms + sum of every per-node apply time (not just last 10).
    let sum_ms: f64 = timings.iter().map(|t| t.time_ms).sum();
    let time_took_ms = fmt_ms(eval_ms + sum_ms);

    // expensive_nodes: sort by per-node time DESC, top 3.
    let mut by_time: Vec<&NodeTiming> = timings.iter().collect();
    by_time.sort_by(|a, b| {
        b.time_ms
            .partial_cmp(&a.time_ms)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    let expensive_nodes: Vec<Expression> =
        by_time.iter().take(3).map(|t| to_expression(t)).collect();

    Some(FeatureEntry {
        version,
        expressions,
        time_took_ms,
        expensive_nodes,
    })
}

/// Merge the matched features' entries into `body["rre"]["feature_expressions"]`
/// (spec v2.2) and set `body["rre"]["total_time_ms"]` + `["compute_time_ms"]`
/// (spec item 7) as top-level SIBLINGS. Creates the `rre` object if absent; sets
/// the keys without clobbering other `rre.*` keys. A non-object `body` is left
/// untouched.
pub fn inject_json(
    body: &mut serde_json::Value,
    matched: &[(String, FeatureEntry)],
    total_time_ms: &str,
    compute_time_ms: &str,
) {
    let Some(root) = body.as_object_mut() else {
        return; // top-level non-object body: nothing to namespace under.
    };
    let rre = root
        .entry("rre")
        .or_insert_with(|| serde_json::Value::Object(serde_json::Map::new()));
    // If `rre` exists but is not an object, replace it (reserved namespace).
    if !rre.is_object() {
        *rre = serde_json::Value::Object(serde_json::Map::new());
    }
    let rre_obj = rre.as_object_mut().expect("just ensured object");
    let fe: serde_json::Map<String, serde_json::Value> = matched
        .iter()
        .map(|(id, entry)| (id.clone(), serde_json::to_value(entry).unwrap_or_default()))
        .collect();
    rre_obj.insert(
        "feature_expressions".to_string(),
        serde_json::Value::Object(fe),
    );
    rre_obj.insert(
        "total_time_ms".to_string(),
        serde_json::Value::String(total_time_ms.to_string()),
    );
    rre_obj.insert(
        "compute_time_ms".to_string(),
        serde_json::Value::String(compute_time_ms.to_string()),
    );
}

/// The `<script>` that carries the same block on an HTML response.
///
/// Every `<` in the serialized payload is escaped to `\u003c` so an embedded
/// `</script>` cannot break out of the tag. `window.rre.total_time_ms` /
/// `compute_time_ms` are top-level SIBLINGS of `feature_expressions` (spec
/// item 7).
pub fn html_script(
    matched: &[(String, FeatureEntry)],
    total_time_ms: &str,
    compute_time_ms: &str,
) -> String {
    let map: serde_json::Map<String, serde_json::Value> = matched
        .iter()
        .map(|(id, entry)| (id.clone(), serde_json::to_value(entry).unwrap_or_default()))
        .collect();
    let json = serde_json::to_string(&serde_json::Value::Object(map))
        .unwrap_or_else(|_| "{}".to_string())
        .replace('<', "\\u003c");
    // The time fields are `d.dd` strings; serialize so each is a quoted JSON
    // string (escaping any `<` for the same break-out safety).
    let total_json = serde_json::to_string(total_time_ms)
        .unwrap_or_else(|_| "\"0.00\"".to_string())
        .replace('<', "\\u003c");
    let compute_json = serde_json::to_string(compute_time_ms)
        .unwrap_or_else(|_| "\"0.00\"".to_string())
        .replace('<', "\\u003c");
    format!(
        "<script>window.rre=window.rre||{{}};window.rre.feature_expressions={json};\
         window.rre.total_time_ms={total_json};window.rre.compute_time_ms={compute_json};</script>"
    )
}

/// Insert [`html_script`] immediately before `</body>`, or at the end of the
/// document when there is none.
pub fn inject_html(
    body: String,
    matched: &[(String, FeatureEntry)],
    total_time_ms: &str,
    compute_time_ms: &str,
) -> String {
    let script = html_script(matched, total_time_ms, compute_time_ms);
    match body.rfind("</body>") {
        Some(idx) => {
            let mut out = String::with_capacity(body.len() + script.len());
            out.push_str(&body[..idx]);
            out.push_str(&script);
            out.push_str(&body[idx..]);
            out
        }
        None => body + &script,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn timing(id: &str, ms: f64) -> NodeTiming {
        NodeTiming {
            node_id: id.to_string(),
            label: id.to_string(),
            custom_label: None,
            time_ms: ms,
        }
    }

    #[test]
    fn no_timings_means_no_entry() {
        assert!(build_entry(&[], 1.0, Some(1)).is_none());
    }

    /// The format is LOCKED at two decimals: `0.10`, never `0.1`.
    #[test]
    fn times_are_d_dd_strings() {
        let e = build_entry(&[timing("a", 0.1)], 0.0, Some(8)).unwrap();
        assert_eq!(e.expressions[0].expression_time_ms, "0.10");
        assert_eq!(e.time_took_ms, "0.10");
        assert_eq!(e.version, Some(8));
    }

    #[test]
    fn expensive_nodes_are_top_three_desc() {
        let t = vec![
            timing("a", 1.0),
            timing("b", 5.0),
            timing("c", 3.0),
            timing("d", 4.0),
        ];
        let e = build_entry(&t, 0.0, None).unwrap();
        let ids: Vec<&str> = e
            .expensive_nodes
            .iter()
            .map(|x| x.expression_id.as_str())
            .collect();
        assert_eq!(ids, vec!["b", "d", "c"]);
        assert_eq!(e.version, None);
    }

    /// `time_took_ms` counts EVERY node, not just the last ten.
    #[test]
    fn time_took_sums_all_nodes_plus_eval() {
        let t: Vec<NodeTiming> = (0..12).map(|i| timing(&format!("n{i}"), 1.0)).collect();
        let e = build_entry(&t, 2.0, Some(1)).unwrap();
        assert_eq!(e.expressions.len(), 10);
        assert_eq!(e.time_took_ms, "14.00");
    }

    #[test]
    fn inject_json_creates_the_rre_namespace() {
        let entry = build_entry(&[timing("a", 1.0)], 0.0, Some(8)).unwrap();
        let mut body = serde_json::json!({"title": "x"});
        inject_json(&mut body, &[("f1".to_string(), entry)], "9.00", "4.00");
        assert_eq!(body["rre"]["total_time_ms"], "9.00");
        assert_eq!(body["rre"]["compute_time_ms"], "4.00");
        assert_eq!(body["rre"]["feature_expressions"]["f1"]["version"], 8);
        assert_eq!(body["title"], "x");
    }

    /// Other `rre.*` keys a host already set must survive.
    #[test]
    fn inject_json_does_not_clobber_other_rre_keys() {
        let entry = build_entry(&[timing("a", 1.0)], 0.0, None).unwrap();
        let mut body = serde_json::json!({"rre": {"kept": true}});
        inject_json(&mut body, &[("f1".to_string(), entry)], "1.00", "1.00");
        assert_eq!(body["rre"]["kept"], true);
        assert!(body["rre"]["feature_expressions"]["f1"].is_object());
    }

    #[test]
    fn inject_json_leaves_a_non_object_body_alone() {
        let entry = build_entry(&[timing("a", 1.0)], 0.0, None).unwrap();
        let mut body = serde_json::json!([1, 2, 3]);
        inject_json(&mut body, &[("f1".to_string(), entry)], "1.00", "1.00");
        assert_eq!(body, serde_json::json!([1, 2, 3]));
    }

    #[test]
    fn html_script_carries_the_same_three_keys() {
        let entry = build_entry(&[timing("a", 1.0)], 0.0, Some(8)).unwrap();
        let s = html_script(&[("f1".to_string(), entry)], "9.00", "4.00");
        assert!(s.contains("window.rre.feature_expressions="));
        assert!(s.contains("window.rre.total_time_ms=\"9.00\""));
        assert!(s.contains("window.rre.compute_time_ms=\"4.00\""));
    }

    /// A `<` inside the payload must not be able to close the script tag.
    #[test]
    fn html_script_escapes_angle_brackets() {
        let mut t = timing("a", 1.0);
        t.custom_label = Some("</script><img src=x>".to_string());
        let entry = build_entry(&[t], 0.0, None).unwrap();
        let s = html_script(&[("f1".to_string(), entry)], "1.00", "1.00");
        assert!(!s.contains("</script><img"), "must be escaped: {s}");
        assert!(s.contains("\\u003c/script"));
    }

    #[test]
    fn inject_html_goes_before_the_closing_body_tag() {
        let entry = build_entry(&[timing("a", 1.0)], 0.0, Some(8)).unwrap();
        let out = inject_html(
            "<html><body><p>x</p></body></html>".to_string(),
            &[("f1".to_string(), entry)],
            "9.00",
            "4.00",
        );
        let script_at = out.find("<script>window.rre").unwrap();
        assert!(script_at < out.find("</body>").unwrap());
        assert!(out.ends_with("</body></html>"));
    }

    /// No `</body>` (a JSON-ish or fragment response) still gets the block.
    #[test]
    fn inject_html_appends_when_there_is_no_body_tag() {
        let entry = build_entry(&[timing("a", 1.0)], 0.0, None).unwrap();
        let out = inject_html(
            "<p>x</p>".to_string(),
            &[("f1".to_string(), entry)],
            "1.00",
            "1.00",
        );
        assert!(out.starts_with("<p>x</p><script>window.rre"));
    }
}
