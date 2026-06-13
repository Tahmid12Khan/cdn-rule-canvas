//! Component-template DTOs (Component Editor design §3.2).
//!
//! A Component is a globally-scoped, independently-versioned HTML (mustache)
//! template holding ONLY a result payload. `*Create`/`*Update`/`VersionCreate`/
//! `VersionUpdate` are validated request bodies; `*Read`/`*Summary`/
//! `ResolvedComponentRead` are response shapes. All DTOs `deny_unknown_fields`.
//!
//! `ComponentVariable.name` is the mustache key (`{{title}}` → `"title"`);
//! `title`/`description` are author metadata that drive the rule-side population
//! UI. The slug regex reuses [`crate::schemas::feature::SLUG_RE`] (kebab-case).

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;
use validator::Validate;

use crate::{
    models::{
        component_template::{ComponentTemplate, ComponentTemplateVersion},
        enums::DefaultMode,
    },
    schemas::feature::SLUG_RE,
};

/// One declared template variable. `name` is the mustache key; `title` and
/// `description` are author metadata for the rule-side population UI.
#[derive(Debug, Clone, Serialize, Deserialize, Validate, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct ComponentVariable {
    /// Mustache key (`{{name}}`).
    #[validate(length(min = 1, max = 64))]
    pub name: String,
    /// Author-facing title (label) shown in the rule population UI.
    #[validate(length(min = 1, max = 100))]
    pub title: String,
    /// Optional author-facing help text.
    #[validate(length(max = 500))]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

/// Request body for `POST /component-templates` — creates the component + its
/// v1. `html_body`/`variables` seed the first version.
#[derive(Debug, Deserialize, Validate, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct ComponentTemplateCreate {
    /// Slug (kebab-case, lowercase, 3..=120 chars).
    #[validate(length(min = 3, max = 120), regex(path = *SLUG_RE))]
    pub slug: String,
    /// Human-readable name.
    #[validate(length(min = 1, max = 200))]
    pub name: String,
    /// Optional description.
    #[validate(length(max = 2000))]
    #[serde(default)]
    pub description: Option<String>,
    /// Seed mustache body for v1 (defaults to empty).
    #[serde(default)]
    pub html_body: Option<String>,
    /// Seed declared variables for v1 (defaults to empty).
    #[validate(nested)]
    #[serde(default)]
    pub variables: Option<Vec<ComponentVariable>>,
}

/// Request body for `PATCH /component-templates/{cid}` — manages the metadata +
/// the movable default pointer. `default_version_number` is required when
/// switching to `pinned`; switching to `latest` clears the pin.
#[derive(Debug, Default, Deserialize, Validate, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct ComponentTemplateUpdate {
    /// New name.
    #[validate(length(min = 1, max = 200))]
    pub name: Option<String>,
    /// New description (`None` leaves it untouched; `Some` sets it). There is no
    /// NULL-clear, matching the feature/site update DTOs.
    #[validate(length(max = 2000))]
    #[serde(default)]
    pub description: Option<String>,
    /// New default mode (`latest` | `pinned`).
    #[serde(default)]
    pub default_mode: Option<DefaultMode>,
    /// The version to pin as default (required when `default_mode = pinned`).
    #[serde(default)]
    pub default_version_number: Option<i32>,
}

impl ComponentTemplateUpdate {
    /// Whether the update carries no fields (a no-op PATCH).
    pub fn is_empty(&self) -> bool {
        self.name.is_none()
            && self.description.is_none()
            && self.default_mode.is_none()
            && self.default_version_number.is_none()
    }
}

/// Request body for `POST /component-templates/{cid}/versions`. The new version
/// clones the current default's body/variables unless supplied. `make_default`
/// re-points the default at the new version.
///
/// Exposed in OpenAPI as `ComponentVersionCreate` to avoid colliding with the
/// feature-version `version::VersionCreate` schema.
#[derive(Debug, Default, Deserialize, Validate, ToSchema)]
#[schema(as = ComponentVersionCreate)]
#[serde(deny_unknown_fields)]
pub struct VersionCreate {
    /// Optional per-version description.
    #[validate(length(max = 2000))]
    #[serde(default)]
    pub description: Option<String>,
    /// Mustache body (clones the current default's when omitted).
    #[serde(default)]
    pub html_body: Option<String>,
    /// Declared variables (clones the current default's when omitted).
    #[validate(nested)]
    #[serde(default)]
    pub variables: Option<Vec<ComponentVariable>>,
    /// When true, re-point the default at the new version (`default_mode` →
    /// `pinned`).
    #[serde(default)]
    pub make_default: bool,
}

/// Request body for `PATCH /component-templates/{cid}/versions/{vnum}` — edits
/// the version in place (changes live output for default-following rules).
///
/// Exposed in OpenAPI as `ComponentVersionUpdate` to avoid colliding with the
/// feature-version `version::VersionUpdate` schema.
#[derive(Debug, Default, Deserialize, Validate, ToSchema)]
#[schema(as = ComponentVersionUpdate)]
#[serde(deny_unknown_fields)]
pub struct VersionUpdate {
    /// New per-version description.
    #[validate(length(max = 2000))]
    pub description: Option<String>,
    /// New mustache body.
    #[serde(default)]
    pub html_body: Option<String>,
    /// New declared variables.
    #[validate(nested)]
    #[serde(default)]
    pub variables: Option<Vec<ComponentVariable>>,
}

impl VersionUpdate {
    /// Whether the update carries no fields (a no-op PATCH).
    pub fn is_empty(&self) -> bool {
        self.description.is_none() && self.html_body.is_none() && self.variables.is_none()
    }
}

/// A version summary (no `html_body`/`variables`) embedded in
/// [`ComponentTemplateRead::versions`].
#[derive(Debug, Serialize, ToSchema)]
pub struct ComponentTemplateVersionSummary {
    /// Version UUID.
    pub id: Uuid,
    /// Per-component version number.
    pub version_number: i32,
    /// Optional per-version description.
    pub description: Option<String>,
    /// Whether this version is the component's current default.
    pub is_default: bool,
    /// Creation timestamp.
    pub created_at: DateTime<Utc>,
    /// Last-update timestamp.
    pub updated_at: DateTime<Utc>,
}

/// Full component read: metadata + default pointer + all version summaries.
#[derive(Debug, Serialize, ToSchema)]
pub struct ComponentTemplateRead {
    /// Component UUID.
    pub id: Uuid,
    /// Slug.
    pub slug: String,
    /// Name.
    pub name: String,
    /// Optional description.
    pub description: Option<String>,
    /// How the default resolves (`latest` | `pinned`).
    pub default_mode: DefaultMode,
    /// The current default's `version_number` (resolved from mode + pointer).
    pub default_version_number: Option<i32>,
    /// The highest `version_number` present.
    pub latest_version_number: i32,
    /// All version summaries, ordered by `version_number ASC`.
    pub versions: Vec<ComponentTemplateVersionSummary>,
    /// Creation timestamp.
    pub created_at: DateTime<Utc>,
    /// Last-update timestamp.
    pub updated_at: DateTime<Utc>,
}

/// List-row summary for `GET /component-templates`.
#[derive(Debug, Serialize, ToSchema)]
pub struct ComponentTemplateSummary {
    /// Component UUID.
    pub id: Uuid,
    /// Slug.
    pub slug: String,
    /// Name.
    pub name: String,
    /// Optional description.
    pub description: Option<String>,
    /// The current default's `version_number`.
    pub default_version_number: Option<i32>,
    /// The highest `version_number` present.
    pub latest_version_number: i32,
    /// Last-update timestamp.
    pub updated_at: DateTime<Utc>,
}

/// Full version read: body + variables + default flag.
#[derive(Debug, Serialize, ToSchema)]
pub struct ComponentTemplateVersionRead {
    /// Version UUID.
    pub id: Uuid,
    /// Per-component version number.
    pub version_number: i32,
    /// Optional per-version description.
    pub description: Option<String>,
    /// Mustache template body.
    pub html_body: String,
    /// Declared variables (`[{name, title, description}]`).
    pub variables: serde_json::Value,
    /// Whether this version is the component's current default.
    pub is_default: bool,
    /// Creation timestamp.
    pub created_at: DateTime<Utc>,
    /// Last-update timestamp.
    pub updated_at: DateTime<Utc>,
}

/// Proxy-facing resolve payload for `GET /component-templates/{cid}/resolve`.
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct ResolvedComponentRead {
    /// The resolved version number.
    pub version_number: i32,
    /// The resolved mustache body.
    pub html_body: String,
    /// The resolved declared variables.
    pub variables: serde_json::Value,
}

impl ComponentTemplateVersionRead {
    /// Map a [`ComponentTemplateVersion`] row + default flag to the read DTO.
    pub fn from_row(v: ComponentTemplateVersion, is_default: bool) -> Self {
        Self {
            id: v.id,
            version_number: v.version_number,
            description: v.description,
            html_body: v.html_body,
            variables: v.variables,
            is_default,
            created_at: v.created_at,
            updated_at: v.updated_at,
        }
    }
}

impl ComponentTemplateVersionSummary {
    /// Map a [`ComponentTemplateVersion`] row + default flag to the summary DTO.
    pub fn from_row(v: &ComponentTemplateVersion, is_default: bool) -> Self {
        Self {
            id: v.id,
            version_number: v.version_number,
            description: v.description.clone(),
            is_default,
            created_at: v.created_at,
            updated_at: v.updated_at,
        }
    }
}

impl ResolvedComponentRead {
    /// Map a [`ComponentTemplateVersion`] row to the proxy-facing resolve DTO.
    pub fn from_row(v: ComponentTemplateVersion) -> Self {
        Self {
            version_number: v.version_number,
            html_body: v.html_body,
            variables: v.variables,
        }
    }
}

/// Assemble a [`ComponentTemplateRead`] from a component row + its (ordered)
/// version rows. `default_version_id` resolves the default flag + number per the
/// component's `default_mode`.
pub fn build_read(
    component: ComponentTemplate,
    versions: Vec<ComponentTemplateVersion>,
) -> ComponentTemplateRead {
    let latest_version_number = versions.iter().map(|v| v.version_number).max().unwrap_or(0);
    let default_id = resolve_default_id(&component, &versions);
    let default_version_number = versions
        .iter()
        .find(|v| Some(v.id) == default_id)
        .map(|v| v.version_number);

    let summaries = versions
        .iter()
        .map(|v| ComponentTemplateVersionSummary::from_row(v, Some(v.id) == default_id))
        .collect();

    ComponentTemplateRead {
        id: component.id,
        slug: component.slug,
        name: component.name,
        description: component.description,
        default_mode: component.default_mode,
        default_version_number,
        latest_version_number,
        versions: summaries,
        created_at: component.created_at,
        updated_at: component.updated_at,
    }
}

/// Resolve which version id is the current default for a component given its
/// mode + the (ordered) version rows. `latest` → highest `version_number`;
/// `pinned` → `default_version_id` (when still present in `versions`).
pub fn resolve_default_id(
    component: &ComponentTemplate,
    versions: &[ComponentTemplateVersion],
) -> Option<Uuid> {
    match component.default_mode {
        DefaultMode::Latest => versions
            .iter()
            .max_by_key(|v| v.version_number)
            .map(|v| v.id),
        DefaultMode::Pinned => component
            .default_version_id
            .filter(|id| versions.iter().any(|v| v.id == *id)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn ts() -> DateTime<Utc> {
        Utc::now()
    }

    fn component(mode: DefaultMode, default_version_id: Option<Uuid>) -> ComponentTemplate {
        ComponentTemplate {
            id: Uuid::new_v4(),
            slug: "paywall-cta".to_string(),
            name: "Paywall CTA".to_string(),
            description: None,
            default_mode: mode,
            default_version_id,
            created_at: ts(),
            updated_at: ts(),
        }
    }

    fn version(component_id: Uuid, n: i32) -> ComponentTemplateVersion {
        ComponentTemplateVersion {
            id: Uuid::new_v4(),
            component_id,
            version_number: n,
            description: None,
            html_body: "<div>{{headline}}</div>".to_string(),
            variables: json!([]),
            created_at: ts(),
            updated_at: ts(),
        }
    }

    #[test]
    fn create_accepts_valid_slug() {
        let input = ComponentTemplateCreate {
            slug: "paywall-cta".to_string(),
            name: "Paywall CTA".to_string(),
            description: None,
            html_body: None,
            variables: None,
        };
        assert!(input.validate().is_ok());
    }

    #[test]
    fn create_rejects_uppercase_slug_and_short_slug() {
        let bad = ComponentTemplateCreate {
            slug: "AB".to_string(),
            name: "x".to_string(),
            description: None,
            html_body: None,
            variables: None,
        };
        assert!(bad.validate().is_err());
    }

    #[test]
    fn create_rejects_blank_variable_name() {
        let input = ComponentTemplateCreate {
            slug: "paywall-cta".to_string(),
            name: "Paywall CTA".to_string(),
            description: None,
            html_body: None,
            variables: Some(vec![ComponentVariable {
                name: String::new(),
                title: "Headline".to_string(),
                description: None,
            }]),
        };
        assert!(input.validate().is_err());
    }

    #[test]
    fn update_is_empty_helper() {
        assert!(ComponentTemplateUpdate::default().is_empty());
        let only_name = ComponentTemplateUpdate {
            name: Some("Renamed".to_string()),
            ..ComponentTemplateUpdate::default()
        };
        assert!(!only_name.is_empty());
    }

    #[test]
    fn version_update_is_empty_helper() {
        assert!(VersionUpdate::default().is_empty());
        let only_body = VersionUpdate {
            html_body: Some("<p>x</p>".to_string()),
            ..VersionUpdate::default()
        };
        assert!(!only_body.is_empty());
    }

    #[test]
    fn resolve_default_latest_picks_highest_number() {
        let comp = component(DefaultMode::Latest, None);
        let v1 = version(comp.id, 1);
        let v2 = version(comp.id, 2);
        let v2_id = v2.id;
        let id = resolve_default_id(&comp, &[v1, v2]);
        assert_eq!(id, Some(v2_id));
    }

    #[test]
    fn resolve_default_pinned_picks_pointer() {
        let v1 = version(Uuid::new_v4(), 1);
        let v2 = version(v1.component_id, 2);
        let comp = component(DefaultMode::Pinned, Some(v1.id));
        let v1_id = v1.id;
        let id = resolve_default_id(&comp, &[v1, v2]);
        assert_eq!(id, Some(v1_id));
    }

    #[test]
    fn resolve_default_pinned_missing_pointer_yields_none() {
        let comp = component(DefaultMode::Pinned, Some(Uuid::new_v4()));
        let v1 = version(comp.id, 1);
        assert_eq!(resolve_default_id(&comp, &[v1]), None);
    }

    #[test]
    fn build_read_marks_default_and_latest() {
        let comp = component(DefaultMode::Latest, None);
        let cid = comp.id;
        let v1 = version(cid, 1);
        let v2 = version(cid, 2);
        let v2_num = v2.version_number;
        let read = build_read(comp, vec![v1, v2]);
        assert_eq!(read.latest_version_number, 2);
        assert_eq!(read.default_version_number, Some(v2_num));
        assert!(read
            .versions
            .iter()
            .any(|s| s.is_default && s.version_number == 2));
        assert!(read
            .versions
            .iter()
            .any(|s| !s.is_default && s.version_number == 1));
    }
}
