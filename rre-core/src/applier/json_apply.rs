//! JSON applier orchestrator (parallel to `orchestrator.rs`, but for JSON
//! responses). Runs an outcome's components in order — inline first, then
//! placement-specific, `order_index` ASC within each rank, identical to the HTML
//! path — dispatching on `component.r#type`:
//!
//! - `json_remove { target_path }`  — delete the value at `target_path` if present.
//! - `json_set    { target_path, value }` — upsert (create intermediate objects
//!   for missing `Key` segments, then set the leaf).
//! - `json_replace { target_path, value }` — overwrite ONLY if the path already
//!   resolves to a value.
//! - `html_injection` / `content_truncation` / `html_remove` / unknown — no-op for
//!   JSON (warn + skip).
//!
//! Per-component failure (bad path, type mismatch) is caught and that component
//! is skipped — same fail-open contract as the HTML orchestrator. `applied` is
//! true iff some component changed the body. Idempotent: re-running set/replace
//! with the same value is a no-op; remove on a missing path is a no-op.

use std::collections::HashMap;
use std::sync::Arc;

use serde_json::Value;
use uuid::Uuid;

use crate::applier::json_path::{self, Seg};
use crate::applier::{
    component_ref, component_render, html_injection, html_sanitizer, orchestrator, ApplyError,
    JsonModificationResult,
};
use crate::bundle::{
    ActiveComponent, ActiveOutcome, Placement, ResolvedComponent, VersionSelector,
};

/// Map of PRE-RESOLVED Component templates for one feature's apply pass, keyed by
/// the action's `(component_id, VersionSelector)`. Built on the async side
/// (resolve is a cached await) BEFORE the sync `apply_action_*` runs, so the
/// applier never does I/O in the request hot path. A missing key → the component
/// is skipped (fail-open). Design §4.3.
pub type ResolvedComponentMap = HashMap<(Uuid, VersionSelector), Arc<ResolvedComponent>>;

/// Pre-resolved saved-outcome map for one feature's apply pass, keyed by
/// `saved_outcome_id`. Parallel to `ResolvedComponentMap` but with no version
/// selector (the backend `resolve` route already picked the version).
pub type ResolvedSavedOutcomeMap = HashMap<Uuid, Arc<crate::bundle::ResolvedSavedOutcome>>;

/// Apply an outcome's components to `body`. Always returns `Ok` (per-component
/// failures fail open). `applied` = any component mutated the body.
///
/// `components` is the per-feature PRE-RESOLVED map (design §4.3): a
/// `component_ref_json` component looks up its `(component_id, version)` here on the
/// SYNC side (the resolve `await` already happened async). `sanitizer` cleans the
/// rendered Component HTML before it is set as a string. A `component_ref_json` with
/// an absent key (unresolved) is skipped (fail-open).
pub fn apply_outcome_json(
    body: Value,
    outcome: &ActiveOutcome,
    components: &ResolvedComponentMap,
    sanitizer: &ammonia::Builder<'static>,
) -> Result<JsonModificationResult, ApplyError> {
    let mut ordered: Vec<&ActiveComponent> = outcome.components.iter().collect();
    ordered.sort_by_key(|c| (placement_rank(c.placement), c.order_index));

    let mut current = body;
    let mut applied = false;

    for component in ordered {
        match apply_component(&mut current, component, components, sanitizer) {
            Ok(changed) => applied |= changed,
            Err(e) => {
                tracing::warn!(component_id = %component.id, error = %e, "json component skipped");
            }
        }
    }

    Ok(JsonModificationResult {
        json: current,
        applied,
    })
}

/// Inline placement sorts before sticky_footer/popup (parity with HTML order).
fn placement_rank(p: Placement) -> u8 {
    match p {
        Placement::Inline => 0,
        Placement::StickyFooter => 1,
        Placement::Popup => 2,
    }
}

/// Dispatch a single component. Returns `Ok(true)` if it changed `root`.
/// Unknown / HTML component types are no-ops (warn + `Ok(false)`).
fn apply_component(
    root: &mut Value,
    component: &ActiveComponent,
    components: &ResolvedComponentMap,
    sanitizer: &ammonia::Builder<'static>,
) -> Result<bool, ApplyError> {
    match component.r#type.as_str() {
        "json_remove" => {
            let target = target_path(component)?;
            let segs = json_path::parse(target)?;
            Ok(remove_path(root, &segs))
        }
        "json_set" => {
            let target = target_path(component)?;
            let value = component
                .config
                .get("value")
                .cloned()
                .unwrap_or(Value::Null);
            let segs = json_path::parse(target)?;
            Ok(set_path(root, &segs, value, true))
        }
        "json_replace" => {
            let target = target_path(component)?;
            let value = component
                .config
                .get("value")
                .cloned()
                .unwrap_or(Value::Null);
            let segs = json_path::parse(target)?;
            // Replace only if the path already resolves.
            if resolve(root, &segs).is_none() {
                return Ok(false);
            }
            Ok(set_path(root, &segs, value, false))
        }
        // `component_ref_json` (design §4): render a PRE-RESOLVED Component to a
        // sanitized HTML STRING and SET it at `target_path` (json_set core). A
        // missing resolution / render error skips it (fail-open, Ok(false)).
        "component_ref_json" => {
            let Some(target) = component.config.get("target_path").and_then(Value::as_str) else {
                tracing::warn!(component_id = %component.id, "component_ref_json missing `target_path`, skipped");
                return Ok(false);
            };
            let Some(sanitized) =
                component_ref::render_json_string(component, components, sanitizer)
            else {
                return Ok(false); // unresolved / render error already logged.
            };
            let segs = json_path::parse(target)?;
            Ok(set_path(root, &segs, Value::String(sanitized), true))
        }
        other => {
            tracing::warn!(component_type = %other, "non-json component type, skipped for JSON");
            Ok(false)
        }
    }
}

/// Read `target_path` from a component config (required, non-empty).
fn target_path(component: &ActiveComponent) -> Result<&str, ApplyError> {
    component
        .config
        .get("target_path")
        .and_then(Value::as_str)
        .ok_or(ApplyError::JsonPath(json_path::ParsePathError::Malformed))
}

/// Resolve a path to an immutable reference, or `None` if any segment misses.
fn resolve<'a>(root: &'a Value, segs: &[Seg]) -> Option<&'a Value> {
    let mut cur = root;
    for seg in segs {
        cur = match seg {
            Seg::Key(k) => cur.get(k)?,
            Seg::Index(i) => cur.get(i)?,
        };
    }
    Some(cur)
}

/// Remove the value at `segs`. Returns true if something was removed.
fn remove_path(root: &mut Value, segs: &[Seg]) -> bool {
    let Some((last, parents)) = segs.split_last() else {
        return false;
    };
    let Some(parent) = navigate_mut(root, parents) else {
        return false;
    };
    match last {
        Seg::Key(k) => parent
            .as_object_mut()
            .map(|m| m.remove(k).is_some())
            .unwrap_or(false),
        Seg::Index(i) => match parent.as_array_mut() {
            Some(arr) if *i < arr.len() => {
                arr.remove(*i);
                true
            }
            _ => false,
        },
    }
}

/// Set the value at `segs`. When `create` is true, *missing* (absent) `Key`
/// segments are created as empty objects (upsert); an existing `Key` value that
/// is not a JSON object is a fail-open no-op (we never clobber existing data to
/// force a path). A missing `Index` on a non-array is likewise a fail-open
/// no-op. Returns true if the body changed.
fn set_path(root: &mut Value, segs: &[Seg], value: Value, create: bool) -> bool {
    let Some((last, parents)) = segs.split_last() else {
        return false;
    };

    // Walk/create intermediate containers.
    let mut cur = root;
    for seg in parents {
        match seg {
            Seg::Key(k) => {
                // An existing non-object value here would have to be clobbered
                // to descend -> fail-open skip instead of destroying data.
                if !cur.is_object() {
                    return false;
                }
                let map = cur.as_object_mut().expect("just ensured object");
                if !map.contains_key(k) {
                    if create {
                        map.insert(k.clone(), Value::Object(serde_json::Map::new()));
                    } else {
                        return false;
                    }
                }
                cur = map.get_mut(k).expect("inserted or present");
            }
            Seg::Index(i) => {
                let Some(arr) = cur.as_array_mut() else {
                    // Missing Index on a non-array -> fail-open skip.
                    return false;
                };
                if *i >= arr.len() {
                    return false;
                }
                cur = &mut arr[*i];
            }
        }
    }

    // Set the leaf.
    match last {
        Seg::Key(k) => {
            // Same rule as intermediates: never clobber an existing non-object
            // value at the parent of the leaf key.
            if !cur.is_object() {
                return false;
            }
            let map = cur.as_object_mut().expect("just ensured object");
            if map.get(k) == Some(&value) {
                return false; // idempotent: no change.
            }
            map.insert(k.clone(), value);
            true
        }
        Seg::Index(i) => match cur.as_array_mut() {
            Some(arr) if *i < arr.len() => {
                if arr[*i] == value {
                    return false;
                }
                arr[*i] = value;
                true
            }
            _ => false,
        },
    }
}

/// Navigate to a mutable reference at `segs` WITHOUT creating anything; `None`
/// if any segment misses.
fn navigate_mut<'a>(root: &'a mut Value, segs: &[Seg]) -> Option<&'a mut Value> {
    let mut cur = root;
    for seg in segs {
        cur = match seg {
            Seg::Key(k) => cur.as_object_mut()?.get_mut(k)?,
            Seg::Index(i) => cur.as_array_mut()?.get_mut(*i)?,
        };
    }
    Some(cur)
}

// ---------------------------------------------------------------------------
// Expression-node actions (spec §4): trim_json / add_attribute / apply_outcome.
// The forwarder folds the evaluator's ordered `MatchedAction`s over the body via
// `apply_action_json` (JSON) / `apply_action_html` (HTML).
// ---------------------------------------------------------------------------

/// `trim_json`: truncate the array at `json_path` to `min(actual_len, length)`.
/// A non-array / missing path is a fail-open no-op; `length < 0` is treated as 0.
/// Returns true iff the body changed (the array shrank). Idempotent: a second run
/// with the same `length` is a no-op once `len <= length`.
pub fn trim_json(root: &mut Value, json_path: &str, length: i64) -> Result<bool, ApplyError> {
    let target = length.max(0) as usize;
    let segs = json_path::parse(json_path)?;
    let Some(node) = navigate_mut(root, &segs) else {
        return Ok(false); // missing path -> no-op.
    };
    let Some(arr) = node.as_array_mut() else {
        return Ok(false); // not an array -> no-op.
    };
    let keep = target.min(arr.len());
    if keep == arr.len() {
        return Ok(false); // already short enough -> no change.
    }
    arr.truncate(keep);
    Ok(true)
}

/// `add_attribute`: upsert `value` at `json_path` (= `set_path(create=true)`).
/// Creates missing object parents; replaces an existing value. A `$.a.b` JSONPath
/// is mapped to internal `Seg`s via `json_path::parse`. Returns true iff the body
/// changed. Idempotent: re-setting the same value is a no-op.
pub fn add_attribute(root: &mut Value, json_path: &str, value: Value) -> Result<bool, ApplyError> {
    let segs = json_path::parse(json_path)?;
    Ok(set_path(root, &segs, value, true))
}

/// Apply ONE matched expression action to a JSON body. Dispatches on
/// `action["type"]`:
///
/// - `trim_json     { json_path, length }`
/// - `add_attribute { json_path, value }`
/// - `apply_outcome { outcome_id }` -> look up the outcome and run its components
///   via `apply_outcome_json`.
///
/// Returns whether the body changed. An unknown type / missing config is a
/// fail-open no-op (warn + `false`) — never a panic.
pub fn apply_action_json(
    body: &mut Value,
    action: &Value,
    outcomes: &[ActiveOutcome],
    components: &ResolvedComponentMap,
    saved_outcomes: &ResolvedSavedOutcomeMap,
    sanitizer: &ammonia::Builder<'static>,
) -> bool {
    let Some(kind) = action.get("type").and_then(Value::as_str) else {
        tracing::warn!("expression action missing `type`, skipped");
        return false;
    };
    match kind {
        "trim_json" => {
            let Some(path) = action.get("json_path").and_then(Value::as_str) else {
                tracing::warn!("trim_json action missing `json_path`, skipped");
                return false;
            };
            let length = action_length(action);
            match trim_json(body, path, length) {
                Ok(changed) => changed,
                Err(e) => {
                    tracing::warn!(error = %e, "trim_json failed, skipped");
                    false
                }
            }
        }
        "add_attribute" => {
            let Some(path) = action.get("json_path").and_then(Value::as_str) else {
                tracing::warn!("add_attribute action missing `json_path`, skipped");
                return false;
            };
            let value = action.get("value").cloned().unwrap_or(Value::Null);
            match add_attribute(body, path, value) {
                Ok(changed) => changed,
                Err(e) => {
                    tracing::warn!(error = %e, "add_attribute failed, skipped");
                    false
                }
            }
        }
        "apply_outcome" => match lookup_outcome(action, outcomes) {
            Some(outcome) => {
                let taken = std::mem::replace(body, Value::Null);
                match apply_outcome_json(taken, outcome, components, sanitizer) {
                    Ok(m) => {
                        *body = m.json;
                        m.applied
                    }
                    Err(e) => {
                        tracing::warn!(error = %e, "apply_outcome (json) failed, skipped");
                        false
                    }
                }
            }
            None => {
                tracing::warn!("apply_outcome action: outcome not found, skipped");
                false
            }
        },
        "apply_component_json" => apply_component_json(body, action, components, sanitizer),
        "apply_component" => {
            tracing::warn!("apply_component (html) action on JSON body, skipped");
            false
        }
        "apply_saved_outcome_json" => apply_saved_outcome_json(body, action, saved_outcomes),
        "apply_saved_outcome" => {
            tracing::warn!("apply_saved_outcome (html) action on JSON body, skipped");
            false
        }
        other => {
            tracing::warn!(action_type = %other, "unknown expression action type, skipped");
            false
        }
    }
}

/// `apply_component_json`: render a PRE-RESOLVED Component to an HTML STRING (same
/// render + ammonia sanitize as the HTML path) and set it at `target_path` via the
/// `json_set` core (`add_attribute` = `set_path(create=true)`). Any missing
/// resolution / render error / bad path skips the component (fail-open: body
/// untouched). Returns true iff the body changed. Idempotent: re-setting the same
/// rendered string at the same path is a `set_path` no-op.
fn apply_component_json(
    body: &mut Value,
    action: &Value,
    components: &ResolvedComponentMap,
    sanitizer: &ammonia::Builder<'static>,
) -> bool {
    let Some(key) = component_ref(action) else {
        tracing::warn!("apply_component_json action: bad/absent component_id, skipped");
        return false;
    };
    let Some(resolved) = components.get(&key) else {
        tracing::warn!("apply_component_json action: component not resolved, skipped");
        return false;
    };
    let Some(target) = action.get("target_path").and_then(Value::as_str) else {
        tracing::warn!("apply_component_json action missing `target_path`, skipped");
        return false;
    };
    let Some(sanitized) = render_component(resolved, action, sanitizer) else {
        return false; // render failure already logged.
    };
    match add_attribute(body, target, Value::String(sanitized)) {
        Ok(changed) => changed,
        Err(e) => {
            tracing::warn!(error = %e, "apply_component_json set-at-path failed, skipped");
            false
        }
    }
}

/// Apply ONE matched expression action to an HTML body. `apply_outcome` runs an
/// outcome's components; `apply_component` renders a PRE-RESOLVED Component
/// template and injects it; `trim_json` / `add_attribute` are JSON-only (warn +
/// no-op). `components` is the per-feature pre-resolved map (design §4.3) — a
/// missing key fails open (skip). Returns the (possibly modified) HTML and whether
/// it changed.
pub fn apply_action_html(
    body: String,
    action: &Value,
    outcomes: &[ActiveOutcome],
    components: &ResolvedComponentMap,
    saved_outcomes: &ResolvedSavedOutcomeMap,
    sanitizer: &ammonia::Builder<'static>,
) -> (String, bool) {
    let Some(kind) = action.get("type").and_then(Value::as_str) else {
        tracing::warn!("expression action missing `type`, skipped");
        return (body, false);
    };
    match kind {
        "apply_outcome" => match lookup_outcome(action, outcomes) {
            // Snapshot the body before moving it into the orchestrator so the Err
            // arm can fail open to the ORIGINAL upstream HTML (never an empty body);
            // the success path consumes the clone-free `ModificationResult`.
            Some(outcome) => {
                let original = body.clone();
                match orchestrator::apply_outcome(body, outcome, components, sanitizer) {
                    Ok(m) => (m.html, m.applied),
                    Err(e) => {
                        tracing::warn!(error = %e, "apply_outcome (html) failed, serving original");
                        (original, false)
                    }
                }
            }
            None => {
                tracing::warn!("apply_outcome action: outcome not found, skipped");
                (body, false)
            }
        },
        "apply_component" => apply_component_html(body, action, components, sanitizer),
        "apply_component_json" => {
            tracing::warn!("apply_component_json action on HTML body, skipped");
            (body, false)
        }
        "apply_saved_outcome" => apply_saved_outcome_html(body, action, saved_outcomes),
        "apply_saved_outcome_json" => {
            tracing::warn!("apply_saved_outcome_json action on HTML body, skipped");
            (body, false)
        }
        "trim_json" | "add_attribute" => {
            tracing::warn!(action_type = %kind, "json-only action on HTML body, skipped");
            (body, false)
        }
        other => {
            tracing::warn!(action_type = %other, "unknown expression action type, skipped");
            (body, false)
        }
    }
}

/// `apply_component` (HTML): look up the PRE-RESOLVED Component, render its
/// `html_body` against `action.variables`, ammonia-sanitize, then inject at
/// `target_selector` per `placement_mode` (reusing the `html_injection` core +
/// idempotency marker). Any missing resolution / render error / bad selector skips
/// the component (fail-open: original body returned, never an empty body).
/// Idempotent: a stable per-(component,version) marker makes a second pass a no-op.
fn apply_component_html(
    body: String,
    action: &Value,
    components: &ResolvedComponentMap,
    sanitizer: &ammonia::Builder<'static>,
) -> (String, bool) {
    let Some(key) = component_ref(action) else {
        tracing::warn!("apply_component action: bad/absent component_id, skipped");
        return (body, false);
    };
    let Some(resolved) = components.get(&key) else {
        tracing::warn!("apply_component action: component not resolved, skipped");
        return (body, false);
    };
    let Some(sanitized) = render_component(resolved, action, sanitizer) else {
        return (body, false); // render failure already logged.
    };
    let target_selector = action
        .get("target_selector")
        .and_then(Value::as_str)
        .unwrap_or("");
    let placement_mode = action
        .get("placement_mode")
        .and_then(Value::as_str)
        .unwrap_or("append");
    // Marker keys on (component, resolved version) so the same component injected
    // by two rules at the same version dedupes, while different versions don't.
    let marker = component_marker(key.0, resolved.version_number);
    match html_injection::inject_html(&body, target_selector, placement_mode, &sanitized, &marker) {
        Ok(next) => {
            let changed = next != body;
            (next, changed)
        }
        Err(e) => {
            tracing::warn!(error = %e, "apply_component inject failed, serving original");
            (body, false)
        }
    }
}

/// Stable idempotency marker for an injected Component, keyed by component id +
/// the RESOLVED version number (so `"default"` and a pin resolving to the same
/// version dedupe, and distinct versions inject independently).
fn component_marker(component_id: Uuid, version_number: i32) -> String {
    format!("rc-{component_id}-{version_number}")
}

/// If `action` is an `apply_component` / `apply_component_json` action, return its
/// `(component_id, VersionSelector)` reference for PRE-RESOLUTION on the async
/// side. Returns `None` for any other action type or a malformed/absent
/// `component_id` (which the apply branch then skips, fail-open). Shared by the
/// forwarder, full-journey and eval pre-resolve passes (design §4.3) so the set of
/// resolved refs always matches what the apply branches look up.
pub fn component_ref(action: &Value) -> Option<(Uuid, VersionSelector)> {
    let kind = action.get("type").and_then(Value::as_str)?;
    if kind != "apply_component" && kind != "apply_component_json" {
        return None;
    }
    let id_str = action.get("component_id").and_then(Value::as_str)?;
    let id = Uuid::parse_str(id_str).ok()?;
    let selector = VersionSelector::from_action_value(action.get("version"));
    Some((id, selector))
}

/// If `action` is an `apply_saved_outcome` / `apply_saved_outcome_json` action,
/// return its `saved_outcome_id` for PRE-RESOLUTION on the async side. `None`
/// for any other action type or a malformed/absent id.
pub fn saved_outcome_ref(action: &Value) -> Option<Uuid> {
    let kind = action.get("type").and_then(Value::as_str)?;
    if kind != "apply_saved_outcome" && kind != "apply_saved_outcome_json" {
        return None;
    }
    let id_str = action.get("saved_outcome_id").and_then(Value::as_str)?;
    Uuid::parse_str(id_str).ok()
}

/// Stable idempotency marker for an injected saved outcome, keyed by its id
/// alone (the backend resolve already picked/rendered the version).
fn saved_outcome_marker(id: Uuid) -> String {
    format!("rso-{id}")
}

/// `apply_saved_outcome` (HTML): look up the PRE-RESOLVED, ALREADY-RENDERED
/// saved outcome, sanitize its `html_body`, then inject at `target_selector`
/// per `placement_mode` (reuses the `html_injection` core exactly like
/// `apply_component`). No mustache render here — the backend already rendered
/// it against the saved outcome's own `variables`.
fn apply_saved_outcome_html(
    body: String,
    action: &Value,
    saved_outcomes: &ResolvedSavedOutcomeMap,
) -> (String, bool) {
    let Some(id) = saved_outcome_ref(action) else {
        tracing::warn!("apply_saved_outcome action: bad/absent saved_outcome_id, skipped");
        return (body, false);
    };
    let Some(resolved) = saved_outcomes.get(&id) else {
        tracing::warn!("apply_saved_outcome action: saved outcome not resolved, skipped");
        return (body, false);
    };
    // Trusted-author content, NOT sanitized — see `apply_saved_outcome_json`.
    let sanitized = resolved.html_body.clone();
    let target_selector = action
        .get("target_selector")
        .and_then(Value::as_str)
        .unwrap_or("");
    let placement_mode = action
        .get("placement_mode")
        .and_then(Value::as_str)
        .unwrap_or("append");
    let marker = saved_outcome_marker(id);
    match html_injection::inject_html(&body, target_selector, placement_mode, &sanitized, &marker) {
        Ok(next) => {
            let changed = next != body;
            (next, changed)
        }
        Err(e) => {
            tracing::warn!(error = %e, "apply_saved_outcome inject failed, serving original");
            (body, false)
        }
    }
}

/// `apply_saved_outcome_json`: sanitize the PRE-RESOLVED, ALREADY-RENDERED
/// saved outcome's `html_body` and set it as a STRING at `target_path` via the
/// `json_set` core (`add_attribute`). No mustache render here.
fn apply_saved_outcome_json(
    body: &mut Value,
    action: &Value,
    saved_outcomes: &ResolvedSavedOutcomeMap,
) -> bool {
    let Some(id) = saved_outcome_ref(action) else {
        tracing::warn!("apply_saved_outcome_json action: bad/absent saved_outcome_id, skipped");
        return false;
    };
    let Some(resolved) = saved_outcomes.get(&id) else {
        tracing::warn!("apply_saved_outcome_json action: saved outcome not resolved, skipped");
        return false;
    };
    let Some(target) = action.get("target_path").and_then(Value::as_str) else {
        tracing::warn!("apply_saved_outcome_json action missing `target_path`, skipped");
        return false;
    };
    // NOT sanitized, deliberately. A saved outcome is trusted-author content
    // (the Outcomes Library), and its whole purpose here is to carry the
    // publication's paywall template: <script>/<link>/<meta> ARE the payload.
    // Running ammonia over it leaves the mount point and deletes everything
    // that fills it, which ships an empty paywall to the reader.
    match add_attribute(body, target, Value::String(resolved.html_body.clone())) {
        Ok(changed) => changed,
        Err(e) => {
            tracing::warn!(error = %e, "apply_saved_outcome_json set-at-path failed, skipped");
            false
        }
    }
}

/// Render a resolved Component's `html_body` against the action's `variables` and
/// sanitize the result. `None` on render failure (fail-open). The flat `variables`
/// object on the action holds `{ name: value }`; a missing/non-object `variables`
/// is treated as empty. The rendered HTML is ALWAYS `ammonia`-sanitized before it
/// leaves this function (raw `{{{x}}}` values included), so injection is safe.
fn render_component(
    resolved: &ResolvedComponent,
    action: &Value,
    sanitizer: &ammonia::Builder<'static>,
) -> Option<String> {
    static EMPTY: std::sync::OnceLock<serde_json::Map<String, Value>> = std::sync::OnceLock::new();
    let values = action
        .get("variables")
        .and_then(Value::as_object)
        .unwrap_or_else(|| EMPTY.get_or_init(serde_json::Map::new));
    match component_render::render(&resolved.html_body, values) {
        Ok(rendered) => Some(html_sanitizer::sanitize(sanitizer, &rendered)),
        Err(e) => {
            tracing::warn!(error = %e, "apply_component render failed, skipped");
            None
        }
    }
}

/// Read a numeric `length` from an action config (accepts JSON number or numeric
/// string). Defaults to 0 when absent / unparseable.
fn action_length(action: &Value) -> i64 {
    match action.get("length") {
        Some(Value::Number(n)) => n.as_i64().unwrap_or(0),
        Some(Value::String(s)) => s.trim().parse::<i64>().unwrap_or(0),
        _ => 0,
    }
}

/// Resolve an `apply_outcome` action's `outcome_id` against the active outcomes.
fn lookup_outcome<'a>(action: &Value, outcomes: &'a [ActiveOutcome]) -> Option<&'a ActiveOutcome> {
    let id_str = action
        .get("outcome_id")
        .and_then(Value::as_str)
        .or_else(|| {
            action
                .get("fields")
                .and_then(|f| f.get("outcome_id"))
                .and_then(Value::as_str)
        })?;
    let id = uuid::Uuid::parse_str(id_str).ok()?;
    outcomes.iter().find(|o| o.id == id)
}
