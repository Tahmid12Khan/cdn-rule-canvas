//! The forwarder: the single pipeline that ties middleware -> classify -> fetch
//! -> eval -> transform -> re-encode together. The ONLY place that does network
//! IO. Every failure degrades to "serve upstream untouched" — never panics.

use std::collections::HashMap;
use std::time::Instant;

use axum::body::Body;
use axum::extract::State;
use axum::http::{HeaderMap, HeaderName, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use bytes::Bytes;

use crate::domain::applier::json_apply;
use crate::domain::context::EvaluationContextParts;
use crate::domain::evaluator::{GraphEvaluator, MatchedAction};
use crate::domain::features_matched::{self, FeatureEntry, NodeTiming};
use crate::domain::graph::{CanvasGraph, Node};
use crate::error::ProxyError;
use crate::infra::backend_client::{ActiveVersionRead, Applicability, Env};
use crate::infra::encoding;
use crate::state::AppState;

const APPLY_STATUS_HEADER: &str = "x-rre-apply-status";

/// Hop-by-hop headers (RFC 7230 §6.1) stripped in both directions.
const HOP_BY_HOP: &[&str] = &[
    "connection",
    "keep-alive",
    "proxy-authenticate",
    "proxy-authorization",
    "te",
    "trailers",
    "transfer-encoding",
    "upgrade",
];

/// Remove hop-by-hop headers from a header map (in place).
pub fn strip_hop_by_hop(headers: &mut HeaderMap) {
    for name in HOP_BY_HOP {
        headers.remove(*name);
    }
}

/// Header names a configured per-site header may NOT set: the proxy-managed `host`
/// and `content-length` plus every hop-by-hop header (transfer-encoding,
/// connection, etc.). Lets a Site config never override the upstream Host or enable
/// request smuggling. Compared case-insensitively (HTTP header names are ASCII).
fn is_forbidden_site_header(name: &str) -> bool {
    name.eq_ignore_ascii_case("host")
        || name.eq_ignore_ascii_case("content-length")
        || HOP_BY_HOP.iter().any(|h| name.eq_ignore_ascii_case(h))
}

/// axum fallback handler. Returns a fully-built response.
pub async fn forward(State(state): State<AppState>, req: axum::extract::Request) -> Response {
    let e2e_start = Instant::now();
    let path = req.uri().path().to_string();
    let host = host(&req).unwrap_or_default();
    // Source scheme: the proxy terminates plain HTTP; an inbound `x-forwarded-proto`
    // (set by a TLS terminator in front) selects the https default port (443).
    let is_https = req
        .headers()
        .get("x-forwarded-proto")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.eq_ignore_ascii_case("https"))
        .unwrap_or(false);

    // 0. Resolve the upstream destination from the matched Site (by Host header).
    //    No match -> fall back to `upstream_base_url` (dev + tests); `site = None`.
    let route = resolve_route(&state, &host, is_https).await;

    // 1. Fetch the typed feature list (TTL-cached: id + type/kind). Empty list ->
    //    pass-through. The list is filtered to the RESPONSE content kind once the
    //    upstream content-type is known (step 5), so only matching-type features
    //    are evaluated — no pointless cross-type evals (e.g. an HTML feature on a
    //    JSON response) and HALF the active-version fetches per request.
    let feature_list = state.backend.feature_list_cached().await;
    if feature_list.is_empty() {
        return passthrough(&state, &route, req).await;
    }

    // Snapshot request context before consuming the request for upstream fetch.
    let headers = req.headers().clone();
    let cookies = parse_cookies(&headers);
    let identity_settings = crate::domain::identity::IdentitySettings {
        user_cookie: state.settings.identity_user_cookie.clone(),
        products_cookie: state.settings.identity_products_cookie.clone(),
        user_header: state.settings.identity_user_header.clone(),
        products_header: state.settings.identity_products_header.clone(),
    };
    let identity = crate::domain::identity::resolve(&headers, &cookies, &identity_settings);

    // 3. Upstream fetch (once). On error -> typed ProxyError response.
    let upstream = match send_upstream(&state, &route, req).await {
        Ok(u) => u,
        Err(e) => return e.into_response(),
    };

    // 4. Content-kind gate: HTML vs JSON. Neither -> pass-through (skipped).
    //    The gate keys off the RESPONSE content-type, not the feature type:
    //    a json_expression node on an HTML response simply sees no
    //    response_json and returns No (and vice versa for meta_tags on JSON).
    let is_html = upstream.is_html();
    let is_json = upstream.is_json();
    if !is_html && !is_json {
        return upstream.into_response(APPLY_STATUS_HEADER, "skipped");
    }

    let UpstreamResponse {
        status,
        mut resp_headers,
        content_encoding,
        body,
        ..
    } = upstream;

    // Decode for modification (gzip/identity). Unsupported encoding -> pass-through.
    let body_string = match encoding::decode_for_modify(
        content_encoding.as_deref(),
        body.clone(),
        state.settings.max_decompressed_bytes,
    ) {
        Ok(s) => s,
        Err(_) => {
            tracing::warn!(encoding = ?content_encoding, "encoding=unsupported");
            return rebuild_response(status, resp_headers, body, "skipped");
        }
    };

    // 5. Content-type FILTER: keep only features whose `type` matches the response
    //    content kind (`html` features for an HTML response, `json` for JSON). This
    //    removes cross-type evals + their cold active-version fetches entirely.
    //    Count `proxy_requests_total` for the features actually evaluated.
    let want_kind = if is_html { "html" } else { "json" };
    let feature_ids: Vec<String> = feature_list
        .iter()
        .filter(|f| f.kind.eq_ignore_ascii_case(want_kind))
        .map(|f| f.id.clone())
        .collect();
    if feature_ids.is_empty() {
        // No feature of this content kind -> serve upstream untouched (skipped).
        return rebuild_response(status, resp_headers, body, "skipped");
    }
    for feature_id in feature_ids.iter() {
        metrics::counter!("proxy_requests_total", "feature" => feature_id.clone()).increment(1);
    }

    // 5b. Apply each matching feature in order, chaining the running body. The
    //    overall apply_status is "ok" if ANY feature changed the body, else
    //    "skipped" (every feature self-gates: a non-matching feature is a no-op).
    //    `matched` carries each MATCHED feature's (feature_id, feature_expressions
    //    entry) for header + body injection (spec §1/§2).
    let ApplyResult {
        body: final_body,
        any_applied,
        matched,
        total_time_ms: _total_time_ms,
        compute_time_ms: _compute_time_ms,
    } = if is_html {
        apply_features_html(
            &state,
            &feature_ids,
            &headers,
            &path,
            &cookies,
            route.site.as_deref(),
            &identity,
            body_string,
        )
        .await
    } else {
        // JSON: parse once. Parse failure -> serve original, skipped.
        match serde_json::from_str::<serde_json::Value>(&body_string) {
            Ok(parsed) => {
                apply_features_json(
                    &state,
                    &feature_ids,
                    &headers,
                    &path,
                    &cookies,
                    route.site.as_deref(),
                    &identity,
                    parsed,
                    &body_string,
                )
                .await
            }
            Err(_) => {
                tracing::warn!("json parse failed, serving original");
                return rebuild_response(status, resp_headers, body, "skipped");
            }
        }
    };

    let apply_status = if any_applied { "ok" } else { "skipped" };
    metrics::histogram!("proxy_e2e_ms").record(e2e_start.elapsed().as_secs_f64() * 1000.0);

    // 6. Re-encode + rebuild. Stamp a match-marker header for every matched
    //    feature (spec §1). If the upstream said gzip but re-encoding fell back
    //    to identity bytes, drop the now-stale `content-encoding` so the header
    //    matches the body.
    let (out_bytes, gzipped) = encoding::reencode(content_encoding.as_deref(), final_body);
    strip_hop_by_hop(&mut resp_headers);
    if !gzipped {
        resp_headers.remove(axum::http::header::CONTENT_ENCODING);
    }
    set_content_length(&mut resp_headers, out_bytes.len());
    let mut res = rebuild_response(status, resp_headers, out_bytes, apply_status);
    for (feature_id, _) in &matched {
        stamp(&mut res, &format!("x-rre-feature-{feature_id}"), "true");
    }
    res
}

/// The result of folding every matching feature over a response body: the final
/// (chained) body, whether ANY feature changed it, and each MATCHED feature's
/// `feature_expressions` entry (spec §2) keyed by feature_id, in resolution order.
struct ApplyResult {
    body: String,
    any_applied: bool,
    matched: Vec<(String, FeatureEntry)>,
    /// EXACT rule-engine wall-clock across ALL features (spec item 7), `d.dd`.
    /// Measured from immediately before the per-feature loop to immediately after
    /// the feature-expression injection — i.e. the rule-engine boundary (excludes
    /// upstream fetch + response re-encoding). Includes selector gates + context
    /// builds + evals + applies AND the per-feature rule-fetch I/O wait, so it is
    /// legitimately >= Σ feature `time_took_ms`.
    total_time_ms: String,
    /// `total_time_ms` MINUS the rule-fetch I/O wait (the sum of each
    /// `active_version(...)` await), `d.dd`. This is the actual COMPUTE the engine
    /// did (selector gates + evals + applies + injection) with the backend round
    /// trips excluded — so a 34ms cold request whose 32ms was a rule refetch
    /// reports ~2ms compute, matching the warm number. Always <= `total_time_ms`.
    compute_time_ms: String,
}

/// Apply every matching feature's actions to an HTML body, in order, chaining
/// the result. Each feature: active-version lookup (None -> skip), html_selector
/// applicability gate (no match -> skip), eval the canvas, fold its
/// matched actions. Returns the final body and whether ANY feature changed it.
#[allow(clippy::too_many_arguments)]
async fn apply_features_html(
    state: &AppState,
    feature_ids: &[String],
    headers: &HeaderMap,
    path: &str,
    cookies: &HashMap<String, String>,
    site: Option<&str>,
    identity: &crate::domain::identity::Identity,
    mut current: String,
) -> ApplyResult {
    let mut any_applied = false;
    let mut matched: Vec<(String, FeatureEntry)> = Vec::new();
    // EXACT rule-engine wall-clock (spec item 7): start IMMEDIATELY before the
    // per-feature loop, stop IMMEDIATELY after injection is computed below.
    let engine_start = Instant::now();
    // Accumulated rule-fetch I/O wait (each `active_version(...)` await). Subtracted
    // from the wall-clock to derive `compute_time_ms` (engine work only).
    let mut fetch_ms = 0.0_f64;
    for feature_id in feature_ids {
        let fetch_start = Instant::now();
        let av = state.backend.active_version(feature_id, Env::Live).await;
        fetch_ms += fetch_start.elapsed().as_secs_f64() * 1000.0;
        let Some(av) = av else {
            continue;
        };
        if !html_selector_matches(&av, &current) {
            tracing::info!(
                feature_id = %feature_id,
                apply_status = "skipped", reason = "html_selector_no_match", "request"
            );
            continue;
        }
        // Only the HTML body needs parsing when the canvas has a meta_tags
        // node; otherwise skip the parse AND the full-body clone by passing
        // an empty body — the raw HTML is read only to extract meta tags.
        let needs_meta_tags = canvas_has_meta_tags(av.canvas());
        let body_for_ctx = if needs_meta_tags {
            current.clone()
        } else {
            String::new()
        };
        let mut ctx = EvaluationContextParts::from_request(
            headers,
            path,
            cookies,
            body_for_ctx,
            false,
            identity.clone(),
        )
        .with_site(site.map(str::to_string));
        ctx.needs_meta_tags = needs_meta_tags;
        let (actions, eval_ms) = evaluate(state, &av, ctx, feature_id).await;
        if actions.is_empty() {
            log_skipped(feature_id, eval_ms);
            continue;
        }
        let canvas = av.canvas();
        // Pre-resolve every Component reference (apply_component* actions AND
        // component_ref* components inside applied outcomes) on the ASYNC side
        // (cached await) before the sync apply — no I/O in the apply path (§4.3).
        let components = resolve_action_components(state, &actions, &av.outcomes).await;
        let mut applied = false;
        // Per-node timing (spec §4): time each expression node's apply.
        let mut timings: Vec<NodeTiming> = Vec::with_capacity(actions.len());
        for ma in &actions {
            let t_node = Instant::now();
            let (next, changed) = json_apply::apply_action_html(
                current,
                &ma.action,
                &av.outcomes,
                &components,
                &state.sanitizer,
            );
            let time_ms = t_node.elapsed().as_secs_f64() * 1000.0;
            current = next;
            applied |= changed;
            let (label, custom_label) = expression_label(canvas, &ma.node_id);
            timings.push(NodeTiming {
                node_id: ma.node_id.clone(),
                label,
                custom_label,
                time_ms,
            });
        }
        let transform_ms: f64 = timings.iter().map(|t| t.time_ms).sum();
        metrics::histogram!("proxy_transform_ms").record(transform_ms);
        let status = if applied { "ok" } else { "skipped" };
        tracing::info!(
            feature_id = %feature_id,
            actions = actions.len(), eval_ms, transform_ms, apply_status = status, "request"
        );
        any_applied |= applied;
        // The feature is MARKED only when it actually CHANGED the body (spec
        // v2.1 applied-gate) AND it traversed >=1 expression node (build_entry).
        if applied {
            if let Some(entry) =
                features_matched::build_entry(&timings, eval_ms, Some(av.version_number))
            {
                matched.push((feature_id.clone(), entry));
            }
        }
    }
    // total_time_ms boundary: the rule-engine wall-clock spans the loop AND the
    // feature-expression injection. Compute it BEFORE injecting so the injected
    // value reflects the work measured up to (but not including) re-encoding.
    let elapsed_ms = engine_start.elapsed().as_secs_f64() * 1000.0;
    let total_time_ms = features_matched::fmt_ms(elapsed_ms);
    // compute_time_ms = wall-clock EXCLUDING the rule-fetch I/O wait (clamped >= 0).
    let compute_time_ms = features_matched::fmt_ms((elapsed_ms - fetch_ms).max(0.0));
    // Inject `window.rre.{feature_expressions,total_time_ms,compute_time_ms}` as the
    // FINAL step — after all sanitized component transforms (spec §2 sanitizer
    // bypass) — only when >=1 feature matched.
    if !matched.is_empty() {
        current =
            inject_html_feature_expressions(current, &matched, &total_time_ms, &compute_time_ms);
    }
    ApplyResult {
        body: current,
        any_applied,
        matched,
        total_time_ms,
        compute_time_ms,
    }
}

/// Append the trusted `window.rre.{feature_expressions,total_time_ms,compute_time_ms}`
/// script immediately before `</body>` (or at the end of the document if there is
/// none). The serialized map has every `<` escaped to `<` so an embedded
/// `</script>` cannot break out — XSS-safe; this is first-party trusted data
/// (spec §2). `total_time_ms` and `compute_time_ms` are top-level SIBLINGS of
/// `feature_expressions` (spec item 7; `compute_time_ms` = total minus rule-fetch I/O).
fn inject_html_feature_expressions(
    body: String,
    matched: &[(String, FeatureEntry)],
    total_time_ms: &str,
    compute_time_ms: &str,
) -> String {
    let map: serde_json::Map<String, serde_json::Value> = matched
        .iter()
        .map(|(id, entry)| (id.clone(), serde_json::to_value(entry).unwrap_or_default()))
        .collect();
    let json = serde_json::to_string(&serde_json::Value::Object(map))
        .unwrap_or_else(|_| "{}".to_string())
        .replace('<', "\\u003c");
    // The time fields are `d.dd` strings; serialize so each is a quoted JSON string
    // (escaping any `<` for the same break-out safety).
    let total_json = serde_json::to_string(total_time_ms)
        .unwrap_or_else(|_| "\"0.00\"".to_string())
        .replace('<', "\\u003c");
    let compute_json = serde_json::to_string(compute_time_ms)
        .unwrap_or_else(|_| "\"0.00\"".to_string())
        .replace('<', "\\u003c");
    let script = format!(
        "<script>window.rre=window.rre||{{}};window.rre.feature_expressions={json};window.rre.total_time_ms={total_json};window.rre.compute_time_ms={compute_json};</script>"
    );
    match body.rfind("</body>") {
        Some(idx) => {
            let mut out = String::with_capacity(body.len() + script.len());
            out.push_str(&body[..idx]);
            out.push_str(&script);
            out.push_str(&body[idx..]);
            out
        }
        None => {
            let mut out = body;
            out.push_str(&script);
            out
        }
    }
}

/// Apply every matching feature's actions to a JSON body, in order, chaining the
/// result. Each feature: active-version lookup (None -> skip), json_selector
/// applicability gate (no match -> skip), eval against the CURRENT (chained)
/// body so a later feature sees an earlier one's edits, fold its matched actions.
/// Returns the serialized final body and whether ANY feature changed it.
#[allow(clippy::too_many_arguments)]
async fn apply_features_json(
    state: &AppState,
    feature_ids: &[String],
    headers: &HeaderMap,
    path: &str,
    cookies: &HashMap<String, String>,
    site: Option<&str>,
    identity: &crate::domain::identity::Identity,
    mut current: serde_json::Value,
    original: &str,
) -> ApplyResult {
    let mut any_applied = false;
    let mut matched: Vec<(String, FeatureEntry)> = Vec::new();
    // EXACT rule-engine wall-clock (spec item 7): start IMMEDIATELY before the
    // per-feature loop, stop IMMEDIATELY after injection is computed below.
    let engine_start = Instant::now();
    // Accumulated rule-fetch I/O wait (each `active_version(...)` await). Subtracted
    // from the wall-clock to derive `compute_time_ms` (engine work only).
    let mut fetch_ms = 0.0_f64;
    for feature_id in feature_ids {
        let fetch_start = Instant::now();
        let av = state.backend.active_version(feature_id, Env::Live).await;
        fetch_ms += fetch_start.elapsed().as_secs_f64() * 1000.0;
        let Some(av) = av else {
            continue;
        };
        if !json_selector_matches(&av, &current) {
            tracing::info!(
                feature_id = %feature_id,
                apply_status = "skipped", reason = "json_selector_no_match", "request"
            );
            continue;
        }
        // Eval reads response_json from the body string, so feed it the CURRENT
        // (already-chained) body — not the original — for correct chaining.
        let ctx_body = serde_json::to_string(&current).unwrap_or_default();
        let ctx = EvaluationContextParts::from_request(
            headers,
            path,
            cookies,
            ctx_body,
            true,
            identity.clone(),
        )
        .with_site(site.map(str::to_string));
        let (actions, eval_ms) = evaluate(state, &av, ctx, feature_id).await;
        if actions.is_empty() {
            log_skipped(feature_id, eval_ms);
            continue;
        }
        let canvas = av.canvas();
        // Pre-resolve component references on the async side (design §4.3):
        // apply_component* actions AND component_ref* components inside outcomes.
        let components = resolve_action_components(state, &actions, &av.outcomes).await;
        let mut applied = false;
        // Per-node timing (spec §4): time each expression node's apply.
        let mut timings: Vec<NodeTiming> = Vec::with_capacity(actions.len());
        for ma in &actions {
            let t_node = Instant::now();
            let changed = json_apply::apply_action_json(
                &mut current,
                &ma.action,
                &av.outcomes,
                &components,
                &state.sanitizer,
            );
            let time_ms = t_node.elapsed().as_secs_f64() * 1000.0;
            applied |= changed;
            let (label, custom_label) = expression_label(canvas, &ma.node_id);
            timings.push(NodeTiming {
                node_id: ma.node_id.clone(),
                label,
                custom_label,
                time_ms,
            });
        }
        let transform_ms: f64 = timings.iter().map(|t| t.time_ms).sum();
        metrics::histogram!("proxy_transform_ms").record(transform_ms);
        let status = if applied { "ok" } else { "skipped" };
        tracing::info!(
            feature_id = %feature_id,
            actions = actions.len(), eval_ms, transform_ms, apply_status = status, "request"
        );
        any_applied |= applied;
        // The feature is MARKED only when it actually CHANGED the body (spec
        // v2.1 applied-gate) AND it traversed >=1 expression node (build_entry).
        if applied {
            if let Some(entry) =
                features_matched::build_entry(&timings, eval_ms, Some(av.version_number))
            {
                matched.push((feature_id.clone(), entry));
            }
        }
    }
    // total_time_ms boundary: the rule-engine wall-clock spans the loop AND the
    // feature-expression injection. Compute it BEFORE injecting so the injected
    // value reflects the work measured up to (but not including) re-serialization.
    let elapsed_ms = engine_start.elapsed().as_secs_f64() * 1000.0;
    let total_time_ms = features_matched::fmt_ms(elapsed_ms);
    // compute_time_ms = wall-clock EXCLUDING the rule-fetch I/O wait (clamped >= 0).
    let compute_time_ms = features_matched::fmt_ms((elapsed_ms - fetch_ms).max(0.0));
    // Inject `body.rre.{feature_expressions,total_time_ms,compute_time_ms}` (spec
    // v2.2 / item 7): create `rre` if absent, merge without clobbering other
    // `rre.*` keys. Only when >=1 feature matched.
    if !matched.is_empty() {
        inject_json_feature_expressions(&mut current, &matched, &total_time_ms, &compute_time_ms);
    }
    // Serialize the final chained body. A serialize error (≈never for a Value)
    // falls back to the original untouched body.
    match serde_json::to_string(&current) {
        Ok(s) => ApplyResult {
            body: s,
            any_applied,
            matched,
            total_time_ms,
            compute_time_ms,
        },
        Err(e) => {
            metrics::counter!("proxy_apply_errors_total").increment(1);
            tracing::warn!(error = %e, "json serialize failed, serving original");
            ApplyResult {
                body: original.to_string(),
                any_applied: false,
                matched: Vec::new(),
                total_time_ms: features_matched::fmt_ms(0.0),
                compute_time_ms: features_matched::fmt_ms(0.0),
            }
        }
    }
}

/// Merge the matched features' entries into `body["rre"]["feature_expressions"]`
/// (spec v2.2) and set `body["rre"]["total_time_ms"]` + `["compute_time_ms"]` (spec
/// item 7) as top-level SIBLINGS. Creates the `rre` object if absent; sets the keys
/// without clobbering other `rre.*` keys. A non-object `body` is left untouched.
fn inject_json_feature_expressions(
    body: &mut serde_json::Value,
    matched: &[(String, FeatureEntry)],
    total_time_ms: &str,
    compute_time_ms: &str,
) {
    let Some(root) = body.as_object_mut() else {
        return; // top-level non-object body: nothing to namespace under.
    };
    let rre = root
        .entry("rre")
        .or_insert_with(|| serde_json::Value::Object(serde_json::Map::new()));
    // If `rre` exists but is not an object, replace it (reserved namespace).
    if !rre.is_object() {
        *rre = serde_json::Value::Object(serde_json::Map::new());
    }
    let rre_obj = rre.as_object_mut().expect("just ensured object");
    let fe: serde_json::Map<String, serde_json::Value> = matched
        .iter()
        .map(|(id, entry)| (id.clone(), serde_json::to_value(entry).unwrap_or_default()))
        .collect();
    rre_obj.insert(
        "feature_expressions".to_string(),
        serde_json::Value::Object(fe),
    );
    rre_obj.insert(
        "total_time_ms".to_string(),
        serde_json::Value::String(total_time_ms.to_string()),
    );
    rre_obj.insert(
        "compute_time_ms".to_string(),
        serde_json::Value::String(compute_time_ms.to_string()),
    );
}

/// Run the canvas through the evaluator, returning the ordered matched
/// expression actions and the eval duration in ms. Records `proxy_eval_ms`.
async fn evaluate(
    state: &AppState,
    av: &ActiveVersionRead,
    ctx: EvaluationContextParts,
    feature_id: &str,
) -> (Vec<MatchedAction>, f64) {
    let canvas_graph = av.canvas();
    let eval_start = Instant::now();
    let evaluator = GraphEvaluator::new(state.registry.clone(), &state.compiled);
    let actions = evaluator
        .evaluate(canvas_graph, ctx, feature_id, av.version_number)
        .await;
    let eval_ms = eval_start.elapsed().as_secs_f64() * 1000.0;
    metrics::histogram!("proxy_eval_ms").record(eval_ms);
    (actions, eval_ms)
}

/// Pre-resolve every Component-template reference touched by `actions` into a
/// `ResolvedComponentMap`, ON THE ASYNC SIDE (design §4.3). Two ref sources:
///
/// 1. ACTION refs — an `apply_component` / `apply_component_json` action's own
///    `(component_id, version)`.
/// 2. OUTCOME-COMPONENT refs — a `component_ref` / `component_ref_json` COMPONENT
///    inside an outcome referenced by an `apply_outcome` action. These render
///    during outcome application, so they must be resolved too.
///
/// Each distinct `(component_id, version)` is resolved at most once (the map
/// dedupes); `resolve_component` is itself a cached await (SWR), so a warm component
/// is a HashMap hit with no I/O. A reference that fails to resolve (404/error) is
/// simply absent from the map → the sync apply branch skips it (fail-open). Never
/// panics.
pub(crate) async fn resolve_action_components(
    state: &AppState,
    actions: &[MatchedAction],
    outcomes: &[crate::infra::backend_client::ActiveOutcome],
) -> json_apply::ResolvedComponentMap {
    let mut map = json_apply::ResolvedComponentMap::new();
    // Collect every distinct ref first (dedup), then resolve once each.
    let mut keys: Vec<(uuid::Uuid, crate::infra::backend_client::VersionSelector)> = Vec::new();
    let mut push = |key| {
        if !keys.contains(&key) {
            keys.push(key);
        }
    };
    for ma in actions {
        // 1. Direct action ref.
        if let Some(key) = json_apply::component_ref(&ma.action) {
            push(key);
        }
        // 2. component_ref/component_ref_json components inside an applied outcome.
        if ma.action.get("type").and_then(serde_json::Value::as_str) == Some("apply_outcome") {
            if let Some(outcome) = lookup_applied_outcome(&ma.action, outcomes) {
                for component in &outcome.components {
                    if let Some(key) =
                        crate::domain::applier::component_ref::config_ref(&component.config)
                    {
                        push(key);
                    }
                }
            }
        }
    }
    for key in keys {
        if let Some(resolved) = state.component_cache.resolve_component(key.0, key.1).await {
            map.insert(key, resolved);
        }
    }
    map
}

/// Resolve an `apply_outcome` action's `outcome_id` against `outcomes` (mirrors the
/// applier's lookup, including the `fields.outcome_id` fallback) so the pre-resolve
/// pass can walk the applied outcome's components for `component_ref*` refs.
fn lookup_applied_outcome<'a>(
    action: &serde_json::Value,
    outcomes: &'a [crate::infra::backend_client::ActiveOutcome],
) -> Option<&'a crate::infra::backend_client::ActiveOutcome> {
    let id_str = action
        .get("outcome_id")
        .and_then(serde_json::Value::as_str)
        .or_else(|| {
            action
                .get("fields")
                .and_then(|f| f.get("outcome_id"))
                .and_then(serde_json::Value::as_str)
        })?;
    let id = uuid::Uuid::parse_str(id_str).ok()?;
    outcomes.iter().find(|o| o.id == id)
}

/// Structured "no modification" log shared by both content kinds (no matched
/// expression actions on the routed path).
fn log_skipped(feature_id: &str, eval_ms: f64) {
    tracing::info!(
        feature_id = %feature_id,
        actions = 0,
        eval_ms,
        apply_status = "skipped",
        "request"
    );
}

/// HTML applicability gate. True when `html_selector` is absent/empty (apply
/// always) OR it parses and matches >=1 element. A malformed selector fails open
/// (apply). Never panics.
fn html_selector_matches(av: &ActiveVersionRead, body: &str) -> bool {
    selector_matches(&av.applicability, false, None, body)
}

/// JSON applicability gate. True when `json_selector` is absent/empty (apply
/// always) OR it parses and matches >=1 node. A malformed selector fails open
/// (apply). Never panics.
fn json_selector_matches(av: &ActiveVersionRead, body: &serde_json::Value) -> bool {
    selector_matches(&av.applicability, true, Some(body), "")
}

/// Shared applicability gate over an `&Applicability` + the CURRENT running body.
/// `is_json` selects which selector applies: for JSON, `json_body` must be
/// `Some` and the `json_selector` is queried against it; for HTML, the
/// `html_selector` is matched against `html_body`. An absent/empty selector for
/// the active content kind applies always (`true`); a malformed selector fails
/// open (`true`). Never panics. Shared by production (`html_selector_matches`/
/// `json_selector_matches`) and the full-journey endpoint so both gate identically.
pub(crate) fn selector_matches(
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

/// Upstream response captured for modification or pass-through.
pub(crate) struct UpstreamResponse {
    status: StatusCode,
    resp_headers: HeaderMap,
    content_encoding: Option<String>,
    content_type: Option<String>,
    body: Bytes,
}

impl UpstreamResponse {
    fn is_html(&self) -> bool {
        self.content_type
            .as_deref()
            .map(|ct| ct.to_ascii_lowercase().contains("text/html"))
            .unwrap_or(false)
    }

    fn is_json(&self) -> bool {
        self.content_type
            .as_deref()
            .map(|ct| ct.to_ascii_lowercase().contains("application/json"))
            .unwrap_or(false)
    }

    /// Build a response from an unmodified upstream body, stamping apply-status.
    fn into_response(self, status_header: &str, apply: &str) -> Response {
        let mut headers = self.resp_headers;
        strip_hop_by_hop(&mut headers);
        let mut res = build_with_headers(self.status, headers, self.body);
        stamp(&mut res, status_header, apply);
        res
    }
}

/// The resolved upstream destination for a request: the upstream base URL
/// (`scheme://host:port`), the destination Host header to set, and the matched
/// Site slug (`None` on fallback to `settings.upstream_base_url`).
pub(crate) struct Route {
    /// Upstream base URL: `dest_protocol://dest_host:dest_port` (matched Site) or
    /// `settings.upstream_base_url` (fallback).
    base_url: String,
    /// Host header to forward upstream: the destination host (matched Site) or
    /// the fallback base URL's authority host.
    host: Option<String>,
    /// Matched Site slug, threaded into `EvaluationContext.site`. `None` on fallback.
    pub site: Option<String>,
    /// Per-site custom headers to inject on the upstream request (matched Site
    /// only). Each configured header OVERRIDES any client-supplied same-named
    /// header. Empty on the no-match fallback path (apply none).
    headers: HashMap<String, String>,
}

/// Resolve the upstream destination from the inbound `Host` header. A matched
/// Site drives the upstream scheme+authority + the `site` tag; no match falls
/// back to `settings.upstream_base_url` with `site = None` (preserves dev/tests).
async fn resolve_route(state: &AppState, host: &str, is_https: bool) -> Route {
    let index = state.site_map.index().await;
    if let Some(site) = index.lookup(host, is_https) {
        return Route {
            base_url: site.dest_base_url(),
            host: Some(site.dest_host.clone()),
            site: Some(site.slug.clone()),
            headers: site.headers.clone(),
        };
    }
    // Fallback: the static upstream. Derive the Host header from its authority so
    // a vhosted upstream still receives the expected Host. No custom headers on
    // the no-match path.
    let base = state.settings.upstream_base_url.clone();
    let host = base
        .parse::<axum::http::Uri>()
        .ok()
        .and_then(|u| u.host().map(str::to_string));
    Route {
        base_url: base,
        host,
        site: None,
        headers: HashMap::new(),
    }
}

/// Fetch the upstream response and capture it fully (HTML buffered in memory).
/// The upstream authority+scheme comes from the resolved [`Route`].
pub(crate) async fn send_upstream(
    state: &AppState,
    route: &Route,
    req: axum::extract::Request,
) -> Result<UpstreamResponse, ProxyError> {
    let (parts, body) = req.into_parts();

    let body_bytes = axum::body::to_bytes(body, 1024 * 1024)
        .await
        .map_err(|_| ProxyError::UpstreamProtocol)?;

    let url = format!(
        "{}{}",
        route.base_url.trim_end_matches('/'),
        parts
            .uri
            .path_and_query()
            .map(|pq| pq.as_str())
            .unwrap_or("/")
    );

    let method = reqwest::Method::from_bytes(parts.method.as_str().as_bytes())
        .map_err(|_| ProxyError::UpstreamProtocol)?;

    let mut builder = state.http.request(method, &url);
    for (name, value) in parts.headers.iter() {
        if HOP_BY_HOP.contains(&name.as_str()) || name.as_str() == "host" {
            continue;
        }
        if let Ok(v) = reqwest::header::HeaderValue::from_bytes(value.as_bytes()) {
            if let Ok(n) = reqwest::header::HeaderName::from_bytes(name.as_str().as_bytes()) {
                builder = builder.header(n, v);
            }
        }
    }
    // Set the upstream Host header to the destination host (the inbound `host`
    // header was stripped above). A vhosted upstream routes by this Host.
    if let Some(dest_host) = &route.host {
        if let Ok(v) = reqwest::header::HeaderValue::from_str(dest_host) {
            builder = builder.header(reqwest::header::HOST, v);
        }
    }
    if !body_bytes.is_empty() {
        builder = builder.body(body_bytes.to_vec());
    }

    // Build the concrete request so the matched Site's per-site headers can be
    // applied AFTER the inbound copy + Host set. For each ALLOWED configured header
    // we FIRST remove the name (drops any client-supplied value), THEN insert the
    // configured value if it parses. So a configured header OVERRIDES a client value
    // (a client cannot spoof a header the site sets) AND a malformed config leaves
    // the header ABSENT rather than leaking the client value (fail-closed). The
    // no-match fallback carries an empty map, so this is a no-op there. Backend
    // already validates names/values + rejects forbidden names; the denylist +
    // parse guards here are defense-in-depth -> skip + warn, never fail the request.
    let mut request = builder.build().map_err(|_| ProxyError::UpstreamProtocol)?;
    if !route.headers.is_empty() {
        let headers_mut = request.headers_mut();
        for (name, value) in &route.headers {
            // Denylist: a config may never set the proxy-managed Host /
            // content-length or any hop-by-hop header (request-smuggling guard).
            if is_forbidden_site_header(name) {
                tracing::warn!(
                    site = route.site.as_deref().unwrap_or(""),
                    header = %name,
                    "site_header=skipped (forbidden name)"
                );
                continue;
            }
            let Ok(n) = reqwest::header::HeaderName::from_bytes(name.as_bytes()) else {
                tracing::warn!(
                    site = route.site.as_deref().unwrap_or(""),
                    header = %name,
                    "site_header=skipped (unparseable name)"
                );
                continue;
            };
            // Fail-closed: drop any client-supplied value FIRST, so a parse failure
            // on the configured value leaves the header absent (no client spoof).
            headers_mut.remove(&n);
            match reqwest::header::HeaderValue::from_str(value) {
                Ok(v) => {
                    headers_mut.insert(n, v);
                }
                Err(_) => {
                    tracing::warn!(
                        site = route.site.as_deref().unwrap_or(""),
                        header = %name,
                        "site_header=skipped (unparseable value), client value dropped"
                    );
                }
            }
        }
    }

    let mut resp = state
        .http
        .execute(request)
        .await
        .map_err(map_reqwest_error)?;

    let status = StatusCode::from_u16(resp.status().as_u16()).unwrap_or(StatusCode::BAD_GATEWAY);
    let content_encoding = header_string(resp.headers(), reqwest::header::CONTENT_ENCODING);
    let content_type = header_string(resp.headers(), reqwest::header::CONTENT_TYPE);

    let mut resp_headers = HeaderMap::new();
    for (name, value) in resp.headers().iter() {
        if let Ok(n) = HeaderName::from_bytes(name.as_str().as_bytes()) {
            if let Ok(v) = HeaderValue::from_bytes(value.as_bytes()) {
                resp_headers.insert(n, v);
            }
        }
    }

    // Bound the buffered upstream body. Reject up front when the advertised
    // Content-Length already exceeds the cap, AND bound the actual read so a
    // chunked/streamed body cannot grow past the budget (a 502 is cleaner than
    // serving a truncated body).
    let cap = state.settings.max_upstream_body_bytes;
    if resp.content_length().is_some_and(|len| len as usize > cap) {
        return Err(ProxyError::UpstreamTooLarge);
    }
    let mut buf: Vec<u8> = Vec::new();
    while let Some(chunk) = resp.chunk().await.map_err(map_reqwest_error)? {
        if buf.len() + chunk.len() > cap {
            return Err(ProxyError::UpstreamTooLarge);
        }
        buf.extend_from_slice(&chunk);
    }
    let body = Bytes::from(buf);

    Ok(UpstreamResponse {
        status,
        resp_headers,
        content_encoding,
        content_type,
        body,
    })
}

/// Forward a request upstream and return the untouched response (skipped).
async fn passthrough(state: &AppState, route: &Route, req: axum::extract::Request) -> Response {
    match send_upstream(state, route, req).await {
        Ok(u) => u.into_response(APPLY_STATUS_HEADER, "skipped"),
        Err(e) => e.into_response(),
    }
}

fn map_reqwest_error(e: reqwest::Error) -> ProxyError {
    if e.is_timeout() {
        ProxyError::UpstreamTimeout
    } else if e.is_connect() {
        ProxyError::UpstreamUnavailable
    } else {
        ProxyError::UpstreamProtocol
    }
}

fn header_string(
    headers: &reqwest::header::HeaderMap,
    name: reqwest::header::HeaderName,
) -> Option<String> {
    headers
        .get(name)
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string())
}

/// Build an axum response with the given status/headers/body.
fn build_with_headers(status: StatusCode, headers: HeaderMap, body: Bytes) -> Response {
    let mut res = Response::new(Body::from(body));
    *res.status_mut() = status;
    *res.headers_mut() = headers;
    res
}

/// Rebuild the final response: body + headers + apply-status stamp.
fn rebuild_response(status: StatusCode, headers: HeaderMap, body: Bytes, apply: &str) -> Response {
    let mut res = build_with_headers(status, headers, body);
    stamp(&mut res, APPLY_STATUS_HEADER, apply);
    res
}

/// Stamp a header value on a response (best-effort).
fn stamp(res: &mut Response, header: &str, value: &str) {
    if let (Ok(name), Ok(val)) = (
        HeaderName::from_bytes(header.as_bytes()),
        HeaderValue::from_str(value),
    ) {
        res.headers_mut().insert(name, val);
    }
}

fn set_content_length(headers: &mut HeaderMap, len: usize) {
    headers.remove(axum::http::header::CONTENT_LENGTH);
    if let Ok(v) = HeaderValue::from_str(&len.to_string()) {
        headers.insert(axum::http::header::CONTENT_LENGTH, v);
    }
}

/// Resolve the request host (Host header or URI authority), as received.
/// Returned verbatim — case-insensitive lookups happen downstream
/// (`parse_host_header` lowercases the host before indexing).
fn host(req: &axum::extract::Request) -> Option<String> {
    req.headers()
        .get(axum::http::header::HOST)
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string())
        .or_else(|| req.uri().authority().map(|a| a.as_str().to_string()))
}

/// Convert a `{name: value}` test-header map into a `HeaderMap`, skipping any
/// entry whose name or value is not a valid HTTP header (defensive — the frontend
/// already validates header names/values to the same rules as Site headers).
/// Shared by `/__rre/eval-url` and `/__rre/eval-full-journey`.
pub(crate) fn headers_to_map(headers: &HashMap<String, String>) -> HeaderMap {
    let mut map = HeaderMap::new();
    for (name, value) in headers {
        if let (Ok(n), Ok(v)) = (
            HeaderName::from_bytes(name.as_bytes()),
            HeaderValue::from_str(value),
        ) {
            map.insert(n, v);
        }
    }
    map
}

/// Parse all request cookies into a map.
pub(crate) fn parse_cookies(headers: &HeaderMap) -> HashMap<String, String> {
    let mut map = HashMap::new();
    for header in headers.get_all(axum::http::header::COOKIE).iter() {
        let Ok(s) = header.to_str() else { continue };
        for pair in s.split(';') {
            let pair = pair.trim();
            if let Some((k, v)) = pair.split_once('=') {
                map.insert(k.trim().to_string(), v.trim().to_string());
            }
        }
    }
    map
}

/// True when the canvas has at least one `meta_tags` decision node — the only
/// reason to parse the HTML DOM for `<meta>` tags. When false, the evaluator
/// skips both the parse and the full-body clone.
fn canvas_has_meta_tags(canvas: &CanvasGraph) -> bool {
    canvas.nodes.iter().any(|n| {
        matches!(
            n,
            Node::Decision { processor, .. } if processor.kind == "meta_tags"
        )
    })
}

/// Display label + custom label for an expression node (spec §4/v2.3): the
/// manifest label for the action kind (the proxy has no manifest at runtime, so
/// the action kind IS the label — fieldless fallback) plus the node's
/// `custom_label`. Mirrors the `eval.rs` `label()` helper — a node-id->label
/// lookup over the canvas.
fn expression_label(canvas: &CanvasGraph, node_id: &str) -> (String, Option<String>) {
    match canvas.nodes.iter().find(|n| n.id() == node_id) {
        Some(Node::Expression {
            action,
            custom_label,
            ..
        }) => (action.kind.clone(), custom_label.clone()),
        _ => (node_id.to_string(), None),
    }
}

// ---------------------------------------------------------------------------
// Eval-URL support (`POST /__rre/eval-url`): fetch a REAL upstream response
// through the proxy so the test panel can run the editor canvas against live
// content. Reuses `send_upstream` (so a matched Site's headers override the test
// headers EXACTLY like production traffic) but the caller evaluates the
// request-body canvas instead of the active saved features.
// ---------------------------------------------------------------------------

/// A decoded upstream body fetched for `/__rre/eval-url`, plus the request facts
/// the evaluator needs. The test headers stand in for the simulated client
/// request (Site headers are upstream-injected and, like production, are NOT part
/// of the eval context).
pub(crate) struct EvalFetch {
    /// Matched Site slug (always `Some` — eval-url requires a configured Site).
    pub site: Option<String>,
    /// True when the upstream content-type is JSON (else HTML).
    pub is_json: bool,
    /// The decoded (de-gzipped) response body.
    pub body_string: String,
    /// The test headers, as the simulated client request headers.
    pub request_headers: HeaderMap,
    /// Cookies parsed from the test headers.
    pub request_cookies: HashMap<String, String>,
    /// The request path (no query) — the eval context's `request_path`.
    pub request_path: String,
}

/// Why a `/__rre/eval-url` upstream fetch could not be turned into an evaluable
/// body. Each maps to a client-safe JSON error envelope `{ error: { code, message } }`
/// so the frontend surfaces the message verbatim (status 400, never 422 — a 422
/// is treated as a field-validation error by the UI and would hide the message).
pub(crate) enum EvalFetchError {
    /// The URL was malformed / non-http(s) / hostless.
    BadUrl(String),
    /// The URL host matched no configured Site (SSRF guard: configured sites only).
    NoSite(String),
    /// The upstream responded with a content-type that is neither HTML nor JSON.
    NonRenderable(String),
    /// The upstream fetch itself failed (timeout / unavailable / too large).
    Upstream(ProxyError),
}

impl IntoResponse for EvalFetchError {
    fn into_response(self) -> Response {
        let (code, message) = match self {
            EvalFetchError::BadUrl(m) => ("BAD_URL", m),
            EvalFetchError::NoSite(host) => (
                "NO_SITE",
                format!(
                    "No Site is configured for host \"{host}\". Add a Site whose source host matches this URL, then run the test again."
                ),
            ),
            EvalFetchError::NonRenderable(ct) => (
                "NON_RENDERABLE",
                format!(
                    "The upstream responded with content-type \"{ct}\" — only HTML or JSON responses can be tested."
                ),
            ),
            // Reuse the typed upstream error response (502/504 + its own envelope).
            EvalFetchError::Upstream(e) => return e.into_response(),
        };
        (
            StatusCode::BAD_REQUEST,
            axum::Json(serde_json::json!({ "error": { "code": code, "message": message } })),
        )
            .into_response()
    }
}

/// Fetch a real upstream response for the test panel. Resolves the Site by the URL
/// host (REQUIRED — no `upstream_base_url` fallback, so the proxy never fetches an
/// arbitrary host), builds a synthetic GET carrying the test headers + path, and
/// runs it through [`send_upstream`] (which applies the Site's configured headers
/// AFTER the test headers, so a site default always WINS on a name collision).
/// Returns the decoded body + the request facts the evaluator needs.
pub(crate) async fn fetch_for_eval(
    state: &AppState,
    host_header: &str,
    is_https: bool,
    path: &str,
    path_and_query: &str,
    test_headers: HeaderMap,
) -> Result<EvalFetch, EvalFetchError> {
    // 1. Resolve the Site — configured sites only (no fallback). The fetch target
    //    is the Site's `dest`, never the user-supplied host (SSRF-safe).
    let index = state.site_map.index().await;
    let Some(site) = index.lookup(host_header, is_https) else {
        return Err(EvalFetchError::NoSite(host_header.to_string()));
    };
    let route = Route {
        base_url: site.dest_base_url(),
        host: Some(site.dest_host.clone()),
        site: Some(site.slug.clone()),
        headers: site.headers.clone(),
    };

    // 2. Synthetic GET carrying the test headers + the URL path/query. `send_upstream`
    //    copies these as client headers (minus hop-by-hop + Host), sets the dest Host,
    //    THEN applies the Site headers (site wins over a test header on collision).
    let cookies = parse_cookies(&test_headers);
    let mut req = axum::extract::Request::new(Body::empty());
    *req.method_mut() = axum::http::Method::GET;
    *req.uri_mut() = match path_and_query.parse::<axum::http::Uri>() {
        Ok(u) => u,
        Err(_) => {
            return Err(EvalFetchError::BadUrl(
                "The URL path is invalid.".to_string(),
            ))
        }
    };
    *req.headers_mut() = test_headers.clone();

    // 3. Fetch through the shared upstream path.
    let upstream = send_upstream(state, &route, req)
        .await
        .map_err(EvalFetchError::Upstream)?;

    let is_html = upstream.is_html();
    let is_json = upstream.is_json();
    if !is_html && !is_json {
        return Err(EvalFetchError::NonRenderable(
            upstream.content_type.unwrap_or_default(),
        ));
    }

    let UpstreamResponse {
        content_encoding,
        body,
        ..
    } = upstream;
    let body_string = match encoding::decode_for_modify(
        content_encoding.as_deref(),
        body,
        state.settings.max_decompressed_bytes,
    ) {
        Ok(s) => s,
        Err(_) => {
            return Err(EvalFetchError::NonRenderable(format!(
                "content-encoding {}",
                content_encoding.as_deref().unwrap_or("unknown")
            )));
        }
    };

    Ok(EvalFetch {
        site: route.site,
        is_json,
        body_string,
        request_headers: test_headers,
        request_cookies: cookies,
        request_path: path.to_string(),
    })
}
