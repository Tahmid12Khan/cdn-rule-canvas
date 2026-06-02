//! The `features_matched` per-feature entry shape (spec §2), shared by the
//! forwarder (runtime header + body injection) and the `/__rre/eval` test panel
//! (`EvalResponse.summary`). A "matched" feature is one whose evaluated path
//! produced >=1 expression action — i.e. `outcome_ids` is non-empty.
//!
//! All times are STRINGS formatted `d.dd` (spec §3) so the trailing-zero format
//! survives JSON (`0.10`, not `0.1`).

use serde::Serialize;

/// One expression node's measured apply time (spec §4): canvas node id, display
/// label, and its individual apply duration in ms.
#[derive(Clone, Debug)]
pub struct NodeTiming {
    pub node_id: String,
    pub label: String,
    pub time_ms: f64,
}

/// One of the top-3 slowest expression nodes (spec §2 `expensive_nodes`).
#[derive(Serialize, Clone, Debug)]
pub struct ExpensiveNode {
    pub outcome_id: String,
    pub outcome_label: String,
    /// Per-node apply time, `d.dd`.
    pub outcome_time_in_ms: String,
}

/// A single feature's `features_matched` entry (spec §2). Also the `/__rre/eval`
/// `summary` for the single canvas under test (spec §5).
#[derive(Serialize, Clone, Debug)]
pub struct FeatureEntry {
    /// Last-10 expression node ids the matched path traversed, in order.
    pub outcome_ids: Vec<String>,
    /// Parallel labels, equal length.
    pub outcome_labels: Vec<String>,
    /// Whole-feature time (`eval_ms + sum(per-node apply times)`), `d.dd`.
    pub time_took: String,
    /// Expression nodes sorted by per-node apply time DESC, top 3.
    pub expensive_nodes: Vec<ExpensiveNode>,
}

/// Format a millisecond value as the LOCKED `d.dd` string (spec §3).
pub fn fmt_ms(ms: f64) -> String {
    format!("{ms:.2}")
}

/// Build a feature's entry from the per-node expression timings (in traversal
/// order) and the feature's eval duration. Returns `None` when no expression
/// node was applied (the feature did NOT match — no header, no entry).
pub fn build_entry(timings: &[NodeTiming], eval_ms: f64) -> Option<FeatureEntry> {
    if timings.is_empty() {
        return None;
    }

    // outcome_ids / outcome_labels: traversal order, LAST 10, parallel arrays.
    let start = timings.len().saturating_sub(10);
    let last10 = &timings[start..];
    let outcome_ids: Vec<String> = last10.iter().map(|t| t.node_id.clone()).collect();
    let outcome_labels: Vec<String> = last10.iter().map(|t| t.label.clone()).collect();

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
    let expensive_nodes: Vec<ExpensiveNode> = by_time
        .iter()
        .take(3)
        .map(|t| ExpensiveNode {
            outcome_id: t.node_id.clone(),
            outcome_label: t.label.clone(),
            outcome_time_in_ms: fmt_ms(t.time_ms),
        })
        .collect();

    Some(FeatureEntry {
        outcome_ids,
        outcome_labels,
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
        // 5 nodes; outcome_ids keep traversal order, expensive_nodes sort DESC.
        let timings = vec![
            timing("a", 0.5),
            timing("b", 3.0),
            timing("c", 1.0),
            timing("d", 2.0),
            timing("e", 0.25),
        ];
        let entry = build_entry(&timings, 0.0).unwrap();

        // outcome_ids: traversal order (here all 5, since <=10).
        assert_eq!(entry.outcome_ids, vec!["a", "b", "c", "d", "e"]);

        // time_took = eval_ms(0) + sum = 6.75.
        assert_eq!(entry.time_took, "6.75");

        // expensive_nodes: top 3 DESC -> b(3.0), d(2.0), c(1.0).
        let ids: Vec<&str> = entry
            .expensive_nodes
            .iter()
            .map(|n| n.outcome_id.as_str())
            .collect();
        assert_eq!(ids, vec!["b", "d", "c"]);
        assert_eq!(entry.expensive_nodes[0].outcome_time_in_ms, "3.00");
        assert_eq!(entry.expensive_nodes.len(), 3);
    }

    #[test]
    fn outcome_ids_keep_last_10() {
        let timings: Vec<NodeTiming> = (0..12).map(|i| timing(&format!("n{i}"), 1.0)).collect();
        let entry = build_entry(&timings, 0.0).unwrap();
        assert_eq!(entry.outcome_ids.len(), 10);
        assert_eq!(entry.outcome_ids.first().unwrap(), "n2"); // last 10 = n2..n11
        assert_eq!(entry.outcome_ids.last().unwrap(), "n11");
        assert_eq!(entry.outcome_ids.len(), entry.outcome_labels.len());
        // time_took still sums ALL 12 nodes (not just last 10).
        assert_eq!(entry.time_took, "12.00");
    }
}
