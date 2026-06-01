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
//! - `html_injection` / `content_truncation` / unknown — no-op for JSON (warn + skip).
//!
//! Per-component failure (bad path, type mismatch) is caught and that component
//! is skipped — same fail-open contract as the HTML orchestrator. `applied` is
//! true iff some component changed the body. Idempotent: re-running set/replace
//! with the same value is a no-op; remove on a missing path is a no-op.

use serde_json::Value;

use crate::domain::applier::json_path::{self, Seg};
use crate::domain::applier::{orchestrator, ApplyError, JsonModificationResult};
use crate::infra::backend_client::{ActiveComponent, ActiveOutcome, Placement};

/// Apply an outcome's components to `body`. Always returns `Ok` (per-component
/// failures fail open). `applied` = any component mutated the body.
pub fn apply_outcome_json(
    body: Value,
    outcome: &ActiveOutcome,
) -> Result<JsonModificationResult, ApplyError> {
    let mut ordered: Vec<&ActiveComponent> = outcome.components.iter().collect();
    ordered.sort_by_key(|c| (placement_rank(c.placement), c.order_index));

    let mut current = body;
    let mut applied = false;

    for component in ordered {
        match apply_component(&mut current, component) {
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
fn apply_component(root: &mut Value, component: &ActiveComponent) -> Result<bool, ApplyError> {
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
pub fn apply_action_json(body: &mut Value, action: &Value, outcomes: &[ActiveOutcome]) -> bool {
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
                match apply_outcome_json(taken, outcome) {
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
        other => {
            tracing::warn!(action_type = %other, "unknown expression action type, skipped");
            false
        }
    }
}

/// Apply ONE matched expression action to an HTML body. Only `apply_outcome` is
/// meaningful for HTML; `trim_json` / `add_attribute` are JSON-only (warn + no-op).
/// Returns the (possibly modified) HTML and whether it changed.
pub fn apply_action_html(
    body: String,
    action: &Value,
    outcomes: &[ActiveOutcome],
    sanitizer: &ammonia::Builder<'static>,
) -> (String, bool) {
    let Some(kind) = action.get("type").and_then(Value::as_str) else {
        tracing::warn!("expression action missing `type`, skipped");
        return (body, false);
    };
    match kind {
        "apply_outcome" => match lookup_outcome(action, outcomes) {
            Some(outcome) => match orchestrator::apply_outcome(body.clone(), outcome, sanitizer) {
                Ok(m) => (m.html, m.applied),
                Err(e) => {
                    tracing::warn!(error = %e, "apply_outcome (html) failed, serving original");
                    (body, false)
                }
            },
            None => {
                tracing::warn!("apply_outcome action: outcome not found, skipped");
                (body, false)
            }
        },
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
