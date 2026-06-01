//! OpenAPI aggregator (utoipa). Served at `/docs` via swagger-ui.
//!
//! Paths and schemas are wired against the canonical contract; domain agents
//! supply the `#[utoipa::path]`-annotated handlers and `ToSchema` DTOs they
//! reference here.

use utoipa::OpenApi;

use crate::{
    api::v1::{components, features, health, outcomes, versions},
    error::{ErrorBody, ErrorEnvelope, ValidationDetail},
    models::enums::{FeatureType, Placement, VersionStatus},
    schemas::{
        active_version::{ActiveComponent, ActiveOutcome, ActiveVersionRead},
        component::{
            ComponentConfig, ComponentCreate, ComponentRead, ComponentUpdate, HtmlPlacementMode,
        },
        feature::{FeatureCreate, FeatureRead, FeatureUpdate},
        health::HealthResponse,
        outcome::{OutcomeCreate, OutcomeRead, OutcomeUpdate, ReorderItem},
        rule_graph::{
            Branch, CanvasGraph, DeviceOperator, DeviceValue, Edge, MetaTagsOperator, Node,
            Position, ProcessorConfig, RuleGraph,
        },
        version::{
            PublishEnvironment, PublishRequest, VersionCreate, VersionRead, VersionSummary,
            VersionUpdate,
        },
    },
};

/// Aggregated OpenAPI document for the RRE backend.
#[derive(OpenApi)]
#[openapi(
    info(
        title = "RRE Backend",
        description = "Response Rule Engine admin API",
        version = "0.1.0"
    ),
    paths(
        health::health,
        health::healthz_db,
        features::create,
        features::list,
        features::get,
        features::update,
        features::delete,
        features::active_version,
        versions::create,
        versions::list,
        versions::get,
        versions::update,
        versions::publish,
        versions::unpublish,
        versions::delete,
        outcomes::list_for_version,
        outcomes::create,
        outcomes::get,
        outcomes::update,
        outcomes::delete,
        outcomes::clone,
        outcomes::reorder_components,
        outcomes::add_component,
        components::update,
        components::delete,
    ),
    components(schemas(
        // Infra / error envelope
        ErrorEnvelope,
        ErrorBody,
        ValidationDetail,
        HealthResponse,
        // Enums
        FeatureType,
        VersionStatus,
        Placement,
        // Feature
        FeatureCreate,
        FeatureUpdate,
        FeatureRead,
        // Version
        VersionCreate,
        VersionUpdate,
        VersionRead,
        VersionSummary,
        PublishRequest,
        PublishEnvironment,
        // Outcome / Component
        OutcomeCreate,
        OutcomeUpdate,
        OutcomeRead,
        ReorderItem,
        ComponentCreate,
        ComponentUpdate,
        ComponentRead,
        ComponentConfig,
        HtmlPlacementMode,
        // Rule graph
        RuleGraph,
        CanvasGraph,
        Node,
        ProcessorConfig,
        Edge,
        Branch,
        Position,
        MetaTagsOperator,
        DeviceOperator,
        DeviceValue,
        // Active version (proxy-facing)
        ActiveVersionRead,
        ActiveOutcome,
        ActiveComponent,
    )),
    tags(
        (name = "health", description = "Liveness and readiness"),
        (name = "features", description = "Feature CRUD + active version"),
        (name = "versions", description = "Version lifecycle"),
        (name = "outcomes", description = "Outcomes and their components"),
        (name = "components", description = "Flat component operations"),
    )
)]
pub struct ApiDoc;
