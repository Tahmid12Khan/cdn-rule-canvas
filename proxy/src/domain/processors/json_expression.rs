//! `JsonExpressionProcessor` (kind = "json_expression"). Queries a JSONPath in
//! the response JSON body (`ctx.response_json`) and compares the matched node(s)
//! against the configured value with one of: equals / contains / starts_with /
//! ends_with / is_one_of / exists.
//!
//! No body (HTML response, or unparseable JSON) -> `Branch::No` (a JSON node on
//! an HTML response is simply inert). A bad JSONPath is a `ProcessorError::Config`
//! (fail-open at the evaluator boundary), never a panic. The `json_path` input is
//! length-capped (<= 1000) before it reaches the parser.

use serde_json::Value;
use serde_json_path::JsonPath;

use crate::domain::context::EvaluationContext;

use super::{Branch, CanvasProcessor, ProcessorError, ProcessorOutcome};

const MAX_PATH_LEN: usize = 1000;

#[derive(Debug)]
pub struct JsonExpressionProcessor;

impl CanvasProcessor for JsonExpressionProcessor {
    fn kind(&self) -> &'static str {
        "json_expression"
    }

    fn evaluate(
        &self,
        config: &Value,
        ctx: &EvaluationContext,
    ) -> Result<ProcessorOutcome, ProcessorError> {
        let json_path = config
            .get("json_path")
            .and_then(Value::as_str)
            .ok_or_else(|| ProcessorError::Config("missing json_path".to_string()))?;
        let operator = config
            .get("operator")
            .and_then(Value::as_str)
            .ok_or_else(|| ProcessorError::Config("missing operator".to_string()))?;
        // `value` is optional (only `exists` needs no value).
        let want = config.get("value").and_then(Value::as_str).unwrap_or("");

        if json_path.len() > MAX_PATH_LEN {
            return Err(ProcessorError::Config(format!(
                "json_path exceeds {MAX_PATH_LEN} chars"
            )));
        }

        // No JSON body (HTML response or unparseable JSON) -> No, never an error.
        let Some(body) = ctx.response_json.as_ref() else {
            return Ok(ProcessorOutcome { branch: Branch::No });
        };

        // Parse the path; a malformed path is a typed config error (fail-open).
        let path = JsonPath::parse(json_path)
            .map_err(|e| ProcessorError::Config(format!("invalid json_path: {e}")))?;

        let matched: Vec<&Value> = path.query(body).all();

        let hit = match operator {
            "exists" => !matched.is_empty(),
            "equals" => matched.iter().any(|n| node_string(n) == want),
            "contains" => matched.iter().any(|n| node_string(n).contains(want)),
            "starts_with" => matched.iter().any(|n| node_string(n).starts_with(want)),
            "ends_with" => matched.iter().any(|n| node_string(n).ends_with(want)),
            "is_one_of" => {
                let set: Vec<&str> = want.split(',').map(str::trim).collect();
                matched
                    .iter()
                    .any(|n| set.contains(&node_string(n).as_str()))
            }
            other => return Err(ProcessorError::Config(format!("unknown operator: {other}"))),
        };

        Ok(ProcessorOutcome {
            branch: if hit { Branch::Yes } else { Branch::No },
        })
    }
}

/// Stringify a matched JSON node: strings use their raw value, everything else
/// (numbers, bools, null, objects, arrays) uses its JSON `to_string`.
fn node_string(node: &Value) -> String {
    match node.as_str() {
        Some(s) => s.to_string(),
        None => node.to_string(),
    }
}
