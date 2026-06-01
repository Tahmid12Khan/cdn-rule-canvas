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

use crate::domain::applier::orchestrator;
use crate::domain::classifier;
use crate::domain::context::EvaluationContextParts;
use crate::domain::evaluator::GraphEvaluator;
use crate::domain::graph::Canvas;
use crate::error::ProxyError;
use crate::infra::backend_client::Env;
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

    // 1. Resolve feature. Miss -> pass-through.
    let Some(feature_id) = state.feature_map.resolve(&host, &path) else {
        return passthrough(&state, req).await;
    };

    metrics::counter!("proxy_requests_total", "feature" => feature_id.clone()).increment(1);

    // 2. Classify (canvas isolation source).
    let canvas_class = classifier::classify(req.headers());

    // 3. Active version. Fail-open on None.
    let Some(av) = state.backend.active_version(&feature_id, Env::Live).await else {
        return passthrough(&state, req).await;
    };

    // Snapshot request context before consuming the request for upstream fetch.
    let headers = req.headers().clone();
    let cookies = parse_cookies(&headers);

    // 4. Upstream fetch. On error -> typed ProxyError response.
    let upstream = match send_upstream(&state, req).await {
        Ok(u) => u,
        Err(e) => return e.into_response(),
    };

    // 5. HTML gate.
    if !upstream.is_html() {
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

    // 6/7. Build eval context + evaluate the classified canvas ONLY.
    let ctx = EvaluationContextParts::from_request(&headers, &path, &cookies, body_string.clone());
    let canvas_graph = av.canvas(canvas_class);

    let eval_start = Instant::now();
    let evaluator = GraphEvaluator::new(state.registry.clone(), &state.compiled);
    let outcome_id = evaluator
        .evaluate(
            canvas_graph,
            ctx,
            &feature_id,
            av.version_number,
            canvas_class,
        )
        .await;
    let eval_ms = eval_start.elapsed().as_secs_f64() * 1000.0;
    metrics::histogram!("proxy_eval_ms").record(eval_ms);

    // 8. Resolve outcome.
    let outcome = outcome_id.and_then(|id| av.find_outcome(id));
    let canvas_label = canvas_name(canvas_class);

    let (final_html, apply_status, _transform_ms) = match outcome {
        None => {
            tracing::info!(
                feature_id = %feature_id, canvas = canvas_label, outcome_id = "none",
                eval_ms, apply_status = "skipped", "request"
            );
            (body_string, "skipped", 0.0)
        }
        Some(o) if o.is_builtin_show_content() => {
            tracing::info!(
                feature_id = %feature_id, canvas = canvas_label, outcome_id = %o.id,
                eval_ms, apply_status = "skipped", "request"
            );
            (body_string, "skipped", 0.0)
        }
        Some(o) => {
            metrics::counter!("proxy_outcomes_total", "outcome_id" => o.id.to_string())
                .increment(1);
            let t_start = Instant::now();
            let result = orchestrator::apply_outcome(body_string.clone(), o, &state.sanitizer);
            let transform_ms = t_start.elapsed().as_secs_f64() * 1000.0;
            metrics::histogram!("proxy_transform_ms").record(transform_ms);
            match result {
                Ok(m) => {
                    let status = if m.applied { "ok" } else { "skipped" };
                    tracing::info!(
                        feature_id = %feature_id, canvas = canvas_label, outcome_id = %o.id,
                        eval_ms, transform_ms, apply_status = status, "request"
                    );
                    (m.html, status, transform_ms)
                }
                Err(e) => {
                    metrics::counter!("proxy_apply_errors_total").increment(1);
                    tracing::warn!(error = %e, "apply error, serving original");
                    (body_string, "error", transform_ms)
                }
            }
        }
    };

    metrics::histogram!("proxy_e2e_ms").record(e2e_start.elapsed().as_secs_f64() * 1000.0);

    // 9. Re-encode + rebuild.
    let out_bytes = encoding::reencode(content_encoding.as_deref(), final_html);
    strip_hop_by_hop(&mut resp_headers);
    set_content_length(&mut resp_headers, out_bytes.len());
    rebuild_response(status, resp_headers, out_bytes, apply_status)
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
