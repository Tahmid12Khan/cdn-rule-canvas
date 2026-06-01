//! `ArticleUrlProcessor` (kind = "article_url"). Compares `ctx.request_path`
//! against the configured value using contains / matches (regex) / starts_with /
//! equals. The `value` is length-capped (<= 1000) and, for `matches`, compiled
//! as a regex (an invalid pattern is a config error, never a panic).

use regex::Regex;
use serde_json::Value;

use crate::domain::context::EvaluationContext;

use super::{Branch, CanvasProcessor, ProcessorError, ProcessorOutcome};

const MAX_PATTERN_LEN: usize = 1000;

#[derive(Debug)]
pub struct ArticleUrlProcessor;

impl CanvasProcessor for ArticleUrlProcessor {
    fn kind(&self) -> &'static str {
        "article_url"
    }

    fn evaluate(
        &self,
        config: &Value,
        ctx: &EvaluationContext,
    ) -> Result<ProcessorOutcome, ProcessorError> {
        let operator = config
            .get("operator")
            .and_then(Value::as_str)
            .ok_or_else(|| ProcessorError::Config("missing operator".to_string()))?;
        let value = config
            .get("value")
            .and_then(Value::as_str)
            .ok_or_else(|| ProcessorError::Config("missing value".to_string()))?;
        if value.len() > MAX_PATTERN_LEN {
            return Err(ProcessorError::Config(format!(
                "value exceeds {MAX_PATTERN_LEN} chars"
            )));
        }

        let url = ctx.request_path.as_str();

        let hit = match operator {
            "equals" => url == value,
            "contains" => url.contains(value),
            "starts_with" => url.starts_with(value),
            "matches" => {
                let re = Regex::new(value)
                    .map_err(|e| ProcessorError::Config(format!("invalid regex: {e}")))?;
                re.is_match(url)
            }
            other => return Err(ProcessorError::Config(format!("unknown operator: {other}"))),
        };

        Ok(ProcessorOutcome {
            branch: if hit { Branch::Yes } else { Branch::No },
        })
    }
}
