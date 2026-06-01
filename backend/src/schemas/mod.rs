//! Serde DTOs for the API boundary. Routers map `FromRow` models to these
//! `*Read`/`*Create`/`*Update` types — `FromRow` structs are never serialized
//! directly.

pub mod health;
pub mod pagination;

// Domain-owned leaf modules (filled by domain agents):
pub mod active_version;
pub mod applicability;
pub mod component;
pub mod feature;
pub mod node_type;
pub mod outcome;
pub mod rule_graph;
pub mod version;

pub use pagination::{Page, PageParams};
