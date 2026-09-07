//! `MetaTagsProcessor` (kind = "meta_tags"). Looks `tag_name` up in
//! `ctx.meta_tags` (pre-extracted `<meta name=… content=…>` map) and inspects the
//! `content` value.
//!
//! `tag_name` is still length-capped (<= 200) and char-whitelisted (defensive
//! input validation). A miss fails open to `Branch::No` with a WARN log.

use serde_json::Value;

use crate::context::EvaluationContext;

use super::{Branch, CanvasProcessor, ProcessorError, ProcessorOutcome};

const MAX_SELECTOR_INPUT: usize = 200;

/// Whitelist of characters permitted in user-authored selector fragments.
fn is_allowed_selector_char(c: char) -> bool {
    c.is_ascii_alphanumeric()
        || matches!(
            c,
            '_' | '-' | '[' | ']' | '=' | '"' | '\'' | '.' | '#' | ':' | ' '
        )
}

/// Validate untrusted selector input: length cap + char whitelist.
fn validate_selector_input(input: &str) -> Result<(), ProcessorError> {
    if input.is_empty() {
        return Err(ProcessorError::Config("tag_name is empty".to_string()));
    }
    if input.len() > MAX_SELECTOR_INPUT {
        return Err(ProcessorError::Config(format!(
            "tag_name exceeds {MAX_SELECTOR_INPUT} chars"
        )));
    }
    if !input.chars().all(is_allowed_selector_char) {
        return Err(ProcessorError::Config(
            "tag_name contains disallowed characters".to_string(),
        ));
    }
    Ok(())
}

#[derive(Debug)]
pub struct MetaTagsProcessor;

impl CanvasProcessor for MetaTagsProcessor {
    fn kind(&self) -> &'static str {
        "meta_tags"
    }

    fn evaluate(
        &self,
        config: &Value,
        ctx: &EvaluationContext,
    ) -> Result<ProcessorOutcome, ProcessorError> {
        // Pull the required fields from the JSON config (never unwrap/panic).
        let operator = config
            .get("operator")
            .and_then(Value::as_str)
            .ok_or_else(|| ProcessorError::Config("missing operator".to_string()))?;
        let tag_name = config
            .get("tag_name")
            .and_then(Value::as_str)
            .ok_or_else(|| ProcessorError::Config("missing tag_name".to_string()))?;
        let want = config.get("value").and_then(Value::as_str);

        validate_selector_input(tag_name)?;

        let content: Option<String> = ctx.meta_tags.get(tag_name).cloned();

        let branch = match operator {
            "exists" => {
                if content.is_some() {
                    Branch::Yes
                } else {
                    tracing::warn!(tag_name, "selector_miss");
                    Branch::No
                }
            }
            "equals" => match (&content, want) {
                (Some(c), Some(w)) if c == w => Branch::Yes,
                _ => {
                    if content.is_none() {
                        tracing::warn!(tag_name, "selector_miss");
                    }
                    Branch::No
                }
            },
            "contains" => match (&content, want) {
                (Some(c), Some(w)) if c.contains(w) => Branch::Yes,
                _ => {
                    if content.is_none() {
                        tracing::warn!(tag_name, "selector_miss");
                    }
                    Branch::No
                }
            },
            other => return Err(ProcessorError::Config(format!("unknown operator: {other}"))),
        };

        Ok(ProcessorOutcome { branch })
    }
}
