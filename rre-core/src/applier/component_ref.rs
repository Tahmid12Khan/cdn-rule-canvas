//! `component_ref` / `component_ref_json` COMPONENTS (Component Editor, design §4).
//!
//! These are component types (variants of the persisted `ComponentConfig`, tagged
//! by `"type"`) that live INSIDE an outcome's `components[]` — applied through the
//! `apply_outcome` action path, NOT as a standalone rule action (that is the
//! separate `apply_component` / `apply_component_json` ACTION handled in
//! `json_apply`). Both reference a library Component template by
//! `(component_id, version)`; at request time the proxy resolves the template (on
//! the async side, into a [`ResolvedComponentMap`]), mustache-renders its
//! `html_body` against the component's flat `variables`, ammonia-sanitizes, then:
//!
//! - `component_ref` (HTML): injects at `target_selector` with `placement_mode`,
//!   HONORING the row-level placement (inline → inject; sticky_footer/popup → wrap)
//!   exactly like `html_injection`.
//! - `component_ref_json` (JSON): sets the rendered HTML STRING at `target_path`.
//!
//! Everything is fail-open: a missing resolution / render error / bad selector /
//! bad path skips the component (body untouched), never a panic. Idempotent via the
//! same stable per-(component, resolved-version) marker as the `apply_component`
//! action, so a `component_ref` and an `apply_component` action injecting the same
//! resolved version dedupe.

use serde_json::Value;
use uuid::Uuid;

use crate::applier::json_apply::ResolvedComponentMap;
use crate::applier::{
    component_render, html_injection, html_sanitizer, placement_popup, placement_sticky_footer,
    ApplyError,
};
use crate::bundle::{ActiveComponent, Placement, ResolvedComponent, VersionSelector};

/// Render a `component_ref` component into `html`. Resolves the template from the
/// PRE-RESOLVED `components` map, renders + sanitizes it, then injects/wraps per the
/// row-level placement. A missing resolution returns the body UNCHANGED (fail-open,
/// `Ok`); a render error or bad selector propagates as `Err` so the orchestrator
/// logs + skips (still serving the prior body).
pub fn render_html(
    html: &str,
    component: &ActiveComponent,
    components: &ResolvedComponentMap,
    sanitizer: &ammonia::Builder<'static>,
) -> Result<String, ApplyError> {
    let Some(key) = config_ref(&component.config) else {
        tracing::warn!(component_id = %component.id, "component_ref: bad/absent component_id, skipped");
        return Ok(html.to_string());
    };
    let Some(resolved) = components.get(&key) else {
        tracing::warn!(component_id = %component.id, "component_ref: component not resolved, skipped");
        return Ok(html.to_string());
    };
    let sanitized = render_sanitized(resolved, &component.config, sanitizer)?;
    let marker = component_marker(key.0, resolved.version_number);

    match component.placement {
        Placement::StickyFooter => Ok(placement_sticky_footer::wrap_sanitized(
            html, &sanitized, &marker,
        )),
        Placement::Popup => Ok(placement_popup::wrap_sanitized(html, &sanitized, &marker)),
        Placement::Inline => {
            let target_selector = component
                .config
                .get("target_selector")
                .and_then(Value::as_str)
                .unwrap_or("");
            let placement_mode = component
                .config
                .get("placement_mode")
                .and_then(Value::as_str)
                .unwrap_or("append");
            html_injection::inject_html(html, target_selector, placement_mode, &sanitized, &marker)
        }
    }
}

/// Render a `component_ref_json` component to its sanitized HTML STRING (the value
/// to SET at `target_path`). `None` on a missing resolution / render error
/// (fail-open: the JSON applier then skips the component). The caller owns the
/// `json_set` at `target_path`.
pub fn render_json_string(
    component: &ActiveComponent,
    components: &ResolvedComponentMap,
    sanitizer: &ammonia::Builder<'static>,
) -> Option<String> {
    let key = config_ref(&component.config)?;
    let Some(resolved) = components.get(&key) else {
        tracing::warn!(component_id = %component.id, "component_ref_json: component not resolved, skipped");
        return None;
    };
    // render error already logged by render_sanitized.
    render_sanitized(resolved, &component.config, sanitizer).ok()
}

/// Extract a component config's `(component_id, version)` reference for
/// PRE-RESOLUTION on the async side. Returns `None` for a non-`component_ref*` type
/// or a malformed/absent `component_id`. Shared by the forwarder/eval/full-journey
/// pre-resolve passes and the sync renderers so the resolved set matches the lookups.
pub fn config_ref(config: &Value) -> Option<(Uuid, VersionSelector)> {
    let kind = config.get("type").and_then(Value::as_str)?;
    if kind != "component_ref" && kind != "component_ref_json" {
        return None;
    }
    let id_str = config.get("component_id").and_then(Value::as_str)?;
    let id = Uuid::parse_str(id_str).ok()?;
    let selector = VersionSelector::from_action_value(config.get("version"));
    Some((id, selector))
}

/// Render a resolved Component's `html_body` against the config's flat `variables`
/// and ammonia-sanitize the result. `Err(ApplyError::Render)` on a template
/// compile failure (fail-open at the caller). A missing/non-object `variables` is
/// treated as empty. The output is ALWAYS sanitized (raw `{{{x}}}` included).
fn render_sanitized(
    resolved: &ResolvedComponent,
    config: &Value,
    sanitizer: &ammonia::Builder<'static>,
) -> Result<String, ApplyError> {
    static EMPTY: std::sync::OnceLock<serde_json::Map<String, Value>> = std::sync::OnceLock::new();
    let values = config
        .get("variables")
        .and_then(Value::as_object)
        .unwrap_or_else(|| EMPTY.get_or_init(serde_json::Map::new));
    let rendered = component_render::render(&resolved.html_body, values).inspect_err(|e| {
        tracing::warn!(error = %e, "component_ref render failed, skipped");
    })?;
    Ok(html_sanitizer::sanitize(sanitizer, &rendered))
}

/// Stable idempotency marker for an injected `component_ref`, keyed by component id
/// plus the RESOLVED version number — IDENTICAL to the `apply_component` action's
/// marker (`json_apply::component_marker`) so a `component_ref` and an
/// `apply_component` action injecting the same resolved version dedupe.
fn component_marker(component_id: Uuid, version_number: i32) -> String {
    format!("rc-{component_id}-{version_number}")
}
