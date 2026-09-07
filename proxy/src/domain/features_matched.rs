//! The `feature_expressions` per-feature entry shape (spec §2/v2.2), shared by
//! the forwarder (runtime header + body injection) and the `/__rre/eval` test
//! panel (`EvalResponse.summary`). A "matched" feature is one whose evaluated
//! path produced >=1 expression action — i.e. `expressions` is non-empty.
//!
//! All times are STRINGS formatted `d.dd` (spec §3) so the trailing-zero format
//! survives JSON (`0.10`, not `0.1`).

//! The IMPLEMENTATION now lives in `rre_core::telemetry`, so the proxy and the
//! Fastly guest emit one shape from one place. This module stays as the proxy's
//! name for it; the tests below are the proxy-side contract.

pub use rre_core::telemetry::{build_entry, fmt_ms, Expression, FeatureEntry, NodeTiming};

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
        assert!(build_entry(&[], 1.0, Some(1)).is_none());
    }

    #[test]
    fn version_present_for_production_omitted_for_test_panel() {
        let timings = vec![timing("a", 1.0)];
        // Production: a served version_number is carried.
        let with = build_entry(&timings, 0.0, Some(7)).unwrap();
        assert_eq!(with.version, Some(7));
        // Test panel: no saved version -> omitted (None -> skipped in JSON).
        let without = build_entry(&timings, 0.0, None).unwrap();
        assert_eq!(without.version, None);
        let json = serde_json::to_value(&without).unwrap();
        assert!(
            json.get("version").is_none(),
            "version omitted when None: {json}"
        );
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
        let entry = build_entry(&timings, 0.0, Some(1)).unwrap();

        // expressions: traversal order (here all 5, since <=10).
        let ids: Vec<&str> = entry
            .expressions
            .iter()
            .map(|e| e.expression_id.as_str())
            .collect();
        assert_eq!(ids, vec!["a", "b", "c", "d", "e"]);

        // time_took_ms = eval_ms(0) + sum = 6.75.
        assert_eq!(entry.time_took_ms, "6.75");

        // expensive_nodes: top 3 DESC -> b(3.0), d(2.0), c(1.0).
        let ids: Vec<&str> = entry
            .expensive_nodes
            .iter()
            .map(|n| n.expression_id.as_str())
            .collect();
        assert_eq!(ids, vec!["b", "d", "c"]);
        assert_eq!(entry.expensive_nodes[0].expression_time_ms, "3.00");
        assert_eq!(entry.expensive_nodes.len(), 3);
    }

    #[test]
    fn expressions_keep_last_10() {
        let timings: Vec<NodeTiming> = (0..12).map(|i| timing(&format!("n{i}"), 1.0)).collect();
        let entry = build_entry(&timings, 0.0, Some(1)).unwrap();
        assert_eq!(entry.expressions.len(), 10);
        assert_eq!(entry.expressions.first().unwrap().expression_id, "n2"); // last 10 = n2..n11
        assert_eq!(entry.expressions.last().unwrap().expression_id, "n11");
        // time_took_ms still sums ALL 12 nodes (not just last 10).
        assert_eq!(entry.time_took_ms, "12.00");
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
        let entry = build_entry(&[with, without], 0.0, Some(1)).unwrap();
        assert_eq!(entry.expressions[0].custom_expression_label, "trim_body");
        assert_eq!(entry.expressions[1].custom_expression_label, "");
    }
}
