//! The `feature_expressions` per-feature entry shape (spec §2/v2.2), shared by
//! the forwarder (runtime header + body injection) and the `/__rre/eval` test
//! panel (`EvalResponse.summary`). A "matched" feature is one whose evaluated
//! path produced >=1 expression action — i.e. `expressions` is non-empty.
//!
//! All times are STRINGS formatted `d.dd` (spec §3) so the trailing-zero format
//! survives JSON (`0.10`, not `0.1`).

use serde::Serialize;

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
    pub expression_time_in_ms: String,
}

/// A single feature's `feature_expressions` entry (spec v2.2). Also the
/// `/__rre/eval` `summary` for the single canvas under test (spec §5).
#[derive(Serialize, Clone, Debug)]
pub struct FeatureEntry {
    /// Last-10 expression nodes the matched path traversed, in order.
    pub expressions: Vec<Expression>,
    /// Whole-feature time (`eval_ms + sum(per-node apply times)`), `d.dd`.
    pub time_took: String,
    /// Expression nodes sorted by per-node apply time DESC, top 3.
    pub expensive_nodes: Vec<Expression>,
}

/// Format a millisecond value as the LOCKED `d.dd` string (spec §3).
pub fn fmt_ms(ms: f64) -> String {
    format!("{ms:.2}")
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
        expression_time_in_ms: fmt_ms(t.time_ms),
    }
}

/// Build a feature's entry from the per-node expression timings (in traversal
/// order) and the feature's eval duration. Returns `None` when no expression
/// node was applied (the feature did NOT match — no header, no entry).
pub fn build_entry(timings: &[NodeTiming], eval_ms: f64) -> Option<FeatureEntry> {
    if timings.is_empty() {
        return None;
    }

    // expressions: traversal order, LAST 10.
    let start = timings.len().saturating_sub(10);
    let expressions: Vec<Expression> = timings[start..].iter().map(to_expression).collect();

    // time_took = eval_ms + sum of every per-node apply time (not just last 10).
    let sum_ms: f64 = timings.iter().map(|t| t.time_ms).sum();
    let time_took = fmt_ms(eval_ms + sum_ms);

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
        expressions,
        time_took,
        expensive_nodes,
    })
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
    fn empty_timings_is_not_matched() {
        assert!(build_entry(&[], 1.0).is_none());
    }

    #[test]
    fn fmt_ms_keeps_two_decimals() {
        assert_eq!(fmt_ms(0.1), "0.10");
        assert_eq!(fmt_ms(1.2), "1.20");
        assert_eq!(fmt_ms(123.0), "123.00");
        assert_eq!(fmt_ms(123.125), "123.12"); // round-half-to-even
    }

    #[test]
    fn expensive_nodes_desc_top3_and_last10_in_order() {
        // 5 nodes; expressions keep traversal order, expensive_nodes sort DESC.
        let timings = vec![
            timing("a", 0.5),
            timing("b", 3.0),
            timing("c", 1.0),
            timing("d", 2.0),
            timing("e", 0.25),
        ];
        let entry = build_entry(&timings, 0.0).unwrap();

        // expressions: traversal order (here all 5, since <=10).
        let ids: Vec<&str> = entry
            .expressions
            .iter()
            .map(|e| e.expression_id.as_str())
            .collect();
        assert_eq!(ids, vec!["a", "b", "c", "d", "e"]);

        // time_took = eval_ms(0) + sum = 6.75.
        assert_eq!(entry.time_took, "6.75");

        // expensive_nodes: top 3 DESC -> b(3.0), d(2.0), c(1.0).
        let ids: Vec<&str> = entry
            .expensive_nodes
            .iter()
            .map(|n| n.expression_id.as_str())
            .collect();
        assert_eq!(ids, vec!["b", "d", "c"]);
        assert_eq!(entry.expensive_nodes[0].expression_time_in_ms, "3.00");
        assert_eq!(entry.expensive_nodes.len(), 3);
    }

    #[test]
    fn expressions_keep_last_10() {
        let timings: Vec<NodeTiming> = (0..12).map(|i| timing(&format!("n{i}"), 1.0)).collect();
        let entry = build_entry(&timings, 0.0).unwrap();
        assert_eq!(entry.expressions.len(), 10);
        assert_eq!(entry.expressions.first().unwrap().expression_id, "n2"); // last 10 = n2..n11
        assert_eq!(entry.expressions.last().unwrap().expression_id, "n11");
        // time_took still sums ALL 12 nodes (not just last 10).
        assert_eq!(entry.time_took, "12.00");
    }

    #[test]
    fn custom_label_emitted_trimmed_else_empty() {
        let with = NodeTiming {
            node_id: "a".to_string(),
            label: "trim_json".to_string(),
            custom_label: Some("  trim_body  ".to_string()),
            time_ms: 1.0,
        };
        let without = timing("b", 1.0);
        let entry = build_entry(&[with, without], 0.0).unwrap();
        assert_eq!(entry.expressions[0].custom_expression_label, "trim_body");
        assert_eq!(entry.expressions[1].custom_expression_label, "");
    }
}
