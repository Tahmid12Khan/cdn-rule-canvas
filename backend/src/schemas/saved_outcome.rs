//! Saved-outcome DTOs (Outcomes Library design).

use std::collections::HashMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;
use validator::Validate;

use crate::{models::saved_outcome::SavedOutcome, schemas::feature::SLUG_RE};

#[derive(Debug, Deserialize, Validate, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct SavedOutcomeCreate {
    #[validate(length(min = 3, max = 120), regex(path = *SLUG_RE))]
    pub slug: String,
    #[validate(length(min = 1, max = 200))]
    pub name: String,
    pub component_id: Uuid,
    /// `None`/absent = "Latest".
    #[serde(default)]
    pub version_number: Option<i32>,
    #[serde(default)]
    pub variables: HashMap<String, String>,
}

/// `PATCH` body. `version_number` uses the double-Option convention: field
/// OMITTED -> unchanged; present as JSON `null` -> clear to "Latest"; present
/// with a value -> pin that version.
#[derive(Debug, Default, Deserialize, Validate, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct SavedOutcomeUpdate {
    #[validate(length(min = 1, max = 200))]
    pub name: Option<String>,
    #[serde(default, with = "::serde_with::rust::double_option")]
    #[schema(value_type = Option<i32>)]
    pub version_number: Option<Option<i32>>,
    pub variables: Option<HashMap<String, String>>,
}

impl SavedOutcomeUpdate {
    pub fn is_empty(&self) -> bool {
        self.name.is_none() && self.version_number.is_none() && self.variables.is_none()
    }
}

#[derive(Debug, Serialize, ToSchema)]
pub struct SavedOutcomeRead {
    pub id: Uuid,
    pub slug: String,
    pub name: String,
    pub component_id: Uuid,
    /// Denormalized for list display; joined at read time.
    pub component_name: String,
    pub version_number: Option<i32>,
    pub variables: HashMap<String, String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Proxy-facing resolved-and-rendered shape (already-rendered `html_body`; the
/// applier injects it as-is, no client-side mustache pass).
#[derive(Debug, Serialize, ToSchema)]
pub struct ResolvedSavedOutcomeRead {
    pub html_body: String,
}

/// Build a `SavedOutcomeRead` from a row plus its joined component name.
pub fn to_read(row: SavedOutcome, component_name: String) -> SavedOutcomeRead {
    let variables: HashMap<String, String> = row
        .variables
        .as_object()
        .map(|m| {
            m.iter()
                .filter_map(|(k, v)| v.as_str().map(|s| (k.clone(), s.to_string())))
                .collect()
        })
        .unwrap_or_default();
    SavedOutcomeRead {
        id: row.id,
        slug: row.slug,
        name: row.name,
        component_id: row.component_id,
        component_name,
        version_number: row.version_number,
        variables,
        created_at: row.created_at,
        updated_at: row.updated_at,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn create_input_accepts_valid() {
        let input = SavedOutcomeCreate {
            slug: "promo-banner-default".to_string(),
            name: "Promo Banner (Default)".to_string(),
            component_id: Uuid::new_v4(),
            version_number: None,
            variables: HashMap::new(),
        };
        assert!(input.validate().is_ok());
    }

    #[test]
    fn create_input_rejects_short_slug() {
        let input = SavedOutcomeCreate {
            slug: "ab".to_string(),
            name: "X".to_string(),
            component_id: Uuid::new_v4(),
            version_number: None,
            variables: HashMap::new(),
        };
        assert!(input.validate().is_err());
    }

    #[test]
    fn update_double_option_distinguishes_omitted_null_and_value() {
        let omitted: SavedOutcomeUpdate = serde_json::from_str("{}").unwrap();
        assert_eq!(omitted.version_number, None);

        let cleared: SavedOutcomeUpdate =
            serde_json::from_str(r#"{"version_number": null}"#).unwrap();
        assert_eq!(cleared.version_number, Some(None));

        let pinned: SavedOutcomeUpdate = serde_json::from_str(r#"{"version_number": 3}"#).unwrap();
        assert_eq!(pinned.version_number, Some(Some(3)));
    }

    #[test]
    fn update_is_empty_helper() {
        assert!(SavedOutcomeUpdate::default().is_empty());
        let only_name = SavedOutcomeUpdate {
            name: Some("Renamed".to_string()),
            ..SavedOutcomeUpdate::default()
        };
        assert!(!only_name.is_empty());
    }
}
