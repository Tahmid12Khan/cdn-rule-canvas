//! Site DTOs (sites host-config design §4).
//!
//! `SiteCreate`/`SiteUpdate` are request bodies (validated); `SiteRead` is the
//! response shape. The slug regex reuses [`crate::schemas::feature::SLUG_RE`]
//! (lowercase kebab-case). `source_protocol`/`dest_protocol` are constrained to
//! `{http, https}` and ports to `1..=65535`.

use std::collections::HashMap;

use chrono::{DateTime, Utc};
use once_cell::sync::Lazy;
use regex::Regex;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use validator::{Validate, ValidationError};

use crate::{
    error::{AppError, AppResult, ValidationDetail},
    models::site::Site,
    schemas::feature::SLUG_RE,
};

/// Allowed protocol values for source/destination schemes.
const PROTOCOLS: [&str; 2] = ["http", "https"];

/// Maximum number of custom header entries per site.
const MAX_HEADERS: usize = 32;
/// Maximum header-name length (HTTP token).
const MAX_HEADER_NAME_LEN: usize = 128;
/// Maximum header-value length.
const MAX_HEADER_VALUE_LEN: usize = 2048;

/// HTTP token grammar (RFC 7230 `token`) for a header name.
static HEADER_NAME_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"^[!#$%&'*+\-.^_`|~0-9A-Za-z]+$").expect("valid header-name regex"));

/// Header names an admin may NOT configure (matched case-insensitively). These
/// are proxy-managed routing/framing/connection-control headers; allowing an
/// override would enable request smuggling or break the upstream forward. Stored
/// lowercase for case-insensitive comparison.
const FORBIDDEN_HEADER_NAMES: [&str; 10] = [
    "host",
    "content-length",
    "transfer-encoding",
    "connection",
    "keep-alive",
    "proxy-authenticate",
    "proxy-authorization",
    "te",
    "trailer",
    "upgrade",
];

/// Validator: the protocol must be one of [`PROTOCOLS`] (`http` | `https`).
fn validate_protocol(value: &str) -> Result<(), ValidationError> {
    if PROTOCOLS.contains(&value) {
        Ok(())
    } else {
        Err(ValidationError::new("protocol_invalid"))
    }
}

/// Validate a custom-headers map (called in the service — the `validator` crate
/// cannot easily validate map *contents*). Enforces, in order:
///
/// * at most [`MAX_HEADERS`] entries;
/// * each NAME is a non-empty HTTP token (`HEADER_NAME_RE`), 1..=128 chars;
/// * each NAME is not a proxy-managed routing/framing header
///   ([`FORBIDDEN_HEADER_NAMES`], case-insensitive) — overriding one would enable
///   request smuggling or break the upstream forward;
/// * each VALUE is 0..=2048 chars of visible ASCII/space only — NO control
///   chars (`\r`, `\n`, any byte `< 0x20`, or `0x7f`). This is security-critical
///   (HTTP header / response splitting prevention).
///
/// On violation returns [`AppError::Validation`] (422) with a `loc` of
/// `headers` (count rule) or `headers["<name>"]` (per-entry rule).
pub fn validate_headers(headers: &HashMap<String, String>) -> AppResult<()> {
    if headers.len() > MAX_HEADERS {
        return Err(AppError::validation(vec![ValidationDetail::new(
            "headers",
            format!("at most {MAX_HEADERS} custom headers are allowed"),
            "headers_too_many",
        )]));
    }

    let mut details: Vec<ValidationDetail> = Vec::new();
    for (name, value) in headers {
        let loc = format!("headers[\"{name}\"]");

        if name.is_empty() || name.len() > MAX_HEADER_NAME_LEN || !HEADER_NAME_RE.is_match(name) {
            details.push(ValidationDetail::new(
                &loc,
                format!(
                    "header name must be an HTTP token (1..={MAX_HEADER_NAME_LEN} chars, no spaces or separators)"
                ),
                "header_name_invalid",
            ));
            // Skip the value check for an invalid name — the name is the loc key.
            continue;
        }

        // Reject proxy-managed routing/framing headers (case-insensitive).
        if FORBIDDEN_HEADER_NAMES.contains(&name.to_ascii_lowercase().as_str()) {
            details.push(ValidationDetail::new(
                &loc,
                format!("header name '{name}' is reserved and cannot be configured"),
                "header_name_forbidden",
            ));
            continue;
        }

        if value.len() > MAX_HEADER_VALUE_LEN {
            details.push(ValidationDetail::new(
                &loc,
                format!("header value must be at most {MAX_HEADER_VALUE_LEN} chars"),
                "header_value_too_long",
            ));
            continue;
        }

        // Reject control chars: \r, \n, any C0 control (< 0x20), DEL (0x7f), or
        // any non-ASCII byte. Only visible ASCII (0x20..=0x7e) is permitted.
        if value.bytes().any(|b| !(0x20..=0x7e).contains(&b)) {
            details.push(ValidationDetail::new(
                &loc,
                "header value must contain only visible ASCII characters and spaces \
                 (no control characters)"
                    .to_string(),
                "header_value_control_char",
            ));
        }
    }

    if details.is_empty() {
        Ok(())
    } else {
        Err(AppError::validation(details))
    }
}

/// Convert a model's JSONB `headers` value into a `HashMap<String, String>`.
/// An empty/absent/non-object value (or any non-string value) maps to `{}` —
/// the stored shape is always validated on write, so this is defensive only.
pub fn headers_from_json(value: &serde_json::Value) -> HashMap<String, String> {
    value
        .as_object()
        .map(|map| {
            map.iter()
                .filter_map(|(k, v)| v.as_str().map(|s| (k.clone(), s.to_string())))
                .collect()
        })
        .unwrap_or_default()
}

/// Serialize a header map into a JSONB object for persistence.
pub fn headers_to_json(headers: &HashMap<String, String>) -> serde_json::Value {
    serde_json::json!(headers)
}

/// Request body for `POST /sites`.
#[derive(Debug, Deserialize, Validate, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct SiteCreate {
    /// Slug primary key (kebab-case, lowercase, 3..=64 chars).
    #[validate(length(min = 3, max = 64), regex(path = *SLUG_RE))]
    pub slug: String,
    /// Human-readable name.
    #[validate(length(min = 1, max = 200))]
    pub name: String,
    /// Source scheme (`http` | `https`).
    #[validate(custom(function = "validate_protocol"))]
    pub source_protocol: String,
    /// Source host (1..=255 chars).
    #[validate(length(min = 1, max = 255))]
    pub source_host: String,
    /// Source port (1..=65535).
    #[validate(range(min = 1, max = 65535))]
    pub source_port: i32,
    /// Destination scheme (`http` | `https`).
    #[validate(custom(function = "validate_protocol"))]
    pub dest_protocol: String,
    /// Destination host (1..=255 chars).
    #[validate(length(min = 1, max = 255))]
    pub dest_host: String,
    /// Destination port (1..=65535).
    #[validate(range(min = 1, max = 65535))]
    pub dest_port: i32,
    /// Custom request headers injected by the proxy (`{ "Header-Name": "value" }`).
    /// Validated by [`validate_headers`] in the service. Absent → empty map.
    #[serde(default)]
    pub headers: Option<HashMap<String, String>>,
}

/// Request body for `PATCH /sites/{slug}`. Every field is optional; the slug is
/// immutable (path-derived) and not present here.
#[derive(Debug, Default, Deserialize, Validate, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct SiteUpdate {
    /// New human-readable name.
    #[validate(length(min = 1, max = 200))]
    pub name: Option<String>,
    /// New source scheme (`http` | `https`).
    #[validate(custom(function = "validate_protocol"))]
    pub source_protocol: Option<String>,
    /// New source host (1..=255 chars).
    #[validate(length(min = 1, max = 255))]
    pub source_host: Option<String>,
    /// New source port (1..=65535).
    #[validate(range(min = 1, max = 65535))]
    pub source_port: Option<i32>,
    /// New destination scheme (`http` | `https`).
    #[validate(custom(function = "validate_protocol"))]
    pub dest_protocol: Option<String>,
    /// New destination host (1..=255 chars).
    #[validate(length(min = 1, max = 255))]
    pub dest_host: Option<String>,
    /// New destination port (1..=65535).
    #[validate(range(min = 1, max = 65535))]
    pub dest_port: Option<i32>,
    /// Replacement custom-headers map (`{ "Header-Name": "value" }`). `None`
    /// leaves the stored map unchanged; `Some(map)` replaces it wholesale.
    /// Validated by [`validate_headers`] in the service.
    #[serde(default)]
    pub headers: Option<HashMap<String, String>>,
}

impl SiteUpdate {
    /// Whether the update carries no fields (a no-op PATCH).
    pub fn is_empty(&self) -> bool {
        self.name.is_none()
            && self.source_protocol.is_none()
            && self.source_host.is_none()
            && self.source_port.is_none()
            && self.dest_protocol.is_none()
            && self.dest_host.is_none()
            && self.dest_port.is_none()
            && self.headers.is_none()
    }
}

/// Response shape for a site.
#[derive(Debug, Serialize, ToSchema)]
pub struct SiteRead {
    /// Slug primary key.
    pub slug: String,
    /// Human-readable name.
    pub name: String,
    /// Source scheme.
    pub source_protocol: String,
    /// Source host.
    pub source_host: String,
    /// Source port.
    pub source_port: i32,
    /// Destination scheme.
    pub dest_protocol: String,
    /// Destination host.
    pub dest_host: String,
    /// Destination port.
    pub dest_port: i32,
    /// Custom request headers the proxy injects (`{ "Header-Name": "value" }`).
    /// An empty/absent stored map serializes as `{}`.
    pub headers: HashMap<String, String>,
    /// Creation timestamp.
    pub created_at: DateTime<Utc>,
    /// Last-update timestamp.
    pub updated_at: DateTime<Utc>,
}

impl From<Site> for SiteRead {
    fn from(s: Site) -> Self {
        let headers = headers_from_json(&s.headers);
        Self {
            slug: s.slug,
            name: s.name,
            source_protocol: s.source_protocol,
            source_host: s.source_host,
            source_port: s.source_port,
            dest_protocol: s.dest_protocol,
            dest_host: s.dest_host,
            dest_port: s.dest_port,
            headers,
            created_at: s.created_at,
            updated_at: s.updated_at,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn map(pairs: &[(&str, &str)]) -> HashMap<String, String> {
        pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect()
    }

    fn details_of(result: AppResult<()>) -> Vec<ValidationDetail> {
        match result {
            Err(AppError::Validation { details }) => details,
            other => panic!("expected validation error, got {other:?}"),
        }
    }

    #[test]
    fn accepts_empty_map() {
        assert!(validate_headers(&HashMap::new()).is_ok());
    }

    #[test]
    fn accepts_valid_headers() {
        let h = map(&[("X-Foo", "bar"), ("Authorization", "Bearer abc")]);
        assert!(validate_headers(&h).is_ok());
        // Empty value is allowed (0-length).
        assert!(validate_headers(&map(&[("X-Empty", "")])).is_ok());
    }

    #[test]
    fn rejects_crlf_in_value() {
        let details = details_of(validate_headers(&map(&[("X-Foo", "a\r\nInjected: 1")])));
        assert_eq!(details[0].rule_id, "header_value_control_char");
        assert_eq!(details[0].loc, "headers[\"X-Foo\"]");
    }

    #[test]
    fn rejects_bare_newline_and_del_in_value() {
        assert_eq!(
            details_of(validate_headers(&map(&[("X-Foo", "a\nb")])))[0].rule_id,
            "header_value_control_char"
        );
        assert_eq!(
            details_of(validate_headers(&map(&[("X-Foo", "a\x7fb")])))[0].rule_id,
            "header_value_control_char"
        );
    }

    #[test]
    fn rejects_name_with_space() {
        let details = details_of(validate_headers(&map(&[("Bad Name", "1")])));
        assert_eq!(details[0].rule_id, "header_name_invalid");
        assert_eq!(details[0].loc, "headers[\"Bad Name\"]");
    }

    #[test]
    fn rejects_empty_name() {
        assert_eq!(
            details_of(validate_headers(&map(&[("", "1")])))[0].rule_id,
            "header_name_invalid"
        );
    }

    #[test]
    fn rejects_forbidden_header_names() {
        // Proxy-managed routing/framing headers are rejected (case-insensitive).
        for name in [
            "Host",
            "Content-Length",
            "transfer-encoding",
            "CONNECTION",
            "Te",
        ] {
            let details = details_of(validate_headers(&map(&[(name, "v")])));
            assert_eq!(
                details[0].rule_id, "header_name_forbidden",
                "name {name} should be forbidden"
            );
            assert_eq!(details[0].loc, format!("headers[\"{name}\"]"));
        }
    }

    #[test]
    fn rejects_more_than_max_headers() {
        let many: HashMap<String, String> = (0..=MAX_HEADERS)
            .map(|i| (format!("X-H-{i}"), "v".to_string()))
            .collect();
        let details = details_of(validate_headers(&many));
        assert_eq!(details[0].rule_id, "headers_too_many");
        assert_eq!(details[0].loc, "headers");
    }

    #[test]
    fn rejects_oversized_value() {
        let big = "a".repeat(MAX_HEADER_VALUE_LEN + 1);
        assert_eq!(
            details_of(validate_headers(&map(&[("X-Big", &big)])))[0].rule_id,
            "header_value_too_long"
        );
    }

    #[test]
    fn rejects_oversized_name() {
        let big = "a".repeat(MAX_HEADER_NAME_LEN + 1);
        assert_eq!(
            details_of(validate_headers(&map(&[(big.as_str(), "v")])))[0].rule_id,
            "header_name_invalid"
        );
    }

    #[test]
    fn json_roundtrip_helpers() {
        let h = map(&[("X-Foo", "bar")]);
        let json = headers_to_json(&h);
        assert_eq!(json["X-Foo"], "bar");
        assert_eq!(headers_from_json(&json), h);
        // Non-object / null defaults to empty.
        assert!(headers_from_json(&serde_json::Value::Null).is_empty());
    }
}
