//! Test-preset DTOs (CONTRACT §3/§7).
//!
//! `TestPresetCreate`/`TestPresetUpdate` are request bodies (validated);
//! `TestPresetRead` is the response shape. The slug regex reuses
//! [`crate::schemas::feature::SLUG_RE`] (lowercase kebab-case). `kind` is
//! constrained to `{rule, url}`. `payload` is an opaque JSON object owned by the
//! frontend test panels; the service validates ONLY that it is a JSON object
//! within a serialized-size cap (the frontend zod schema + the proxy's
//! `headers_to_map` are the per-field guards).

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use validator::{Validate, ValidationError};

use crate::{
    error::{AppError, AppResult, ValidationDetail},
    models::test_preset::TestPreset,
    schemas::feature::SLUG_RE,
};

/// Allowed `kind` values.
const KINDS: [&str; 2] = ["rule", "url"];

/// Maximum serialized size of a `payload` object (16 KB).
const MAX_PAYLOAD_BYTES: usize = 16_384;

/// Validator: the kind must be one of [`KINDS`] (`rule` | `url`).
fn validate_kind(value: &str) -> Result<(), ValidationError> {
    if KINDS.contains(&value) {
        Ok(())
    } else {
        Err(ValidationError::new("kind_invalid"))
    }
}

/// Validate a preset `payload` (called in the service — the `validator` crate
/// cannot validate the JSON value's shape). Enforces, in order:
///
/// * the value MUST be a JSON object (`{}`) — not an array/scalar/null
///   (`rule_id = "payload_not_object"`);
/// * the serialized form MUST be at most [`MAX_PAYLOAD_BYTES`] bytes
///   (`rule_id = "payload_too_large"`).
///
/// On violation returns [`AppError::Validation`] (422) with `loc = "payload"`.
pub fn validate_payload(payload: &serde_json::Value) -> AppResult<()> {
    if !payload.is_object() {
        return Err(AppError::validation(vec![ValidationDetail::new(
            "payload",
            "payload must be a JSON object",
            "payload_not_object",
        )]));
    }

    let serialized = serde_json::to_string(payload).map_err(|e| AppError::Internal(e.into()))?;
    if serialized.len() > MAX_PAYLOAD_BYTES {
        return Err(AppError::validation(vec![ValidationDetail::new(
            "payload",
            format!("payload must be at most {MAX_PAYLOAD_BYTES} bytes when serialized"),
            "payload_too_large",
        )]));
    }

    Ok(())
}

/// Request body for `POST /test-presets`.
#[derive(Debug, Deserialize, Validate, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct TestPresetCreate {
    /// Slug primary key (kebab-case, lowercase, 3..=64 chars).
    #[validate(length(min = 3, max = 64), regex(path = *SLUG_RE))]
    pub slug: String,
    /// Human-readable name.
    #[validate(length(min = 1, max = 200))]
    pub name: String,
    /// Preset kind (`rule` | `url`).
    #[validate(custom(function = "validate_kind"))]
    pub kind: String,
    /// Opaque saved Test-panel inputs (a JSON object). Validated by
    /// [`validate_payload`] in the service.
    pub payload: serde_json::Value,
}

/// Request body for `PATCH /test-presets/{slug}`. Every field is optional; the
/// slug and kind are immutable (not present here).
#[derive(Debug, Default, Deserialize, Validate, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct TestPresetUpdate {
    /// New human-readable name.
    #[validate(length(min = 1, max = 200))]
    pub name: Option<String>,
    /// Replacement payload object. `None` leaves the stored payload unchanged;
    /// `Some(value)` replaces it wholesale. Validated by [`validate_payload`].
    #[serde(default)]
    pub payload: Option<serde_json::Value>,
}

impl TestPresetUpdate {
    /// Whether the update carries no fields (a no-op PATCH).
    pub fn is_empty(&self) -> bool {
        self.name.is_none() && self.payload.is_none()
    }
}

/// Response shape for a test preset.
#[derive(Debug, Serialize, ToSchema)]
pub struct TestPresetRead {
    /// Slug primary key.
    pub slug: String,
    /// Human-readable name.
    pub name: String,
    /// Preset kind (`rule` | `url`).
    pub kind: String,
    /// Opaque saved Test-panel inputs (a JSON object).
    pub payload: serde_json::Value,
    /// Creation timestamp.
    pub created_at: DateTime<Utc>,
    /// Last-update timestamp.
    pub updated_at: DateTime<Utc>,
}

impl From<TestPreset> for TestPresetRead {
    fn from(p: TestPreset) -> Self {
        Self {
            slug: p.slug,
            name: p.name,
            kind: p.kind,
            payload: p.payload,
            created_at: p.created_at,
            updated_at: p.updated_at,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn details_of(result: AppResult<()>) -> Vec<ValidationDetail> {
        match result {
            Err(AppError::Validation { details }) => details,
            other => panic!("expected validation error, got {other:?}"),
        }
    }

    #[test]
    fn accepts_object_payload() {
        assert!(validate_payload(&json!({})).is_ok());
        assert!(validate_payload(&json!({ "url": "https://example.com" })).is_ok());
    }

    #[test]
    fn rejects_non_object_payload() {
        for value in [
            json!(null),
            json!(1),
            json!("s"),
            json!([1, 2, 3]),
            json!(true),
        ] {
            let details = details_of(validate_payload(&value));
            assert_eq!(details[0].rule_id, "payload_not_object");
            assert_eq!(details[0].loc, "payload");
        }
    }

    #[test]
    fn rejects_oversized_payload() {
        // A single string field exceeding the serialized cap.
        let big = "a".repeat(MAX_PAYLOAD_BYTES + 1);
        let payload = json!({ "blob": big });
        let details = details_of(validate_payload(&payload));
        assert_eq!(details[0].rule_id, "payload_too_large");
        assert_eq!(details[0].loc, "payload");
    }

    #[test]
    fn validate_kind_accepts_known_and_rejects_unknown() {
        assert!(validate_kind("rule").is_ok());
        assert!(validate_kind("url").is_ok());
        assert!(validate_kind("other").is_err());
    }

    #[test]
    fn update_is_empty_and_partial() {
        assert!(TestPresetUpdate::default().is_empty());
        let only_name = TestPresetUpdate {
            name: Some("X".to_string()),
            ..TestPresetUpdate::default()
        };
        assert!(!only_name.is_empty());
        let only_payload = TestPresetUpdate {
            payload: Some(json!({})),
            ..TestPresetUpdate::default()
        };
        assert!(!only_payload.is_empty());
    }
}
