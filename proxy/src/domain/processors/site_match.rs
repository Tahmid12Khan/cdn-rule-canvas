//! `SiteMatchProcessor` (kind = "site_match"). Branches `Yes` when the request's
//! matched Site slug (`ctx.site`) equals the configured `site`, else `No`. A
//! request that fell back to `upstream_base_url` has `ctx.site == None` and so
//! always branches `No`.

use serde_json::Value;

use crate::domain::context::EvaluationContext;

use super::{Branch, CanvasProcessor, ProcessorError, ProcessorOutcome};

#[derive(Debug)]
pub struct SiteMatchProcessor;

impl CanvasProcessor for SiteMatchProcessor {
    fn kind(&self) -> &'static str {
        "site_match"
    }

    fn evaluate(
        &self,
        config: &Value,
        ctx: &EvaluationContext,
    ) -> Result<ProcessorOutcome, ProcessorError> {
        let site = config
            .get("site")
            .and_then(Value::as_str)
            .ok_or_else(|| ProcessorError::Config("missing site".to_string()))?;

        let branch = if ctx.site.as_deref() == Some(site) {
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

    fn ctx(site: Option<&str>) -> EvaluationContext {
        EvaluationContext {
            request_headers: HeaderMap::new(),
            request_path: "/".to_string(),
            request_cookies: HashMap::new(),
            device: DeviceType::Desktop,
            meta_tags: HashMap::new(),
            response_json: None,
            site: site.map(str::to_string),
            identity: Identity::default(),
        }
    }

    #[test]
    fn matching_site_branches_yes() {
        let p = SiteMatchProcessor;
        let cfg = json!({ "type": "site_match", "site": "demo-localhost" });
        let out = p.evaluate(&cfg, &ctx(Some("demo-localhost"))).unwrap();
        assert_eq!(out.branch, Branch::Yes);
    }

    #[test]
    fn different_site_branches_no() {
        let p = SiteMatchProcessor;
        let cfg = json!({ "type": "site_match", "site": "demo-localhost" });
        let out = p.evaluate(&cfg, &ctx(Some("other-site"))).unwrap();
        assert_eq!(out.branch, Branch::No);
    }

    #[test]
    fn no_site_branches_no() {
        // Fallback request (ctx.site == None) never matches.
        let p = SiteMatchProcessor;
        let cfg = json!({ "type": "site_match", "site": "demo-localhost" });
        let out = p.evaluate(&cfg, &ctx(None)).unwrap();
        assert_eq!(out.branch, Branch::No);
    }

    #[test]
    fn missing_config_is_error() {
        let p = SiteMatchProcessor;
        let cfg = json!({ "type": "site_match" });
        let err = p.evaluate(&cfg, &ctx(Some("demo-localhost"))).unwrap_err();
        assert!(matches!(err, ProcessorError::Config(_)));
    }
}
