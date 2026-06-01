//! Domain models (`sqlx::FromRow` row structs). Never serialized on the API
//! boundary — always mapped to a `*Read` DTO in the service layer.

pub mod enums;

// Domain-owned leaf modules (filled by domain agents):
pub mod component;
pub mod feature;
pub mod outcome;
pub mod version;

pub use enums::{FeatureType, Placement, VersionStatus};
