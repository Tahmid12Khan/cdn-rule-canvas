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
use crate::domain::classifier;
use crate::domain::context::EvaluationContextParts;
use crate::domain::evaluator::{GraphEvaluator, MatchedAction};
use crate::domain::features_matched::{self, FeatureEntry, NodeTiming};
use crate::domain::graph::{Canvas, CanvasGraph, Node};
use crate::error::ProxyError;
use crate::infra::backend_client::{ActiveVersionRead, Env};
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

/// axum fallback handler. Returns a fully-built response.
pub async fn forward(State(state): State<AppState>, req: axum::extract::Request) -> Response {
    let e2e_start = Instant::now();
    let path = req.uri().path().to_string();
    let host = host(&req).unwrap_or_default();

    // 1. Resolve EVERY matching feature (map order, de-duplicated). None -> pass-through.
    //    A request can fan out to several features; we apply each in turn,
    //    chaining the body so one feature's output feeds the next.
    let feature_ids = state.feature_map.resolve_all(&host, &path);
    if feature_ids.is_empty() {
        return passthrough(&state, req).await;
    }
    for feature_id in &feature_ids {
        metrics::counter!("proxy_requests_total", "feature" => feature_id.clone()).increment(1);
    }

    // 2. Classify (canvas isolation source) — same canvas class for every feature.
    let canvas_class = classifier::classify(req.headers());
    let canvas_label = canvas_name(canvas_class);

    // Snapshot request context before consuming the request for upstream fetch.
    let headers = req.headers().clone();
    let cookies = parse_cookies(&headers);

    // 3. Upstream fetch (once). On error -> typed ProxyError response.
    let upstream = match send_upstream(&state, req).await {
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
    let body_string = match encoding::decode_for_modify(content_encoding.as_deref(), body.clone()) {
        Ok(s) => s,
        Err(_) => {
            tracing::warn!(encoding = ?content_encoding, "encoding=unsupported");
            return rebuild_response(status, resp_headers, body, "skipped");
        }
    };

    // 5. Apply each matching feature in order, chaining the running body. The
    //    overall apply_status is "ok" if ANY feature changed the body, else
    //    "skipped" (every feature self-gates: a non-matching feature is a no-op).
    //    `matched` carries each MATCHED feature's (feature_id, features_matched
    //    entry) for header + body injection (spec §1/§2).
    let ApplyResult {
        body: final_body,
        any_applied,
        matched,
    } = if is_html {
        apply_features_html(
            &state,
            &feature_ids,
            canvas_class,
            canvas_label,
            &headers,
            &path,
            &cookies,
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
                    canvas_class,
                    canvas_label,
                    &headers,
                    &path,
                    &cookies,
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
    //    feature (spec §1).
    let out_bytes = encoding::reencode(content_encoding.as_deref(), final_body);
    strip_hop_by_hop(&mut resp_headers);
    set_content_length(&mut resp_headers, out_bytes.len());
    let mut res = rebuild_response(status, resp_headers, out_bytes, apply_status);
    for (feature_id, _) in &matched {
        stamp(&mut res, &format!("x-rre-feature-{feature_id}"), "true");
    }
    res
}

/// The result of folding every matching feature over a response body: the final
/// (chained) body, whether ANY feature changed it, and each MATCHED feature's
/// `features_matched` entry (spec §2) keyed by feature_id, in resolution order.
struct ApplyResult {
    body: String,
    any_applied: bool,
    matched: Vec<(String, FeatureEntry)>,
}

/// Apply every matching feature's actions to an HTML body, in order, chaining
/// the result. Each feature: active-version lookup (None -> skip), html_selector
/// applicability gate (no match -> skip), eval the classified canvas, fold its
/// matched actions. Returns the final body and whether ANY feature changed it.
#[allow(clippy::too_many_arguments)]
async fn apply_features_html(
    state: &AppState,
    feature_ids: &[String],
    canvas_class: Canvas,
    canvas_label: &str,
    headers: &HeaderMap,
    path: &str,
    cookies: &HashMap<String, String>,
    mut current: String,
) -> ApplyResult {
    let mut any_applied = false;
    let mut matched: Vec<(String, FeatureEntry)> = Vec::new();
    for feature_id in feature_ids {
        let Some(av) = state.backend.active_version(feature_id, Env::Live).await else {
            continue;
        };
        if !html_selector_matches(&av, &current) {
            tracing::info!(
                feature_id = %feature_id, canvas = canvas_label,
                apply_status = "skipped", reason = "html_selector_no_match", "request"
            );
            continue;
        }
        let ctx =
            EvaluationContextParts::from_request(headers, path, cookies, current.clone(), false);
        let (actions, eval_ms) = evaluate(state, &av, ctx, feature_id, canvas_class).await;
        if actions.is_empty() {
            log_skipped(feature_id, canvas_label, eval_ms);
            continue;
        }
        let canvas = av.canvas(canvas_class);
        let mut applied = false;
        // Per-node timing (spec §4): time each expression node's apply.
        let mut timings: Vec<NodeTiming> = Vec::with_capacity(actions.len());
        for ma in &actions {
            let t_node = Instant::now();
            let (next, changed) =
                json_apply::apply_action_html(current, &ma.action, &av.outcomes, &state.sanitizer);
            let time_ms = t_node.elapsed().as_secs_f64() * 1000.0;
            current = next;
            applied |= changed;
            timings.push(NodeTiming {
                node_id: ma.node_id.clone(),
                label: expression_label(canvas, &ma.node_id),
                time_ms,
            });
        }
        let transform_ms: f64 = timings.iter().map(|t| t.time_ms).sum();
        metrics::histogram!("proxy_transform_ms").record(transform_ms);
        let status = if applied { "ok" } else { "skipped" };
        tracing::info!(
            feature_id = %feature_id, canvas = canvas_label,
            actions = actions.len(), eval_ms, transform_ms, apply_status = status, "request"
        );
        any_applied |= applied;
        // The feature MATCHED (>=1 expression action). Build its entry (spec §2).
        if let Some(entry) = features_matched::build_entry(&timings, eval_ms) {
            matched.push((feature_id.clone(), entry));
        }
    }
    // Inject `window.rre.features_matched` as the FINAL step — after all sanitized
    // component transforms (spec §2 sanitizer bypass) — only when >=1 matched.
    if !matched.is_empty() {
        current = inject_html_features_matched(current, &matched);
    }
    ApplyResult {
        body: current,
        any_applied,
        matched,
    }
}

/// Append the trusted `window.rre.features_matched` script immediately before
/// `</body>` (or at the end of the document if there is none). The serialized
/// map has every `<` escaped to `<` so an embedded `</script>` cannot break
/// out — XSS-safe; this is first-party trusted data (spec §2).
fn inject_html_features_matched(body: String, matched: &[(String, FeatureEntry)]) -> String {
    let map: serde_json::Map<String, serde_json::Value> = matched
        .iter()
        .map(|(id, entry)| (id.clone(), serde_json::to_value(entry).unwrap_or_default()))
        .collect();
    let json = serde_json::to_string(&serde_json::Value::Object(map))
        .unwrap_or_else(|_| "{}".to_string())
        .replace('<', "\\u003c");
    let script =
        format!("<script>window.rre=window.rre||{{}};window.rre.features_matched={json};</script>");
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
    canvas_class: Canvas,
    canvas_label: &str,
    headers: &HeaderMap,
    path: &str,
    cookies: &HashMap<String, String>,
    mut current: serde_json::Value,
    original: &str,
) -> ApplyResult {
    let mut any_applied = false;
    let mut matched: Vec<(String, FeatureEntry)> = Vec::new();
    for feature_id in feature_ids {
        let Some(av) = state.backend.active_version(feature_id, Env::Live).await else {
            continue;
        };
        if !json_selector_matches(&av, &current) {
            tracing::info!(
                feature_id = %feature_id, canvas = canvas_label,
                apply_status = "skipped", reason = "json_selector_no_match", "request"
            );
            continue;
        }
        // Eval reads response_json from the body string, so feed it the CURRENT
        // (already-chained) body — not the original — for correct chaining.
        let ctx_body = serde_json::to_string(&current).unwrap_or_default();
        let ctx = EvaluationContextParts::from_request(headers, path, cookies, ctx_body, true);
        let (actions, eval_ms) = evaluate(state, &av, ctx, feature_id, canvas_class).await;
        if actions.is_empty() {
            log_skipped(feature_id, canvas_label, eval_ms);
            continue;
        }
        let canvas = av.canvas(canvas_class);
        let mut applied = false;
        // Per-node timing (spec §4): time each expression node's apply.
        let mut timings: Vec<NodeTiming> = Vec::with_capacity(actions.len());
        for ma in &actions {
            let t_node = Instant::now();
            let changed = json_apply::apply_action_json(&mut current, &ma.action, &av.outcomes);
            let time_ms = t_node.elapsed().as_secs_f64() * 1000.0;
            applied |= changed;
            timings.push(NodeTiming {
                node_id: ma.node_id.clone(),
                label: expression_label(canvas, &ma.node_id),
                time_ms,
            });
        }
        let transform_ms: f64 = timings.iter().map(|t| t.time_ms).sum();
        metrics::histogram!("proxy_transform_ms").record(transform_ms);
        let status = if applied { "ok" } else { "skipped" };
        tracing::info!(
            feature_id = %feature_id, canvas = canvas_label,
            actions = actions.len(), eval_ms, transform_ms, apply_status = status, "request"
        );
        any_applied |= applied;
        // The feature MATCHED (>=1 expression action). Build its entry (spec §2).
        if let Some(entry) = features_matched::build_entry(&timings, eval_ms) {
            matched.push((feature_id.clone(), entry));
        }
    }
    // Inject `body.rre.features_matched` (spec §2): create `rre` if absent, merge
    // `features_matched` without clobbering other `rre.*` keys. Only when matched.
    if !matched.is_empty() {
        inject_json_features_matched(&mut current, &matched);
    }
    // Serialize the final chained body. A serialize error (≈never for a Value)
    // falls back to the original untouched body.
    match serde_json::to_string(&current) {
        Ok(s) => ApplyResult {
            body: s,
            any_applied,
            matched,
        },
        Err(e) => {
            metrics::counter!("proxy_apply_errors_total").increment(1);
            tracing::warn!(error = %e, "json serialize failed, serving original");
            ApplyResult {
                body: original.to_string(),
                any_applied: false,
                matched: Vec::new(),
            }
        }
    }
}

/// Merge the matched features' entries into `body["rre"]["features_matched"]`
/// (spec §2). Creates the `rre` object if absent; sets `features_matched`
/// without clobbering other `rre.*` keys. A non-object `body` is left untouched.
fn inject_json_features_matched(body: &mut serde_json::Value, matched: &[(String, FeatureEntry)]) {
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
    let fm: serde_json::Map<String, serde_json::Value> = matched
        .iter()
        .map(|(id, entry)| (id.clone(), serde_json::to_value(entry).unwrap_or_default()))
        .collect();
    rre_obj.insert(
        "features_matched".to_string(),
        serde_json::Value::Object(fm),
    );
}

/// Run the classified canvas through the evaluator, returning the ordered matched
/// expression actions and the eval duration in ms. Records `proxy_eval_ms`.
async fn evaluate(
    state: &AppState,
    av: &ActiveVersionRead,
    ctx: EvaluationContextParts,
    feature_id: &str,
    canvas_class: Canvas,
) -> (Vec<MatchedAction>, f64) {
    let canvas_graph = av.canvas(canvas_class);
    let eval_start = Instant::now();
    let evaluator = GraphEvaluator::new(state.registry.clone(), &state.compiled);
    let actions = evaluator
        .evaluate(
            canvas_graph,
            ctx,
            feature_id,
            av.version_number,
            canvas_class,
        )
        .await;
    let eval_ms = eval_start.elapsed().as_secs_f64() * 1000.0;
    metrics::histogram!("proxy_eval_ms").record(eval_ms);
    (actions, eval_ms)
}

/// Structured "no modification" log shared by both content kinds (no matched
/// expression actions on the routed path).
fn log_skipped(feature_id: &str, canvas_label: &str, eval_ms: f64) {
    tracing::info!(
        feature_id = %feature_id,
        canvas = canvas_label,
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
    let Some(selector) = av
        .applicability
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
    let doc = scraper::Html::parse_document(body);
    doc.select(&sel).next().is_some()
}

/// JSON applicability gate. True when `json_selector` is absent/empty (apply
/// always) OR it parses and matches >=1 node. A malformed selector fails open
/// (apply). Never panics.
fn json_selector_matches(av: &ActiveVersionRead, body: &serde_json::Value) -> bool {
    let Some(selector) = av
        .applicability
        .json_selector
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
    else {
        return true; // no gate configured -> apply.
    };
    match serde_json_path::JsonPath::parse(selector) {
        Ok(path) => !path.query(body).all().is_empty(),
        Err(_) => {
            tracing::warn!(selector, "json_selector parse failed, applying (fail-open)");
            true
        }
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

/// Fetch the upstream response and capture it fully (HTML buffered in memory).
pub(crate) async fn send_upstream(
    state: &AppState,
    req: axum::extract::Request,
) -> Result<UpstreamResponse, ProxyError> {
    let (parts, body) = req.into_parts();

    let body_bytes = axum::body::to_bytes(body, 1024 * 1024)
        .await
        .map_err(|_| ProxyError::UpstreamProtocol)?;

    let url = format!(
        "{}{}",
        state.settings.upstream_base_url.trim_end_matches('/'),
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
    if !body_bytes.is_empty() {
        builder = builder.body(body_bytes.to_vec());
    }

    let resp = builder.send().await.map_err(map_reqwest_error)?;

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

    let body = resp.bytes().await.map_err(map_reqwest_error)?;

    Ok(UpstreamResponse {
        status,
        resp_headers,
        content_encoding,
        content_type,
        body,
    })
}

/// Forward a request upstream and return the untouched response (skipped).
async fn passthrough(state: &AppState, req: axum::extract::Request) -> Response {
    match send_upstream(state, req).await {
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

/// Resolve the request host (Host header or URI authority), lowercased.
fn host(req: &axum::extract::Request) -> Option<String> {
    req.headers()
        .get(axum::http::header::HOST)
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string())
        .or_else(|| req.uri().authority().map(|a| a.as_str().to_string()))
}

/// Parse all request cookies into a map.
fn parse_cookies(headers: &HeaderMap) -> HashMap<String, String> {
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

fn canvas_name(c: Canvas) -> &'static str {
    match c {
        Canvas::Anonymous => "anonymous",
        Canvas::Registered => "registered",
        Canvas::Customer => "customer",
    }
}

/// Display label for an expression node (spec §4): the manifest label for the
/// action kind, with the raw kind string as the fieldless fallback. The proxy
/// has no manifest at runtime, so the action kind IS the label (mirrors the
/// `eval.rs` `label()` helper — a node-id->label lookup over the canvas).
fn expression_label(canvas: &CanvasGraph, node_id: &str) -> String {
    match canvas.nodes.iter().find(|n| n.id() == node_id) {
        Some(Node::Expression { action, .. }) => action.kind.clone(),
        _ => node_id.to_string(),
    }
}
