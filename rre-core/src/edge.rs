//! The edge bundle and `apply`, the single per-feature loop every host runs.
//!
//! An `EdgeBundle` is one self-contained document holding everything needed to
//! evaluate a site's published rules with NO further I/O: the canvases, the
//! outcomes, and every Component template and saved outcome they can reach,
//! resolved inline. That is what lets a Fastly Compute guest decide and rewrite
//! a response without calling the RRE backend.
//!
//! `apply` is deliberately the only implementation of the feature loop. The
//! proxy and the edge both call it, so a rule cannot mean one thing in the Test
//! panel and another at the POP.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::applier::json_apply::{self, ResolvedComponentMap, ResolvedSavedOutcomeMap};
use crate::bundle::{
    ActiveOutcome, Applicability, ResolvedComponent, ResolvedSavedOutcome, VersionSelector,
};
use crate::context::EvaluationContextParts;
use crate::graph::{CanvasGraph, Node, RuleGraph};
use crate::identity::Identity;
use crate::processors::default_registry;
use crate::telemetry::{self, FeatureEntry, NodeTiming};

/// Bumped on any incompatible change to the bundle format. A host REJECTS a
/// bundle whose version it does not know rather than guessing at the shape.
pub const SCHEMA_VERSION: u32 = 1;

#[derive(Deserialize, Serialize, Clone, Debug)]
pub struct EdgeBundle {
    pub schema_version: u32,
    pub site: EdgeSite,
    pub environment: String,
    pub generated_at: String,
    pub features: Vec<EdgeFeature>,
}

#[derive(Deserialize, Serialize, Clone, Debug)]
pub struct EdgeSite {
    pub slug: String,
    pub source_host: String,
}

/// One published feature, frozen at export time. `features` is already sorted
/// by `(type, execution_order)`; a host runs it in ARRAY ORDER and never
/// re-sorts, so the ordering decision lives in one place: the exporter.
#[derive(Deserialize, Serialize, Clone, Debug)]
pub struct EdgeFeature {
    pub id: String,
    /// `"html"` or `"json"` — which responses this feature runs against.
    pub r#type: String,
    pub execution_order: i32,
    pub version_number: i32,
    #[serde(default)]
    pub applicability: Applicability,
    pub rule_graph: RuleGraph,
    #[serde(default)]
    pub outcomes: Vec<ActiveOutcome>,
    /// Every Component template the canvas can reach, keyed
    /// `"<uuid>|<default|N>"` (see `VersionSelector::as_query`).
    #[serde(default)]
    pub resolved_components: HashMap<String, ResolvedComponent>,
    /// Every Outcomes-Library outcome the canvas can reach, keyed by its id.
    #[serde(default)]
    pub saved_outcomes: HashMap<Uuid, ResolvedSavedOutcome>,
}

impl EdgeFeature {
    /// The single Rule Canvas.
    pub fn canvas(&self) -> &CanvasGraph {
        &self.rule_graph.canvas
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum BodyKind {
    Html,
    Json,
}

impl BodyKind {
    /// The bundle `type` discriminator this body kind runs.
    pub fn as_str(self) -> &'static str {
        match self {
            BodyKind::Html => "html",
            BodyKind::Json => "json",
        }
    }
}

/// Everything about a request that a rule may branch on.
///
/// Built by the HOST, because the cookie and header names that carry identity
/// are deployment configuration: the proxy reads them from its settings file,
/// the edge from constants. The resolution logic itself is shared
/// (`identity::resolve`), so a visitor who is logged in for the Test panel is
/// logged in at the POP.
#[derive(Clone, Debug)]
pub struct RequestFacts {
    pub headers: http::HeaderMap,
    pub path: String,
    pub cookies: HashMap<String, String>,
    pub site: Option<String>,
    pub identity: Identity,
}

/// Why a feature did nothing. Reported, never fatal.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SkipReason {
    /// The version's applicability gate did not match this body.
    Applicability,
    /// The canvas routed to no expression node.
    NoMatch,
    /// The body was not the kind the feature needs (e.g. unparseable JSON).
    BadBody,
}

impl SkipReason {
    pub fn as_str(self) -> &'static str {
        match self {
            SkipReason::Applicability => "applicability",
            SkipReason::NoMatch => "no_match",
            SkipReason::BadBody => "bad_body",
        }
    }
}

#[derive(Clone, Debug)]
pub struct FeatureReport {
    pub feature_id: String,
    pub version_number: i32,
    /// The canvas routed to at least one expression node.
    pub matched: bool,
    /// An action actually modified the body.
    pub changed: bool,
    pub skipped_reason: Option<SkipReason>,
}

#[derive(Clone, Debug)]
pub struct ApplyOutcome {
    pub body: String,
    pub changed: bool,
    pub features: Vec<FeatureReport>,
    /// The `rre` block, ALREADY injected into `body` (spec §2/v2.2). Returned
    /// as well so a host can put the same numbers in a header or a log line
    /// without re-parsing the body.
    pub total_time_ms: String,
    pub compute_time_ms: String,
    /// `(feature_id, entry)` for every feature that changed the body, in the
    /// order they ran.
    pub feature_expressions: Vec<(String, FeatureEntry)>,
}

/// Evaluate every feature in `bundle` matching `kind`, in bundle order, folding
/// each one's matched actions over the body and chaining the result into the
/// next feature.
///
/// FAIL-OPEN by construction: a feature that does not apply, does not match, or
/// hits a malformed body is recorded in the report and skipped, leaving the body
/// as it was. Never panics on untrusted input — a panic inside a Wasm guest is a
/// 500 for a reader.
pub fn apply(
    bundle: &EdgeBundle,
    facts: &RequestFacts,
    kind: BodyKind,
    body: String,
    sanitizer: &ammonia::Builder<'static>,
) -> ApplyOutcome {
    // `default_registry()`, NOT `ProcessorRegistry::default()`: the latter is a
    // derived Default that registers NOTHING, so every decision node would fail
    // to resolve its processor and the canvas would silently never match.
    let registry = Arc::new(default_registry());
    let mut current = body;
    let mut changed = false;
    let mut features = Vec::new();
    // Same wall-clock boundary the proxy uses: start immediately before the
    // per-feature loop, stop immediately after the injection is computed.
    let engine_start = Instant::now();
    let mut matched_entries: Vec<(String, FeatureEntry)> = Vec::new();

    for feature in bundle.features.iter().filter(|f| f.r#type == kind.as_str()) {
        let canvas = feature.canvas();

        // JSON features work on a parsed value; parse ONCE per feature and only
        // when needed, so an HTML body never pays for it.
        let json_body = match kind {
            BodyKind::Json => match serde_json::from_str::<serde_json::Value>(&current) {
                Ok(v) => Some(v),
                Err(_) => {
                    features.push(report(feature, false, false, Some(SkipReason::BadBody)));
                    continue;
                }
            },
            BodyKind::Html => None,
        };

        if !selector_matches(
            &feature.applicability,
            kind == BodyKind::Json,
            json_body.as_ref(),
            &current,
        ) {
            features.push(report(
                feature,
                false,
                false,
                Some(SkipReason::Applicability),
            ));
            continue;
        }

        // Only parse the HTML DOM for <meta> tags when the canvas actually has a
        // meta_tags node; otherwise skip the parse AND the full-body clone.
        let needs_meta_tags = kind == BodyKind::Html && canvas_has_meta_tags(canvas);
        let body_for_ctx = if kind == BodyKind::Json || needs_meta_tags {
            current.clone()
        } else {
            String::new()
        };
        let mut ctx = EvaluationContextParts::from_request(
            &facts.headers,
            &facts.path,
            &facts.cookies,
            body_for_ctx,
            kind == BodyKind::Json,
            facts.identity.clone(),
        )
        .with_site(facts.site.clone());
        ctx.needs_meta_tags = needs_meta_tags;

        let content = Arc::new(crate::compile(canvas));
        let eval_start = Instant::now();
        let actions = crate::evaluate_sync(content, canvas, ctx, registry.clone());
        let eval_ms = ms_since(eval_start);
        if actions.is_empty() {
            features.push(report(feature, false, false, Some(SkipReason::NoMatch)));
            continue;
        }

        let components = component_map(feature);
        let saved = saved_outcome_map(feature);
        let mut feature_changed = false;
        // Per-node timing (spec §4): time each expression node's apply, so the
        // injected block names the expensive node exactly as the app does.
        let mut timings: Vec<NodeTiming> = Vec::with_capacity(actions.len());

        match kind {
            BodyKind::Html => {
                for ma in &actions {
                    let node_start = Instant::now();
                    let (next, did) = json_apply::apply_action_html(
                        current,
                        &ma.action,
                        &feature.outcomes,
                        &components,
                        &saved,
                        sanitizer,
                    );
                    timings.push(node_timing(canvas, &ma.node_id, node_start));
                    current = next;
                    feature_changed |= did;
                }
            }
            BodyKind::Json => {
                let mut value = json_body.expect("json_body is Some for BodyKind::Json");
                for ma in &actions {
                    let node_start = Instant::now();
                    feature_changed |= json_apply::apply_action_json(
                        &mut value,
                        &ma.action,
                        &feature.outcomes,
                        &components,
                        &saved,
                        sanitizer,
                    );
                    timings.push(node_timing(canvas, &ma.node_id, node_start));
                }
                if feature_changed {
                    match serde_json::to_string(&value) {
                        Ok(s) => current = s,
                        Err(_) => feature_changed = false, // keep the body we had
                    }
                }
            }
        }

        changed |= feature_changed;
        if feature_changed {
            if let Some(entry) =
                telemetry::build_entry(&timings, eval_ms, Some(feature.version_number))
            {
                matched_entries.push((feature.id.clone(), entry));
            }
        }
        features.push(report(feature, true, feature_changed, None));
    }

    // No rule-fetch I/O at the edge — the bundle is already in the binary — so
    // `compute_time_ms` and `total_time_ms` are the same number here. Both keys
    // are emitted anyway: the block must have ONE shape across hosts.
    let elapsed_ms = ms_since(engine_start);
    let total_time_ms = telemetry::fmt_ms(elapsed_ms);
    let compute_time_ms = total_time_ms.clone();

    // Inject the block into the body the reader gets, exactly as the proxy does:
    // `rre.{feature_expressions,total_time_ms,compute_time_ms}` for JSON, a
    // `window.rre` script for HTML. Only when >=1 feature changed the body.
    if !matched_entries.is_empty() {
        current = inject_telemetry(
            current,
            kind,
            &matched_entries,
            &total_time_ms,
            &compute_time_ms,
        );
    }

    ApplyOutcome {
        body: current,
        changed,
        features,
        total_time_ms,
        compute_time_ms,
        feature_expressions: matched_entries,
    }
}

/// Milliseconds elapsed since `start`, as the `f64` the timings carry.
fn ms_since(start: Instant) -> f64 {
    start.elapsed().as_secs_f64() * 1000.0
}

/// One expression node's timing, labelled off the canvas.
fn node_timing(canvas: &CanvasGraph, node_id: &str, start: Instant) -> NodeTiming {
    let (label, custom_label) = telemetry::expression_label(canvas, node_id);
    NodeTiming {
        node_id: node_id.to_string(),
        label,
        custom_label,
        time_ms: ms_since(start),
    }
}

/// Add the `rre` block to the body. Fail-open in every direction: a body that
/// cannot be parsed (JSON) is returned untouched rather than replaced, because
/// losing the reader's article to a debug field is never the right trade.
fn inject_telemetry(
    body: String,
    kind: BodyKind,
    matched: &[(String, FeatureEntry)],
    total_time_ms: &str,
    compute_time_ms: &str,
) -> String {
    match kind {
        BodyKind::Json => {
            let Ok(mut value) = serde_json::from_str::<serde_json::Value>(&body) else {
                return body;
            };
            telemetry::inject_json(&mut value, matched, total_time_ms, compute_time_ms);
            serde_json::to_string(&value).unwrap_or(body)
        }
        BodyKind::Html => telemetry::inject_html(body, matched, total_time_ms, compute_time_ms),
    }
}

fn report(
    feature: &EdgeFeature,
    matched: bool,
    changed: bool,
    skipped_reason: Option<SkipReason>,
) -> FeatureReport {
    FeatureReport {
        feature_id: feature.id.clone(),
        version_number: feature.version_number,
        matched,
        changed,
        skipped_reason,
    }
}

/// Turn the bundle's flat lookup tables into the maps the applier takes. The
/// proxy fills these from its moka caches; at the edge they arrive pre-resolved.
fn component_map(feature: &EdgeFeature) -> ResolvedComponentMap {
    feature
        .resolved_components
        .iter()
        .filter_map(|(key, rc)| parse_component_key(key).map(|k| (k, Arc::new(rc.clone()))))
        .collect()
}

fn saved_outcome_map(feature: &EdgeFeature) -> ResolvedSavedOutcomeMap {
    feature
        .saved_outcomes
        .iter()
        .map(|(id, so)| (*id, Arc::new(so.clone())))
        .collect()
}

/// `"<uuid>|default"` / `"<uuid>|7"` -> the applier's map key. An unparseable
/// key is DROPPED rather than fatal: the action that wanted it then takes its
/// own missing-component skip path, which is already fail-open.
fn parse_component_key(key: &str) -> Option<(Uuid, VersionSelector)> {
    let (id, version) = key.split_once('|')?;
    let id = Uuid::parse_str(id).ok()?;
    let selector = if version.eq_ignore_ascii_case("default") {
        VersionSelector::Default
    } else {
        VersionSelector::Version(version.parse().ok()?)
    };
    Some((id, selector))
}

/// Build a component-table key for `id` at `selector`. The exporter and the
/// consumer MUST agree on this format, so it lives here and both call it.
pub fn component_key(id: Uuid, selector: &VersionSelector) -> String {
    format!("{}|{}", id, selector.as_query())
}

/// True when the canvas has a `meta_tags` decision node, i.e. when evaluation
/// needs the HTML DOM parsed.
pub fn canvas_has_meta_tags(canvas: &CanvasGraph) -> bool {
    canvas.nodes.iter().any(|n| {
        matches!(
            n,
            Node::Decision { processor, .. } if processor.kind == "meta_tags"
        )
    })
}

/// The version-level applicability gate over the CURRENT running body.
///
/// `is_json` selects which selector applies. An absent or empty selector for the
/// active content kind applies always; a MALFORMED selector fails open (applies)
/// rather than silently disabling a published rule. Never panics.
pub fn selector_matches(
    applicability: &Applicability,
    is_json: bool,
    json_body: Option<&serde_json::Value>,
    html_body: &str,
) -> bool {
    if is_json {
        let Some(selector) = applicability
            .json_selector
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
        else {
            return true; // no gate configured -> apply.
        };
        let Some(body) = json_body else {
            return true; // no body to query -> apply (fail-open).
        };
        match serde_json_path::JsonPath::parse(selector) {
            Ok(path) => !path.query(body).all().is_empty(),
            Err(_) => {
                tracing::warn!(selector, "json_selector parse failed, applying (fail-open)");
                true
            }
        }
    } else {
        let Some(selector) = applicability
            .html_selector
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
        else {
            return true; // no gate configured -> apply.
        };
        let sel = match scraper::Selector::parse(selector) {
            Ok(s) => s,
            Err(_) => {
                tracing::warn!(selector, "html_selector parse failed, applying (fail-open)");
                return true;
            }
        };
        let doc = scraper::Html::parse_document(html_body);
        doc.select(&sel).next().is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_component_key_round_trips() {
        let id = Uuid::nil();
        for selector in [VersionSelector::Default, VersionSelector::Version(7)] {
            let key = component_key(id, &selector);
            assert_eq!(parse_component_key(&key), Some((id, selector)));
        }
    }

    /// A malformed key must be dropped, not panic: the table is exported data
    /// and this crate runs where a panic is a reader-visible 500.
    #[test]
    fn parse_component_key_rejects_junk() {
        for junk in ["", "no-pipe", "not-a-uuid|default", "|default"] {
            assert_eq!(parse_component_key(junk), None, "{junk}");
        }
    }

    #[test]
    fn selector_matches_applies_when_no_gate_is_configured() {
        let a = Applicability::default();
        assert!(selector_matches(&a, false, None, "<p>x</p>"));
        assert!(selector_matches(&a, true, Some(&serde_json::json!({})), ""));
    }

    /// A published rule must not be silently disabled by a typo in its gate.
    #[test]
    fn selector_matches_fails_open_on_a_malformed_selector() {
        let html = Applicability {
            html_selector: Some(">>>bad".into()),
            json_selector: None,
        };
        assert!(selector_matches(&html, false, None, "<p>x</p>"));

        let json = Applicability {
            html_selector: None,
            json_selector: Some("$[".into()),
        };
        assert!(selector_matches(
            &json,
            true,
            Some(&serde_json::json!({"a": 1})),
            ""
        ));
    }

    #[test]
    fn selector_matches_gates_on_a_real_selector() {
        let a = Applicability {
            html_selector: Some("#dn-content-ssr".into()),
            json_selector: None,
        };
        assert!(selector_matches(
            &a,
            false,
            None,
            "<div id=dn-content-ssr></div>"
        ));
        assert!(!selector_matches(&a, false, None, "<div id=other></div>"));
    }
}
