//! `ArticleUrlProcessor` (kind = "article_url"). Compares `ctx.request_path`
//! against the configured value using contains / matches (regex) / starts_with /
//! equals. The `value` is length-capped (<= 1000) and, for `matches`, compiled
//! as a regex (an invalid pattern is a config error, never a panic).

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

use regex::Regex;
use serde_json::Value;

use crate::context::EvaluationContext;

use super::{Branch, CanvasProcessor, ProcessorError, ProcessorOutcome};

const MAX_PATTERN_LEN: usize = 1000;

/// Process-wide cache of compiled regexes keyed by pattern string, so the
/// `matches` operator does not recompile the same (length-capped) pattern on
/// every request. An invalid pattern is still surfaced as a config error.
fn compile_cached(pattern: &str) -> Result<Regex, ProcessorError> {
    static CACHE: OnceLock<Mutex<HashMap<String, Regex>>> = OnceLock::new();
    let cache = CACHE.get_or_init(|| Mutex::new(HashMap::new()));
    if let Ok(map) = cache.lock() {
        if let Some(re) = map.get(pattern) {
            return Ok(re.clone());
        }
    }
    let re =
        Regex::new(pattern).map_err(|e| ProcessorError::Config(format!("invalid regex: {e}")))?;
    if let Ok(mut map) = cache.lock() {
        map.insert(pattern.to_string(), re.clone());
    }
    Ok(re)
}

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
                let re = compile_cached(value)?;
                re.is_match(url)
            }
            other => return Err(ProcessorError::Config(format!("unknown operator: {other}"))),
        };

        Ok(ProcessorOutcome {
            branch: if hit { Branch::Yes } else { Branch::No },
        })
    }
}
