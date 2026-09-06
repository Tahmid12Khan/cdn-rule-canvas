//! `HasProductProcessor` (kind = "has_product"). Branches `Yes` iff the
//! visitor's identity carries the configured `product` label. A stale/unknown
//! label (deleted product) fails open to `No` — mirrors `site_match`'s
//! fail-open behavior on a stale reference.

use serde_json::Value;

use crate::domain::context::EvaluationContext;

use super::{Branch, CanvasProcessor, ProcessorError, ProcessorOutcome};

#[derive(Debug)]
pub struct HasProductProcessor;

impl CanvasProcessor for HasProductProcessor {
    fn kind(&self) -> &'static str {
        "has_product"
    }

    fn evaluate(
        &self,
        config: &Value,
        ctx: &EvaluationContext,
    ) -> Result<ProcessorOutcome, ProcessorError> {
        let product = config
            .get("product")
            .and_then(Value::as_str)
            .ok_or_else(|| ProcessorError::Config("missing product".to_string()))?;

        let branch = if ctx.identity.products.contains(product) {
            Branch::Yes
        } else {
            Branch::No
        };

        Ok(ProcessorOutcome { branch })
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use http::HeaderMap;
    use serde_json::json;

    use crate::domain::context::{DeviceType, EvaluationContext};
    use crate::domain::identity::Identity;

    use super::*;

    fn ctx(products: &[&str]) -> EvaluationContext {
        EvaluationContext {
            request_headers: HeaderMap::new(),
            request_path: "/".to_string(),
            request_cookies: HashMap::new(),
            device: DeviceType::Desktop,
            meta_tags: HashMap::new(),
            response_json: None,
            site: None,
            identity: Identity {
                logged_in: true,
                products: products.iter().map(|s| s.to_string()).collect(),
            },
        }
    }

    #[test]
    fn branches_yes_when_product_held() {
        let outcome = HasProductProcessor
            .evaluate(&json!({ "product": "premium" }), &ctx(&["premium"]))
            .unwrap();
        assert_eq!(outcome.branch, Branch::Yes);
    }

    #[test]
    fn branches_no_when_product_absent_or_stale() {
        let outcome = HasProductProcessor
            .evaluate(&json!({ "product": "gold" }), &ctx(&["premium"]))
            .unwrap();
        assert_eq!(outcome.branch, Branch::No);
    }

    #[test]
    fn errors_when_config_missing_product() {
        assert!(HasProductProcessor.evaluate(&json!({}), &ctx(&[])).is_err());
    }
}
