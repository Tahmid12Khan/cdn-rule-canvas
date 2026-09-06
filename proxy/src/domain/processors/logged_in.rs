//! `LoggedInProcessor` (kind = "logged_in"). Branches on `ctx.identity.logged_in`.
//! No config fields.

use serde_json::Value;

use crate::domain::context::EvaluationContext;

use super::{Branch, CanvasProcessor, ProcessorError, ProcessorOutcome};

#[derive(Debug)]
pub struct LoggedInProcessor;

impl CanvasProcessor for LoggedInProcessor {
    fn kind(&self) -> &'static str {
        "logged_in"
    }

    fn evaluate(
        &self,
        _config: &Value,
        ctx: &EvaluationContext,
    ) -> Result<ProcessorOutcome, ProcessorError> {
        let branch = if ctx.identity.logged_in {
            Branch::Yes
        } else {
            Branch::No
        };
        Ok(ProcessorOutcome { branch })
    }
}

#[cfg(test)]
mod tests {
    use std::collections::{HashMap, HashSet};

    use http::HeaderMap;
    use serde_json::json;

    use crate::domain::context::{DeviceType, EvaluationContext};
    use crate::domain::identity::Identity;

    use super::*;

    fn ctx(logged_in: bool) -> EvaluationContext {
        EvaluationContext {
            request_headers: HeaderMap::new(),
            request_path: "/".to_string(),
            request_cookies: HashMap::new(),
            device: DeviceType::Desktop,
            meta_tags: HashMap::new(),
            response_json: None,
            site: None,
            identity: Identity {
                logged_in,
                products: HashSet::new(),
            },
        }
    }

    #[test]
    fn branches_yes_when_logged_in() {
        let outcome = LoggedInProcessor.evaluate(&json!({}), &ctx(true)).unwrap();
        assert_eq!(outcome.branch, Branch::Yes);
    }

    #[test]
    fn branches_no_when_anonymous() {
        let outcome = LoggedInProcessor.evaluate(&json!({}), &ctx(false)).unwrap();
        assert_eq!(outcome.branch, Branch::No);
    }
}
