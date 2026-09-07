//! Builds the edge bundle: everything a host needs to evaluate one site's
//! published rules with NO further I/O.
//!
//! Component templates and saved outcomes are resolved INLINE here, because the
//! consumer — a Fastly Compute guest — cannot call back to this backend. The
//! resolution is STATIC: every expression node in the canvas and every component
//! of every outcome is walked regardless of routing, because export time does
//! not know which branch a future request will take.
//!
//! The return type is [`rre_core::edge::EdgeBundle`] itself, not a backend
//! mirror. The proxy and the edge deserialize that exact struct, so reusing it
//! makes an exporter/consumer schema skew a compile error instead of a silent
//! field mismatch at the POP.

use std::collections::{HashMap, HashSet};

use rre_core::{
    applier::{component_ref, json_apply},
    bundle::{
        ActiveVersionRead as CoreActiveVersion, ResolvedComponent, ResolvedSavedOutcome,
        VersionSelector,
    },
    edge::{component_key, EdgeBundle, EdgeFeature, EdgeSite, SCHEMA_VERSION},
    graph::Node,
};
use sqlx::PgPool;
use uuid::Uuid;

use crate::{
    error::{AppError, AppResult},
    schemas::{active_version::ActiveVersionRead, version::PublishEnvironment},
    services::{component_template_service, feature_service, saved_outcome_service, site_service},
};

/// Build the bundle for `site_slug` at `environment`.
///
/// An unknown site is [`AppError::SiteNotFound`] — an operator exporting a
/// typo'd slug must find out here, not at the edge. A feature with no published
/// version for the environment is SKIPPED rather than exported with an empty
/// canvas, so the bundle contains only rules that are actually live.
pub async fn build(
    pool: &PgPool,
    site_slug: &str,
    environment: PublishEnvironment,
) -> AppResult<EdgeBundle> {
    let site = site_service::get(pool, site_slug).await?;
    let features = feature_service::list_ordered(pool).await?;

    let mut out = Vec::with_capacity(features.len());
    for feature in features {
        let Some(active) = active_version(pool, &feature.id, environment).await? else {
            continue;
        };
        let core = to_core_version(&feature.id, &active)?;
        let (resolved_components, saved_outcomes) = resolve_refs(pool, &core).await?;

        out.push(EdgeFeature {
            id: feature.id,
            r#type: match feature.r#type {
                crate::models::enums::FeatureType::Html => "html".to_string(),
                crate::models::enums::FeatureType::Json => "json".to_string(),
            },
            execution_order: feature.execution_order,
            version_number: core.version_number,
            applicability: core.applicability,
            rule_graph: core.rule_graph,
            outcomes: core.outcomes,
            resolved_components,
            saved_outcomes,
        });
    }

    Ok(EdgeBundle {
        schema_version: SCHEMA_VERSION,
        site: EdgeSite {
            slug: site.slug,
            source_host: site.source_host,
        },
        environment: environment_str(environment).to_string(),
        generated_at: chrono::Utc::now().to_rfc3339(),
        features: out,
    })
}

/// The environment discriminator written into the bundle. Matches the
/// `PublishEnvironment` wire form the rest of the API uses.
fn environment_str(environment: PublishEnvironment) -> &'static str {
    match environment {
        PublishEnvironment::Live => "live",
        PublishEnvironment::Staging => "staging",
    }
}

/// The active version for `feature_id`, or `None` when the feature has nothing
/// published for this environment.
///
/// `version_service::active_version` signals "nothing published" as an ERROR
/// ([`AppError::NoLiveVersion`]) because its own caller is a 404 route. Export
/// is a bulk read where that is an ordinary skip, so it is translated here
/// rather than propagated. Every other error still propagates.
async fn active_version(
    pool: &PgPool,
    feature_id: &str,
    environment: PublishEnvironment,
) -> AppResult<Option<ActiveVersionRead>> {
    match crate::services::version_service::active_version(pool, feature_id, environment).await {
        Ok(av) => Ok(Some(av)),
        Err(AppError::NoLiveVersion(_)) => Ok(None),
        Err(e) => Err(e),
    }
}

/// Re-read the backend's active-version DTO as the shared `rre-core` struct.
///
/// The two are field-for-field identical on the wire (they are both the
/// BACKEND CONTRACT §5 payload), so a serde round-trip is the conversion AND
/// the assertion that they still agree. A failure is an internal error: it means
/// the two definitions have drifted.
fn to_core_version(feature_id: &str, active: &ActiveVersionRead) -> AppResult<CoreActiveVersion> {
    let wire = serde_json::to_value(active).map_err(|e| {
        AppError::Internal(anyhow::anyhow!(
            "serializing active version for '{feature_id}': {e}"
        ))
    })?;
    serde_json::from_value(wire).map_err(|e| {
        AppError::Internal(anyhow::anyhow!(
            "active-version shape drifted from rre_core::bundle for '{feature_id}': {e}"
        ))
    })
}

/// Every component template and saved outcome this version can reach, resolved.
///
/// Collected STATICALLY — the whole canvas and every outcome, not just the path
/// some request would take — because the bundle is frozen at export time.
async fn resolve_refs(
    pool: &PgPool,
    active: &CoreActiveVersion,
) -> AppResult<(
    HashMap<String, ResolvedComponent>,
    HashMap<Uuid, ResolvedSavedOutcome>,
)> {
    let (component_refs, saved_outcome_refs) = collect_refs(active);

    let mut components = HashMap::with_capacity(component_refs.len());
    for (id, selector) in component_refs {
        // A dangling reference is skipped, not fatal. The appliers already treat
        // an unresolved component as "skip this action" (fail-open), so refusing
        // to export the whole site over one stale rule would be strictly worse
        // for the reader than exporting the rest.
        match component_template_service::resolve(pool, id, to_service_selector(selector)).await {
            Ok(resolved) => {
                components.insert(
                    component_key(id, &selector),
                    ResolvedComponent {
                        version_number: resolved.version_number,
                        html_body: resolved.html_body,
                        variables: resolved.variables,
                    },
                );
            }
            Err(AppError::ComponentNotFound(msg))
            | Err(AppError::ComponentVersionNotFound(msg)) => {
                tracing::warn!(component_id = %id, reason = %msg, "edge bundle: component reference unresolvable, omitted");
            }
            Err(e) => return Err(e),
        }
    }

    let mut saved = HashMap::with_capacity(saved_outcome_refs.len());
    for id in saved_outcome_refs {
        match saved_outcome_service::resolve(pool, id).await {
            Ok(resolved) => {
                saved.insert(
                    id,
                    ResolvedSavedOutcome {
                        html_body: resolved.html_body,
                    },
                );
            }
            Err(AppError::SavedOutcomeNotFound(msg))
            | Err(AppError::ComponentNotFound(msg))
            | Err(AppError::ComponentVersionNotFound(msg)) => {
                tracing::warn!(saved_outcome_id = %id, reason = %msg, "edge bundle: saved-outcome reference unresolvable, omitted");
            }
            Err(e) => return Err(e),
        }
    }

    Ok((components, saved))
}

/// Walk the canvas and the outcomes for references.
///
/// Detection is delegated to the applier's OWN helpers
/// ([`json_apply::component_ref`], [`json_apply::saved_outcome_ref`],
/// [`component_ref::config_ref`]) so the exporter and the runtime cannot
/// disagree about what counts as a reference: a reference the applier will look
/// up is exactly one this collects.
fn collect_refs(active: &CoreActiveVersion) -> (HashSet<(Uuid, VersionSelector)>, HashSet<Uuid>) {
    let mut components = HashSet::new();
    let mut saved = HashSet::new();

    for node in &active.rule_graph.canvas.nodes {
        let Node::Expression { action, .. } = node else {
            continue;
        };
        // `ProcessorRef` flattens to the `{ "type": …, … }` object the appliers
        // dispatch on, which is what the ref helpers expect.
        let Ok(value) = serde_json::to_value(action) else {
            continue;
        };
        if let Some(r) = json_apply::component_ref(&value) {
            components.insert(r);
        }
        if let Some(id) = json_apply::saved_outcome_ref(&value) {
            saved.insert(id);
        }
    }

    for outcome in &active.outcomes {
        for component in &outcome.components {
            if let Some(r) = component_ref::config_ref(&component.config) {
                components.insert(r);
            }
        }
    }

    (components, saved)
}

/// `rre_core`'s selector → the component service's own. Two identical enums live
/// in the two crates because neither may depend on the other's DTO layer.
fn to_service_selector(selector: VersionSelector) -> component_template_service::VersionSelector {
    match selector {
        VersionSelector::Default => component_template_service::VersionSelector::Default,
        VersionSelector::Version(n) => component_template_service::VersionSelector::Version(n),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rre_core::bundle::{ActiveComponent, ActiveOutcome, Applicability, Placement};
    use rre_core::graph::{CanvasGraph, ProcessorRef, RuleGraph};
    use serde_json::json;

    fn expression(action: serde_json::Value) -> Node {
        Node::Expression {
            id: "e".into(),
            action: serde_json::from_value::<ProcessorRef>(action).expect("processor"),
            custom_label: None,
            position: rre_core::graph::Position { x: 0.0, y: 0.0 },
        }
    }

    fn version(nodes: Vec<Node>, outcomes: Vec<ActiveOutcome>) -> CoreActiveVersion {
        CoreActiveVersion {
            version_number: 1,
            rule_graph: RuleGraph {
                canvas: CanvasGraph {
                    nodes,
                    edges: Vec::new(),
                    root_node_id: None,
                },
            },
            applicability: Applicability::default(),
            outcomes,
        }
    }

    #[test]
    fn collect_refs_finds_component_and_saved_outcome_actions() {
        let cid = Uuid::new_v4();
        let sid = Uuid::new_v4();
        let v = version(
            vec![
                expression(json!({
                    "type": "apply_component",
                    "component_id": cid.to_string(),
                    "version": 3,
                    "target_selector": "main",
                    "placement_mode": "append",
                })),
                expression(json!({
                    "type": "apply_saved_outcome_json",
                    "saved_outcome_id": sid.to_string(),
                    "target_path": "$.html",
                })),
            ],
            Vec::new(),
        );

        let (components, saved) = collect_refs(&v);
        assert_eq!(
            components,
            HashSet::from([(cid, VersionSelector::Version(3))])
        );
        assert_eq!(saved, HashSet::from([sid]));
    }

    #[test]
    fn collect_refs_reaches_components_inside_outcomes() {
        let cid = Uuid::new_v4();
        let v = version(
            Vec::new(),
            vec![ActiveOutcome {
                id: Uuid::new_v4(),
                title: "Paywall".into(),
                is_builtin: false,
                order_index: 0,
                components: vec![ActiveComponent {
                    id: Uuid::new_v4(),
                    slug: "boxed".into(),
                    r#type: "component_ref".into(),
                    config: json!({
                        "type": "component_ref",
                        "component_id": cid.to_string(),
                        "version": "default",
                        "variables": {},
                        "target_selector": "main",
                        "placement_mode": "append",
                    }),
                    placement: Placement::Inline,
                    order_index: 0,
                }],
            }],
        );

        let (components, saved) = collect_refs(&v);
        assert_eq!(components, HashSet::from([(cid, VersionSelector::Default)]));
        assert!(saved.is_empty());
    }

    /// A non-reference action contributes nothing — the exporter must not invent
    /// lookups for `json_set`, `apply_outcome` and friends.
    #[test]
    fn collect_refs_ignores_non_reference_actions() {
        let v = version(
            vec![
                expression(json!({ "type": "json_set", "json_path": "$.x", "value": 1 })),
                expression(json!({
                    "type": "apply_outcome",
                    "outcome_id": Uuid::new_v4().to_string(),
                })),
            ],
            Vec::new(),
        );

        let (components, saved) = collect_refs(&v);
        assert!(components.is_empty());
        assert!(saved.is_empty());
    }

    /// The same component referenced twice resolves ONCE.
    #[test]
    fn collect_refs_deduplicates() {
        let cid = Uuid::new_v4();
        let action = json!({
            "type": "apply_component",
            "component_id": cid.to_string(),
            "version": "default",
            "target_selector": "main",
            "placement_mode": "append",
        });
        let v = version(
            vec![expression(action.clone()), expression(action)],
            Vec::new(),
        );

        assert_eq!(collect_refs(&v).0.len(), 1);
    }

    #[test]
    fn environment_str_matches_the_wire_form() {
        assert_eq!(environment_str(PublishEnvironment::Live), "live");
        assert_eq!(environment_str(PublishEnvironment::Staging), "staging");
    }
}
