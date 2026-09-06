//! Data-access layer. Repositories own all SQLx queries and return models;
//! they never produce serde response DTOs. RUNTIME queries only
//! (`sqlx::query_as::<_, T>`, `.bind(...)`) — never the compile-time macros.

// Domain-owned leaf modules (filled by domain agents):
pub mod component_repository;
pub mod feature_repository;
pub mod outcome_repository;
pub mod product_repository;
pub mod site_repository;
pub mod test_preset_repository;
pub mod version_repository;
