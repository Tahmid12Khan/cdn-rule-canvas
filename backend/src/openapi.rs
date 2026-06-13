//! OpenAPI aggregator (utoipa). Served at `/docs` via swagger-ui.
//!
//! Paths and schemas are wired against the canonical contract; domain agents
//! supply the `#[utoipa::path]`-annotated handlers and `ToSchema` DTOs they
//! reference here.

use utoipa::OpenApi;

use crate::{
    api::v1::{
        component_templates, components, features, health, node_types, outcomes, sites,
        test_presets, versions,
    },
    error::{ErrorBody, ErrorEnvelope, ValidationDetail},
    models::enums::{DefaultMode, FeatureType, Placement, VersionStatus},
    schemas::{
        active_version::{ActiveComponent, ActiveOutcome, ActiveVersionRead},
        applicability::Applicability,
        component::{
            ComponentConfig, ComponentCreate, ComponentRead, ComponentUpdate, HtmlPlacementMode,
        },
        component_template::{
            ComponentTemplateCreate, ComponentTemplateRead, ComponentTemplateSummary,
            ComponentTemplateUpdate, ComponentTemplateVersionRead, ComponentTemplateVersionSummary,
            ComponentVariable, ResolvedComponentRead,
            VersionCreate as ComponentTemplateVersionCreate,
            VersionUpdate as ComponentTemplateVersionUpdate,
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
        test_preset::{TestPresetCreate, TestPresetRead, TestPresetUpdate},
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
        test_presets::create,
        test_presets::list,
        test_presets::get,
        test_presets::update,
        test_presets::delete,
        component_templates::create,
        component_templates::list,
        component_templates::get_one,
        component_templates::get_by_slug,
        component_templates::update,
        component_templates::delete,
        component_templates::list_versions,
        component_templates::create_version,
        component_templates::get_version,
        component_templates::update_version,
        component_templates::delete_version,
        component_templates::make_default,
        component_templates::resolve,
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
        DefaultMode,
        // Feature
        FeatureCreate,
        FeatureUpdate,
        FeatureRead,
        // Site
        SiteCreate,
        SiteUpdate,
        SiteRead,
        // Test preset
        TestPresetCreate,
        TestPresetUpdate,
        TestPresetRead,
        // Component templates (Component Editor)
        ComponentTemplateCreate,
        ComponentTemplateUpdate,
        ComponentTemplateRead,
        ComponentTemplateSummary,
        ComponentTemplateVersionRead,
        ComponentTemplateVersionSummary,
        ComponentVariable,
        ComponentTemplateVersionCreate,
        ComponentTemplateVersionUpdate,
        ResolvedComponentRead,
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
        (name = "test_presets", description = "Test-preset library CRUD"),
        (name = "component_templates", description = "Component Editor library CRUD + resolve"),
        (name = "versions", description = "Version lifecycle"),
        (name = "outcomes", description = "Outcomes and their components"),
        (name = "components", description = "Flat component operations"),
    )
)]
pub struct ApiDoc;
