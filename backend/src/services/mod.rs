//! Business-logic layer. Services own the transaction boundary, raise domain
//! errors, and return serde DTOs (never raw `FromRow` rows).

// Domain-owned leaf modules (filled by domain agents):
pub mod feature_service;
pub mod outcome_service;
pub mod rule_graph_service;
pub mod site_service;
pub mod test_preset_service;
pub mod version_service;
