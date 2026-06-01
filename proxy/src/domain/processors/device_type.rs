//! `DeviceTypeProcessor` (kind = "device_type"). Reads `ctx.device` (computed
//! once from the User-Agent) and applies the configured operator/value.

use serde_json::Value;

use crate::domain::context::EvaluationContext;

use super::{Branch, CanvasProcessor, ProcessorError, ProcessorOutcome};

#[derive(Debug)]
pub struct DeviceTypeProcessor;

impl CanvasProcessor for DeviceTypeProcessor {
    fn kind(&self) -> &'static str {
        "device_type"
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

        let device = ctx.device.as_str();

        let branch = match operator {
            "equals" => {
                if device == value {
                    Branch::Yes
                } else {
                    Branch::No
                }
            }
            "contains" => {
                if device.contains(value) {
                    Branch::Yes
                } else {
                    Branch::No
                }
            }
            other => return Err(ProcessorError::Config(format!("unknown operator: {other}"))),
        };

        Ok(ProcessorOutcome { branch })
    }
}
