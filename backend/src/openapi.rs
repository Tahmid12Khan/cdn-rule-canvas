//! OpenAPI aggregator (utoipa). Served at `/docs` via swagger-ui.
//!
//! Paths and schemas are wired against the canonical contract; domain agents
//! supply the `#[utoipa::path]`-annotated handlers and `ToSchema` DTOs they
//! reference here.

use utoipa::OpenApi;

use crate::{
    api::v1::{components, features, health, node_types, outcomes, sites, versions},
    error::{ErrorBody, ErrorEnvelope, ValidationDetail},
    models::enums::{FeatureType, Placement, VersionStatus},
    schemas::{
        active_version::{ActiveComponent, ActiveOutcome, ActiveVersionRead},
        applicability::Applicability,
        component::{
            ComponentConfig, ComponentCreate, ComponentRead, ComponentUpdate, HtmlPlacementMode,
        },
        feature::{FeatureCreate, FeatureRead, FeatureUpdate},
        health::HealthResponse,
        node_type::{
            AppliesTo, BranchSpec, Category, Control, Field, NodeManifest, NodeTypeSpec, Option_,
            Output, RequiredUnless,
        },
        outcome::{OutcomeCreate, OutcomeRead, OutcomeUpdate, ReorderItem},
        rule_graph::{Branch, CanvasGraph, Edge, Node, Position, ProcessorConfig, RuleGraph},
        site::{SiteCreate, SiteRead, SiteUpdate},
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
        node_types::list,
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
        sites::create,
        sites::list,
        sites::get,
        sites::update,
        sites::delete,
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
        // Site
        SiteCreate,
        SiteUpdate,
        SiteRead,
        // Version
        VersionCreate,
        VersionUpdate,
        VersionRead,
        VersionSummary,
        PublishRequest,
        PublishEnvironment,
        Applicability,
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
        // Node-type manifest
        NodeManifest,
        Category,
        NodeTypeSpec,
        AppliesTo,
        Field,
        Control,
        RequiredUnless,
        Option_,
        Output,
        BranchSpec,
        // Active version (proxy-facing)
        ActiveVersionRead,
        ActiveOutcome,
        ActiveComponent,
    )),
    tags(
        (name = "health", description = "Liveness and readiness"),
        (name = "node-types", description = "Node-type manifest"),
        (name = "features", description = "Feature CRUD + active version"),
        (name = "sites", description = "Site (host config) CRUD"),
        (name = "versions", description = "Version lifecycle"),
        (name = "outcomes", description = "Outcomes and their components"),
        (name = "components", description = "Flat component operations"),
    )
)]
pub struct ApiDoc;
