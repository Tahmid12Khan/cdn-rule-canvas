//! Site DTOs (sites host-config design §4).
//!
//! `SiteCreate`/`SiteUpdate` are request bodies (validated); `SiteRead` is the
//! response shape. The slug regex reuses [`crate::schemas::feature::SLUG_RE`]
//! (lowercase kebab-case). `source_protocol`/`dest_protocol` are constrained to
//! `{http, https}` and ports to `1..=65535`.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use validator::{Validate, ValidationError};

use crate::{models::site::Site, schemas::feature::SLUG_RE};

/// Allowed protocol values for source/destination schemes.
const PROTOCOLS: [&str; 2] = ["http", "https"];

/// Validator: the protocol must be one of [`PROTOCOLS`] (`http` | `https`).
fn validate_protocol(value: &str) -> Result<(), ValidationError> {
    if PROTOCOLS.contains(&value) {
        Ok(())
    } else {
        Err(ValidationError::new("protocol_invalid"))
    }
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
    /// Creation timestamp.
    pub created_at: DateTime<Utc>,
    /// Last-update timestamp.
    pub updated_at: DateTime<Utc>,
}

impl From<Site> for SiteRead {
    fn from(s: Site) -> Self {
        Self {
            slug: s.slug,
            name: s.name,
            source_protocol: s.source_protocol,
            source_host: s.source_host,
            source_port: s.source_port,
            dest_protocol: s.dest_protocol,
            dest_host: s.dest_host,
            dest_port: s.dest_port,
            created_at: s.created_at,
            updated_at: s.updated_at,
        }
    }
}
