//! Component DTOs (BACKEND CONTRACT §5).
//!
//! `ComponentConfig` is a typed, internally-tagged (`type`) union; the persisted
//! `components.config` JSONB MUST carry a `type` discriminator equal to the row's
//! `type` column. The service validates by deserializing into `ComponentConfig`
//! before persist; a mismatch surfaces as `VALIDATION_ERROR`.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;
use validator::Validate;

use crate::models::component::Component;
use crate::models::enums::Placement;

/// HTML injection placement mode (component config, not a DB enum).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum HtmlPlacementMode {
    /// Replace the target element's contents.
    Replace,
    /// Append inside the target element.
    Append,
    /// Prepend inside the target element.
    Prepend,
    /// Insert before the target element.
    Before,
    /// Insert after the target element.
    After,
}

/// Typed component configuration. Internally tagged by `type`.
#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ComponentConfig {
    /// `type = "html_injection"`.
    HtmlInjection {
        /// CSS selector the injection targets.
        target_selector: String,
        /// How the HTML body is placed relative to the target.
        placement_mode: HtmlPlacementMode,
        /// Raw HTML to inject (sanitized by the proxy at apply time).
        html_body: String,
        /// Optional theme key.
        #[serde(default)]
        theme: Option<String>,
    },
    /// `type = "content_truncation"`.
    ContentTruncation {
        /// CSS selector whose content is truncated.
        target_selector: String,
        /// Word budget before truncation (`1..=10000`).
        word_count: u32,
        /// Whether to apply a fade-out on the truncated remainder.
        #[serde(default)]
        fade_out: bool,
    },
    /// `type = "html_remove"` — delete matched content and inject NOTHING.
    /// `include_selector = false` empties the matched element (the element
    /// itself survives); `true` removes the element and its contents.
    HtmlRemove {
        /// CSS selector whose content (or whole element) is removed.
        target_selector: String,
        /// Remove the matched element itself too, not just its contents.
        #[serde(default)]
        include_selector: bool,
    },
    /// `type = "json_remove"` — delete the value(s) at `target_path`.
    JsonRemove {
        /// Simple JSON path (dot + `[index]`, e.g. `$.user.premium`) to delete.
        target_path: String,
    },
    /// `type = "json_set"` — upsert (create or overwrite) the value at
    /// `target_path`.
    JsonSet {
        /// Simple JSON path (dot + `[index]`) to upsert.
        target_path: String,
        /// The JSON value to write (any JSON, including `null`).
        value: serde_json::Value,
    },
    /// `type = "json_replace"` — overwrite ONLY if `target_path` already exists.
    JsonReplace {
        /// Simple JSON path (dot + `[index]`) to overwrite when present.
        target_path: String,
        /// The JSON value to write (any JSON, including `null`).
        value: serde_json::Value,
    },
    /// `type = "component_ref"` — reference the versioned component-template
    /// library and inject the rendered (mustache + sanitized) HTML at a CSS
    /// selector. HTML features only. The row-level `Component.placement`
    /// (`inline`/`sticky_footer`/`popup`) applies exactly like `html_injection`.
    ComponentRef {
        /// Library component id (`rre.component_templates.id`).
        component_id: Uuid,
        /// Version selector: the string `"default"` or a positive integer.
        version: serde_json::Value,
        /// Mustache variable values keyed by variable name.
        variables: serde_json::Map<String, serde_json::Value>,
        /// CSS selector the rendered component targets.
        target_selector: String,
        /// How the rendered HTML is placed relative to the target.
        placement_mode: HtmlPlacementMode,
    },
    /// `type = "component_ref_json"` — reference the versioned component-template
    /// library, render to an HTML string, and SET that string at a JSON path.
    /// JSON features only.
    ComponentRefJson {
        /// Library component id (`rre.component_templates.id`).
        component_id: Uuid,
        /// Version selector: the string `"default"` or a positive integer.
        version: serde_json::Value,
        /// Mustache variable values keyed by variable name.
        variables: serde_json::Map<String, serde_json::Value>,
        /// Simple JSON path (dot + `[index]`, e.g. `$.content.html`) to set.
        target_path: String,
    },
}

impl ComponentConfig {
    /// The discriminator string this config corresponds to. Matches the `type`
    /// column and the DB CHECK constraint values.
    pub fn type_str(&self) -> &'static str {
        match self {
            ComponentConfig::HtmlInjection { .. } => "html_injection",
            ComponentConfig::ContentTruncation { .. } => "content_truncation",
            ComponentConfig::HtmlRemove { .. } => "html_remove",
            ComponentConfig::JsonRemove { .. } => "json_remove",
            ComponentConfig::JsonSet { .. } => "json_set",
            ComponentConfig::JsonReplace { .. } => "json_replace",
            ComponentConfig::ComponentRef { .. } => "component_ref",
            ComponentConfig::ComponentRefJson { .. } => "component_ref_json",
        }
    }

    /// Domain-specific config validation beyond shape (e.g. word-count bounds).
    /// Returns an error message on failure.
    pub fn validate_domain(&self) -> Result<(), String> {
        match self {
            ComponentConfig::HtmlInjection {
                target_selector, ..
            } => {
                if target_selector.trim().is_empty() {
                    return Err("target_selector must not be empty".to_string());
                }
                Ok(())
            }
            ComponentConfig::ContentTruncation {
                target_selector,
                word_count,
                ..
            } => {
                if target_selector.trim().is_empty() {
                    return Err("target_selector must not be empty".to_string());
                }
                if *word_count < 1 || *word_count > 10_000 {
                    return Err("word_count must be between 1 and 10000".to_string());
                }
                Ok(())
            }
            // Removal: only the CSS selector needs checking (nothing is injected).
            ComponentConfig::HtmlRemove {
                target_selector, ..
            } => validate_selector(target_selector),
            // JSON mutators: `target_path` is a simple path (dot + `[index]`),
            // non-empty and length-capped. `value` (set/replace) may be any JSON.
            ComponentConfig::JsonRemove { target_path }
            | ComponentConfig::JsonSet { target_path, .. }
            | ComponentConfig::JsonReplace { target_path, .. } => validate_target_path(target_path),
            // Library-component reference (HTML): `target_selector` is a CSS
            // selector (trimmed-non-empty, length-capped) and `version` must be
            // `"default"` or a positive integer. `variables` is the object the
            // serde shape already guarantees.
            ComponentConfig::ComponentRef {
                version,
                target_selector,
                ..
            } => {
                validate_selector(target_selector)?;
                validate_version(version)
            }
            // Library-component reference (JSON): same version rule; the result
            // is set at `target_path` (same simple-path validation as the JSON
            // mutators).
            ComponentConfig::ComponentRefJson {
                version,
                target_path,
                ..
            } => {
                validate_target_path(target_path)?;
                validate_version(version)
            }
        }
    }
}

/// Maximum `target_path` length (chars) for JSON mutation components.
const MAX_TARGET_PATH_LEN: usize = 500;

/// Validate a JSON mutation `target_path`: trimmed-non-empty and at most
/// [`MAX_TARGET_PATH_LEN`] chars.
fn validate_target_path(target_path: &str) -> Result<(), String> {
    if target_path.trim().is_empty() {
        return Err("target_path must not be empty".to_string());
    }
    if target_path.chars().count() > MAX_TARGET_PATH_LEN {
        return Err(format!(
            "target_path must be at most {MAX_TARGET_PATH_LEN} characters"
        ));
    }
    Ok(())
}

/// Validate a `target_selector` (component-ref HTML): trimmed-non-empty and at
/// most [`MAX_TARGET_PATH_LEN`] chars (same cap as the JSON path).
fn validate_selector(target_selector: &str) -> Result<(), String> {
    if target_selector.trim().is_empty() {
        return Err("target_selector must not be empty".to_string());
    }
    if target_selector.chars().count() > MAX_TARGET_PATH_LEN {
        return Err(format!(
            "target_selector must be at most {MAX_TARGET_PATH_LEN} characters"
        ));
    }
    Ok(())
}

/// Validate a component-ref `version` selector: either the string `"default"`
/// or a positive integer (`>= 1`). Well-formedness only — a pinned version that
/// no longer exists is resolved fail-open at the proxy (falls back to default).
fn validate_version(version: &serde_json::Value) -> Result<(), String> {
    match version {
        serde_json::Value::String(s) if s == "default" => Ok(()),
        serde_json::Value::Number(n) => match n.as_i64() {
            Some(v) if v >= 1 => Ok(()),
            _ => Err("version must be \"default\" or a positive integer".to_string()),
        },
        _ => Err("version must be \"default\" or a positive integer".to_string()),
    }
}

/// Create-component request body.
#[derive(Debug, Clone, Deserialize, Validate, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct ComponentCreate {
    /// Stable slug within the outcome.
    #[validate(length(min = 1, max = 120))]
    pub slug: String,
    /// Discriminator string; must match `config`'s embedded `type`.
    pub r#type: String,
    /// Typed configuration payload.
    pub config: ComponentConfig,
    /// Placement on the page.
    pub placement: Placement,
    /// Optional explicit ordering; defaults to append.
    pub order_index: Option<i32>,
}

/// Patch-component request body. All fields optional.
#[derive(Debug, Clone, Deserialize, Validate, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct ComponentUpdate {
    /// New slug.
    #[validate(length(min = 1, max = 120))]
    pub slug: Option<String>,
    /// New discriminator string.
    pub r#type: Option<String>,
    /// New typed configuration.
    pub config: Option<ComponentConfig>,
    /// New placement.
    pub placement: Option<Placement>,
    /// New ordering.
    pub order_index: Option<i32>,
}

/// Component read DTO. `config` is the raw stored JSON (`serde_json::Value`).
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct ComponentRead {
    /// Component id.
    pub id: Uuid,
    /// Owning outcome id.
    pub outcome_id: Uuid,
    /// Slug.
    pub slug: String,
    /// Discriminator string.
    pub r#type: String,
    /// Raw stored config JSON.
    pub config: serde_json::Value,
    /// Placement.
    pub placement: Placement,
    /// Ordering.
    pub order_index: i32,
    /// Creation timestamp.
    pub created_at: DateTime<Utc>,
    /// Last-update timestamp.
    pub updated_at: DateTime<Utc>,
}

impl From<Component> for ComponentRead {
    fn from(c: Component) -> Self {
        Self {
            id: c.id,
            outcome_id: c.outcome_id,
            slug: c.slug,
            r#type: c.r#type,
            config: c.config,
            placement: c.placement,
            order_index: c.order_index,
            created_at: c.created_at,
            updated_at: c.updated_at,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    /// JSON mutation variants deserialize from their wire shape and report the
    /// matching `type_str()` discriminator.
    #[test]
    fn json_variants_round_trip_and_type_str() {
        let remove: ComponentConfig =
            serde_json::from_value(json!({ "type": "json_remove", "target_path": "$.user.email" }))
                .unwrap();
        assert_eq!(remove.type_str(), "json_remove");

        let set: ComponentConfig = serde_json::from_value(
            json!({ "type": "json_set", "target_path": "$.user.premium", "value": true }),
        )
        .unwrap();
        assert_eq!(set.type_str(), "json_set");

        let replace: ComponentConfig = serde_json::from_value(
            json!({ "type": "json_replace", "target_path": "$.items[0].price", "value": 9.99 }),
        )
        .unwrap();
        assert_eq!(replace.type_str(), "json_replace");

        // Serializing back keeps the `type` tag.
        let v = serde_json::to_value(&set).unwrap();
        assert_eq!(v["type"], "json_set");
        assert_eq!(v["target_path"], "$.user.premium");
        assert_eq!(v["value"], true);
    }

    /// `json_set`/`json_replace` accept any JSON value, including `null`.
    #[test]
    fn json_set_accepts_null_value() {
        let set = ComponentConfig::JsonSet {
            target_path: "$.user.token".to_string(),
            value: serde_json::Value::Null,
        };
        assert!(set.validate_domain().is_ok());
    }

    /// `target_path` must be trimmed-non-empty.
    #[test]
    fn empty_target_path_is_rejected() {
        let remove = ComponentConfig::JsonRemove {
            target_path: "   ".to_string(),
        };
        let err = remove.validate_domain().unwrap_err();
        assert_eq!(err, "target_path must not be empty");
    }

    /// `target_path` is length-capped at 500 chars.
    #[test]
    fn overlong_target_path_is_rejected() {
        let replace = ComponentConfig::JsonReplace {
            target_path: "$.".to_string() + &"a".repeat(600),
            value: json!(1),
        };
        let err = replace.validate_domain().unwrap_err();
        assert!(err.contains("at most 500"));
    }

    /// A well-formed simple path passes.
    #[test]
    fn valid_target_path_passes() {
        let remove = ComponentConfig::JsonRemove {
            target_path: "$.items[0].price".to_string(),
        };
        assert!(remove.validate_domain().is_ok());
    }

    /// `component_ref` round-trips and reports its discriminator.
    #[test]
    fn component_ref_round_trip_and_type_str() {
        let cfg: ComponentConfig = serde_json::from_value(json!({
            "type": "component_ref",
            "component_id": "11111111-1111-1111-1111-111111111111",
            "version": "default",
            "variables": { "headline": "Hi" },
            "target_selector": "main .article-body",
            "placement_mode": "append"
        }))
        .unwrap();
        assert_eq!(cfg.type_str(), "component_ref");
        assert!(cfg.validate_domain().is_ok());

        let v = serde_json::to_value(&cfg).unwrap();
        assert_eq!(v["type"], "component_ref");
        assert_eq!(v["placement_mode"], "append");
        assert_eq!(v["variables"]["headline"], "Hi");
    }

    /// `component_ref_json` round-trips and reports its discriminator.
    #[test]
    fn component_ref_json_round_trip_and_type_str() {
        let cfg: ComponentConfig = serde_json::from_value(json!({
            "type": "component_ref_json",
            "component_id": "22222222-2222-2222-2222-222222222222",
            "version": 3,
            "variables": {},
            "target_path": "$.content.html"
        }))
        .unwrap();
        assert_eq!(cfg.type_str(), "component_ref_json");
        assert!(cfg.validate_domain().is_ok());

        let v = serde_json::to_value(&cfg).unwrap();
        assert_eq!(v["type"], "component_ref_json");
        assert_eq!(v["target_path"], "$.content.html");
    }

    /// `version` accepts `"default"` and positive integers; rejects 0/neg and
    /// non-`"default"` strings.
    #[test]
    fn version_selector_rules() {
        assert!(validate_version(&json!("default")).is_ok());
        assert!(validate_version(&json!(1)).is_ok());
        assert!(validate_version(&json!(42)).is_ok());

        assert!(validate_version(&json!(0)).is_err());
        assert!(validate_version(&json!(-1)).is_err());
        assert!(validate_version(&json!("latest")).is_err());
        assert!(validate_version(&json!(null)).is_err());
        assert!(validate_version(&json!(1.5)).is_err());
    }

    /// `component_ref` with a bad version fails domain validation.
    #[test]
    fn component_ref_bad_version_rejected() {
        let cfg = ComponentConfig::ComponentRef {
            component_id: Uuid::new_v4(),
            version: json!(0),
            variables: serde_json::Map::new(),
            target_selector: "main".to_string(),
            placement_mode: HtmlPlacementMode::Append,
        };
        assert!(cfg.validate_domain().is_err());
    }

    /// `component_ref` with an empty selector fails domain validation.
    #[test]
    fn component_ref_empty_selector_rejected() {
        let cfg = ComponentConfig::ComponentRef {
            component_id: Uuid::new_v4(),
            version: json!("default"),
            variables: serde_json::Map::new(),
            target_selector: "   ".to_string(),
            placement_mode: HtmlPlacementMode::Replace,
        };
        let err = cfg.validate_domain().unwrap_err();
        assert!(err.contains("target_selector"));
    }
}
