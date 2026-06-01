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
use crate::domain::applier::{ApplyError, JsonModificationResult};
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
