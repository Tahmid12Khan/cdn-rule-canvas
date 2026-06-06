//! Site domain model (`sqlx::FromRow`). Never serialized on the API boundary —
//! mapped to [`crate::schemas::site::SiteRead`] in the service.

use chrono::{DateTime, Utc};

/// A site row from `rre.sites`: a `source → destination` routing entry. The
/// proxy matches an incoming request's `Host` against `(source_host,
/// source_port)` and forwards to `dest_protocol://dest_host:dest_port`.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct Site {
    /// Slug primary key (kebab-case, 3..=64 chars).
    pub slug: String,
    /// Human-readable name (unique).
    pub name: String,
    /// Source scheme (`http` | `https`) — cosmetic; the proxy binds one port.
    pub source_protocol: String,
    /// Source host matched against the incoming `Host` header.
    pub source_host: String,
    /// Source port matched against the incoming `Host` header (1..=65535).
    pub source_port: i32,
    /// Destination scheme (`http` | `https`) for the upstream forward.
    pub dest_protocol: String,
    /// Destination host for the upstream forward.
    pub dest_host: String,
    /// Destination port for the upstream forward (1..=65535).
    pub dest_port: i32,
    /// Custom request headers injected when forwarding, stored as JSONB
    /// (`{ "Header-Name": "value" }`). Typed into a `HashMap<String, String>` on
    /// read in the service layer (default `{}`).
    pub headers: serde_json::Value,
    /// Creation timestamp.
    pub created_at: DateTime<Utc>,
    /// Last-update timestamp.
    pub updated_at: DateTime<Utc>,
}
