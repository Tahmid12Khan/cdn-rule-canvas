//! Version-level applicability (BACKEND CONTRACT §5).
//!
//! [`Applicability`] gates whether a feature's outcome components apply to a
//! given upstream response. It is persisted as the `rre.versions.applicability`
//! JSONB column and surfaced on `VersionRead`, `VersionCreate`, `VersionUpdate`,
//! and the proxy-facing `ActiveVersionRead`.
//!
//! Backend validation is intentionally light — selectors are length-capped and
//! must be non-empty when present. Real CSS/JSONPath parsing is the proxy's job
//! (resilient, fail-open). The empty value `{}` means "apply whenever the
//! response content-type matches the feature type".

use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::error::ValidationDetail;

/// Maximum selector length (chars) accepted by the backend.
const MAX_SELECTOR_LEN: usize = 500;

/// Version-level applicability gate. Both selectors are optional; an absent or
/// empty selector means "apply whenever the response content-type matches".
#[derive(Deserialize, Serialize, Clone, Debug, Default, PartialEq, ToSchema)]
pub struct Applicability {
    /// CSS selector that must match >=1 element for an HTML feature's rules to
    /// apply. None/empty = apply whenever the response content-type is HTML.
    #[serde(default)]
    pub html_selector: Option<String>,
    /// JSONPath that must match for a JSON feature's rules to apply. None/empty =
    /// apply whenever the response content-type is JSON.
    #[serde(default)]
    pub json_selector: Option<String>,
}

impl Applicability {
    /// Light backend validation: each present selector must be trimmed-non-empty
    /// and at most [`MAX_SELECTOR_LEN`] chars. Returns one
    /// [`ValidationDetail`] per offending field (empty when valid).
    pub fn validation_details(&self) -> Vec<ValidationDetail> {
        let mut details = Vec::new();
        check_selector("html_selector", self.html_selector.as_deref(), &mut details);
        check_selector("json_selector", self.json_selector.as_deref(), &mut details);
        details
    }
}

/// Validate one optional selector, pushing a detail on overflow / blank.
fn check_selector(field: &str, value: Option<&str>, details: &mut Vec<ValidationDetail>) {
    let Some(raw) = value else { return };
    if raw.trim().is_empty() {
        details.push(ValidationDetail::new(
            format!("applicability.{field}"),
            format!("{field} must not be blank when present"),
            "applicability_selector_invalid",
        ));
    } else if raw.chars().count() > MAX_SELECTOR_LEN {
        details.push(ValidationDetail::new(
            format!("applicability.{field}"),
            format!("{field} must be at most {MAX_SELECTOR_LEN} characters"),
            "applicability_selector_invalid",
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_is_empty_and_valid() {
        let a = Applicability::default();
        assert!(a.html_selector.is_none());
        assert!(a.json_selector.is_none());
        assert!(a.validation_details().is_empty());
    }

    #[test]
    fn round_trips_through_json() {
        let a = Applicability {
            html_selector: Some("#paywall".to_string()),
            json_selector: Some("$.type".to_string()),
        };
        let v = serde_json::to_value(&a).unwrap();
        let back: Applicability = serde_json::from_value(v).unwrap();
        assert_eq!(a, back);
    }

    #[test]
    fn empty_object_parses_to_default() {
        let a: Applicability = serde_json::from_value(serde_json::json!({})).unwrap();
        assert_eq!(a, Applicability::default());
    }

    #[test]
    fn blank_selector_is_rejected() {
        let a = Applicability {
            html_selector: Some("   ".to_string()),
            json_selector: None,
        };
        let details = a.validation_details();
        assert_eq!(details.len(), 1);
        assert_eq!(details[0].rule_id, "applicability_selector_invalid");
        assert_eq!(details[0].loc, "applicability.html_selector");
    }

    #[test]
    fn overlong_selector_is_rejected() {
        let a = Applicability {
            html_selector: None,
            json_selector: Some("$.".to_string() + &"a".repeat(600)),
        };
        let details = a.validation_details();
        assert_eq!(details.len(), 1);
        assert_eq!(details[0].loc, "applicability.json_selector");
    }
}
