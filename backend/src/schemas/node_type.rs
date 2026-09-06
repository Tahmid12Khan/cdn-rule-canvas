//! Node-type manifest (BACKEND CONTRACT §6).
//!
//! [`NodeManifest`] is the backend-owned source of truth for every canvas node
//! type plus the palette categories. It is deserialized once at startup from
//! `config/node_types.json` into `Arc<NodeManifest>` (in `AppState`) and served
//! verbatim at `GET /api/v1/node-types`.
//!
//! ALL object keys are snake_case (NEVER camelCase). The `kind` field is the
//! canonical snake_case identifier used uniformly: manifest `kind` ==
//! rule_graph processor `type` == proxy `CanvasProcessor::kind()` == JDM
//! `CustomNode` kind. Adding a node type is ONE entry here (plus a proxy
//! `CanvasProcessor` impl); zero frontend changes, zero backend Rust changes.

use std::collections::HashMap;
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use utoipa::ToSchema;

/// The full node-type manifest: palette categories + node-type specs, in palette
/// order. Served verbatim at `GET /api/v1/node-types`.
///
/// The typed form drives manifest-driven validation. The endpoint serves the
/// raw parsed JSON (see [`LoadedManifest::raw`]) so the response is byte-for-key
/// faithful to the source file — `required: false` and other omitted-by-default
/// fields are never synthesized.
#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
pub struct NodeManifest {
    /// Palette categories (in palette order).
    pub categories: Vec<Category>,
    /// Node-type specifications (in palette order).
    pub node_types: Vec<NodeTypeSpec>,
    /// Canvas display rules (operator symbols live per-option; this carries
    /// the rest). Defaults when omitted from the manifest file.
    #[serde(default)]
    pub display: DisplayConfig,
}

/// Server-owned canvas display rules (the client only renders per these).
#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
pub struct DisplayConfig {
    /// Max chars before a value token is truncated with an ellipsis on the canvas.
    pub value_max_chars: usize,
}

impl Default for DisplayConfig {
    fn default() -> Self {
        Self {
            value_max_chars: 10,
        }
    }
}

impl NodeManifest {
    /// Build a `kind -> spec` index for O(1) lookup during validation.
    pub fn index(&self) -> HashMap<&str, &NodeTypeSpec> {
        self.node_types
            .iter()
            .map(|spec| (spec.kind.as_str(), spec))
            .collect()
    }
}

/// The manifest as loaded at startup: the typed form (for validation) plus the
/// raw parsed JSON (served verbatim at the endpoint).
#[derive(Debug, Clone)]
pub struct LoadedManifest {
    /// Typed manifest, used by manifest-driven validation.
    pub typed: Arc<NodeManifest>,
    /// Raw parsed JSON, served verbatim at `GET /api/v1/node-types`.
    pub raw: Arc<Value>,
}

impl LoadedManifest {
    /// Read and parse the node-type manifest from `path`.
    ///
    /// # Errors
    /// Returns an error if the file is missing or not a valid manifest.
    pub fn load(path: &str) -> anyhow::Result<Self> {
        let bytes = std::fs::read_to_string(path)
            .map_err(|e| anyhow::anyhow!("reading node manifest {path}: {e}"))?;
        Self::from_json(&bytes).map_err(|e| anyhow::anyhow!("parsing node manifest {path}: {e}"))
    }

    /// Parse the manifest from a JSON string (used by tests and `load`).
    ///
    /// # Errors
    /// Returns an error if the JSON is not a valid manifest.
    pub fn from_json(json: &str) -> anyhow::Result<Self> {
        let raw: Value = serde_json::from_str(json)?;
        let typed: NodeManifest = serde_json::from_value(raw.clone())?;
        Ok(Self {
            typed: Arc::new(typed),
            raw: Arc::new(raw),
        })
    }
}

/// One palette category. `coming_soon` categories render as disabled chips.
#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
pub struct Category {
    /// Category id (referenced by `NodeTypeSpec::category`).
    pub id: String,
    /// Human label for the category.
    pub label: String,
    /// When true, the category's chips are disabled ("coming soon").
    #[serde(default)]
    pub coming_soon: bool,
}

/// Feature-type gate for palette availability. The frontend filters palette
/// chips by the current feature's type; `All` is always shown.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum AppliesTo {
    /// Available for any feature type (default when omitted).
    #[default]
    All,
    /// Available only for HTML features (e.g. reads response HTML).
    Html,
    /// Available only for JSON features (e.g. reads response JSON).
    Json,
}

/// The node taxonomy a manifest entry maps to. Drives which React Flow node type
/// the frontend creates on drop and which validation path applies: a `Decision`
/// node routes yes/no, an `Expression` node performs one body action and passes
/// through.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum NodeKind {
    /// A routing node (yes/no branches). Default when omitted.
    #[default]
    Decision,
    /// A body-action node that performs one action and continues.
    Expression,
}

/// One node-type specification.
#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
pub struct NodeTypeSpec {
    /// Canonical snake_case identifier (== rule_graph processor `type`).
    pub kind: String,
    /// Node title + palette chip label.
    pub label: String,
    /// Category id (must exist in `NodeManifest::categories`).
    pub category: String,
    /// Feature-type gate for palette availability (`all` when omitted).
    #[serde(default)]
    pub applies_to: AppliesTo,
    /// Node taxonomy this entry maps to (`decision` when omitted).
    #[serde(default)]
    pub node_kind: NodeKind,
    /// Tooltip "input info".
    pub summary: String,
    /// Ordered config fields.
    pub fields: Vec<Field>,
    /// Output branches.
    pub output: Output,
}

/// One config control on a node type.
#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
pub struct Field {
    /// Wire key inside the processor object (snake_case).
    pub name: String,
    /// Form label / tooltip key.
    pub label: String,
    /// Control kind: `select` (with `options`), `text`, or `number`.
    pub control: Control,
    /// When true, the field must have a non-empty value.
    #[serde(default)]
    pub required: bool,
    /// When set, the field is required unless the named sibling field's current
    /// value equals `value`.
    #[serde(default)]
    pub required_unless: Option<RequiredUnless>,
    /// Default value for a freshly dropped node (missing => empty).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default: Option<Value>,
    /// Placeholder for text/number inputs.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub placeholder: Option<String>,
    /// Options for a `select` control.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub options: Vec<Option_>,
    /// User-facing message shown when `required`/`required_unless` fails.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub required_message: Option<String>,
}

/// Supported MVP form controls.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum Control {
    /// Dropdown of `options`.
    Select,
    /// Free-text input.
    Text,
    /// Numeric input.
    Number,
    /// Dynamic dropdown of the version's outcomes. Options are NOT in the
    /// manifest; they are supplied by the client/validator (the
    /// `apply_outcome_ref_exists` rule covers membership).
    OutcomeSelect,
    /// Dynamic searchable single-select of configured sites. Options are NOT in
    /// the manifest; the client queries `GET /api/v1/sites?q=` and stores the
    /// selected site's slug on the processor config.
    SiteSelect,
    /// Dynamic searchable single-select of configured products. Options are NOT
    /// in the manifest; the client queries `GET /api/v1/products?q=` and stores
    /// the selected product's label.
    ProductSelect,
}

/// Conditional-requirement clause: required unless a sibling field equals a value.
#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
pub struct RequiredUnless {
    /// Sibling field name whose value is inspected.
    pub field: String,
    /// The value that, when matched by the sibling, makes this field optional.
    pub value: Value,
}

/// One `select` option.
#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
#[allow(non_camel_case_types)]
pub struct Option_ {
    /// The wire value stored on the processor.
    pub value: Value,
    /// Human label for the option.
    pub label: String,
    /// Optional compact operator glyph shown on the canvas summary (e.g. "==").
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub symbol: Option<String>,
}

/// A node type's output branches.
#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
pub struct Output {
    /// Output branches (yes/no).
    pub branches: Vec<BranchSpec>,
}

/// One output branch (matches the rule_graph `Branch` ids: yes/no).
#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
pub struct BranchSpec {
    /// Branch id (`yes` | `no`).
    pub id: String,
    /// Human label for the branch.
    pub label: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The committed manifest loads, parses to the three ported kinds, and the
    /// raw form preserves the exact (sparse) source JSON.
    #[test]
    fn committed_manifest_loads_and_is_verbatim() {
        let loaded =
            LoadedManifest::load("config/node_types.json").expect("load committed manifest");

        let kinds: Vec<&str> = loaded
            .typed
            .node_types
            .iter()
            .map(|s| s.kind.as_str())
            .collect();
        assert_eq!(
            kinds,
            [
                "meta_tags",
                "device_type",
                "article_url",
                "json_expression",
                "trim_json",
                "add_attribute",
                "apply_outcome",
                "site_match",
                "logged_in",
                "has_product"
            ]
        );

        // The three new node types are expression-kind; the ported decision
        // kinds default to `decision`.
        let by_kind = loaded.typed.index();
        assert_eq!(by_kind["meta_tags"].node_kind, NodeKind::Decision);
        assert_eq!(by_kind["json_expression"].node_kind, NodeKind::Decision);
        assert_eq!(by_kind["trim_json"].node_kind, NodeKind::Expression);
        assert_eq!(by_kind["add_attribute"].node_kind, NodeKind::Expression);
        assert_eq!(by_kind["apply_outcome"].node_kind, NodeKind::Expression);
        // `apply_outcome`'s outcome field uses the dynamic outcome_select control.
        let outcome_field = &by_kind["apply_outcome"].fields[0];
        assert_eq!(outcome_field.control, Control::OutcomeSelect);

        // `site_match` is a decision node in the `request` category whose `site`
        // field uses the dynamic `site_select` control.
        assert_eq!(by_kind["site_match"].node_kind, NodeKind::Decision);
        assert_eq!(by_kind["site_match"].category, "request");
        assert_eq!(by_kind["site_match"].fields[0].control, Control::SiteSelect);

        // `has_product` is a decision node in the `user` category whose `product`
        // field uses the dynamic `product_select` control; `logged_in` is a
        // decision node with no fields.
        assert_eq!(by_kind["has_product"].node_kind, NodeKind::Decision);
        assert_eq!(by_kind["has_product"].category, "user");
        assert_eq!(
            by_kind["has_product"].fields[0].control,
            Control::ProductSelect
        );
        assert_eq!(by_kind["logged_in"].node_kind, NodeKind::Decision);
        assert!(by_kind["logged_in"].fields.is_empty());

        // 15 palette categories; `user` is no longer coming_soon (now that
        // logged_in/has_product ship) and `json` never was.
        assert_eq!(loaded.typed.categories.len(), 15);
        let user = loaded
            .typed
            .categories
            .iter()
            .find(|c| c.id == "user")
            .unwrap();
        assert!(!user.coming_soon);
        let json_cat = loaded
            .typed
            .categories
            .iter()
            .find(|c| c.id == "json")
            .unwrap();
        assert!(!json_cat.coming_soon);

        // applies_to flows from the manifest: meta_tags=html, json_expression=json,
        // article_url defaults to all (key omitted in source).
        assert_eq!(by_kind["meta_tags"].applies_to, AppliesTo::Html);
        assert_eq!(by_kind["json_expression"].applies_to, AppliesTo::Json);
        assert_eq!(by_kind["article_url"].applies_to, AppliesTo::All);
        assert_eq!(by_kind["device_type"].applies_to, AppliesTo::All);

        // Verbatim: the served raw JSON preserves the exact source keys and does
        // not synthesize omitted optional keys. meta_tags' `value` field carries
        // its required_unless clause; device_type's `operator` field omits the
        // optional placeholder/required_unless/required_message keys.
        let raw = &*loaded.raw;
        let value_field = &raw["node_types"][0]["fields"][2];
        assert_eq!(value_field["name"], "value");
        assert!(value_field.get("required_unless").is_some());

        let dev_operator = &raw["node_types"][1]["fields"][0];
        assert_eq!(dev_operator["name"], "operator");
        assert!(dev_operator.get("placeholder").is_none());
        assert!(dev_operator.get("required_unless").is_none());
        assert!(dev_operator.get("required_message").is_none());

        // Verbatim: `applies_to` is present on meta_tags (html) and omitted on the
        // defaulted article_url (the field is not synthesized in the raw output).
        assert_eq!(raw["node_types"][0]["applies_to"], "html");
        assert!(raw["node_types"][2].get("applies_to").is_none());

        // Server-owned display rules: the value truncation length and per-option
        // operator symbols come from the manifest.
        assert_eq!(loaded.typed.display.value_max_chars, 10);
        let meta_op = by_kind["meta_tags"]
            .fields
            .iter()
            .find(|f| f.name == "operator")
            .unwrap();
        let equals = meta_op
            .options
            .iter()
            .find(|o| o.value == serde_json::json!("equals"))
            .unwrap();
        assert_eq!(equals.symbol.as_deref(), Some("=="));
    }

    /// `index` keys every spec by its snake_case kind for O(1) lookup.
    #[test]
    fn index_maps_kind_to_spec() {
        let manifest: NodeManifest = serde_json::from_value(serde_json::json!({
            "categories": [{ "id": "content", "label": "Content" }],
            "node_types": [{
                "kind": "article_url",
                "label": "Article URL",
                "category": "content",
                "summary": "s",
                "fields": [{ "name": "value", "label": "Value", "control": "text", "required": true }],
                "output": { "branches": [{ "id": "yes", "label": "Yes" }, { "id": "no", "label": "No" }] }
            }]
        }))
        .unwrap();

        let idx = manifest.index();
        assert!(idx.contains_key("article_url"));
        assert_eq!(idx["article_url"].fields[0].control, Control::Text);
    }
}
