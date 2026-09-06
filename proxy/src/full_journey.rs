//! `POST /__rre/eval-full-journey` — run the matching features of a real URL's
//! response content-type, in execution order, CHAINING the body feature→feature
//! like the production forwarder (`apply_features_html`/`apply_features_json`): the
//! per-feature loop (version resolve → selector gate against the CURRENT body →
//! ctx from the CURRENT body → eval → fold actions → chain) mirrors production.
//!
//! ONE deliberate divergence: feature SELECTION. Production runs ALL features and
//! lets each self-gate (a json_expression on an HTML body just returns No); this
//! endpoint instead PRE-FILTERS the feature list by feature `type` so only the
//! features that match the response content kind run (spec items 8/9 — HTML and
//! JSON rules are separate, per-type-ordered lists). On a JSON parse failure it
//! also mirrors production "nothing runs" (returns `features: []`, body untouched).
//!
//! Unlike `/__rre/eval-url` (which runs ONE editor canvas), this fetches the real
//! upstream once, classifies the request, then folds each saved feature's matched
//! expression actions over the running body — capturing each feature's per-node
//! Transformation Journey so the frontend can diff at three levels: per-node,
//! per-feature start↔end, and first-feature-start↔last-feature-end.
//!
//! It is an off-hot-path test tool; correctness mirrors production, latency is not
//! a hot-path concern. CORS + JSON setup match `/__rre/eval-url` (see `lib.rs`).

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;

use axum::extract::State;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::Json;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::domain::context::EvaluationContextParts;
use crate::domain::evaluator::GraphEvaluator;
use crate::domain::features_matched::{self, FeatureEntry};
use crate::domain::graph::{CanvasGraph, RuleGraph};
use crate::domain::translator::to_decision_content;
use crate::eval::{build_journey, JourneyEntry, JourneyInputs, JourneyResult};
use crate::forwarder::{self, selector_matches};
use crate::infra::backend_client::{ActiveOutcome, Applicability, Env, FeatureListItem};
use crate::state::AppState;

// ---------------------------------------------------------------------------
// Request / Response types (LOCKED WIRE CONTRACT — names/shapes are frozen)
// ---------------------------------------------------------------------------

/// `POST /__rre/eval-full-journey` request body.
#[derive(Deserialize)]
pub struct FullJourneyRequest {
    pub url: String,
    #[serde(default)]
    pub headers: HashMap<String, String>,
    /// `"live"` (default) or `"staging"`: which environment's active version each
    /// feature resolves to (unless overridden per-feature below).
    #[serde(default)]
    pub env: Option<String>,
    /// `{ "<feature_id>": <version_number> }`: pin a feature to a specific saved
    /// version (fetched via `GET /features/{fid}/versions/{vnum}`) instead of its
    /// active version. A missing/unresolvable override fails open (the feature is
    /// skipped, the running body is untouched).
    #[serde(default)]
    pub version_overrides: HashMap<String, i32>,
}

/// `POST /__rre/eval-full-journey` response body.
#[derive(Serialize)]
pub struct FullJourneyResponse {
    /// `"html"` or `"json"` — the fetched response's content kind.
    pub content_kind: String,
    /// The matched Site's slug (always `Some` — a configured Site is required).
    pub site: Option<String>,
    /// EXACT wall-clock around the WHOLE multi-feature loop, `d.dd`.
    pub total_time_ms: String,
    /// Every feature of the matching content type, in execution order.
    pub features: Vec<FullJourneyFeature>,
}

/// One feature's full-journey result.
#[derive(Serialize)]
pub struct FullJourneyFeature {
    pub feature_id: String,
    pub name: String,
    /// `"html"` or `"json"`.
    pub r#type: String,
    pub execution_order: i32,
    /// The resolved version number; `null` when unresolved/skipped.
    pub version_number: Option<i32>,
    /// Selector gate passed AND the feature evaluated.
    pub matched: bool,
    /// SAME shape as `EvalResponse.journey` (index, node_id, kind, label, branch,
    /// body_after, time_ms). `[]` when the feature was skipped.
    pub journey: Vec<JourneyEntry>,
    /// SAME as `EvalResponse.summary`; `null` when skipped or no expression matched.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub summary: Option<FeatureEntry>,
    /// `eval_ms + Σ node times`, `d.dd` (consistent with a single feature's entry).
    pub time_took_ms: String,
}

// ---------------------------------------------------------------------------
// Handler
// ---------------------------------------------------------------------------

pub async fn full_journey_handler(
    State(state): State<AppState>,
    Json(req): Json<FullJourneyRequest>,
) -> impl IntoResponse {
    // 1. Parse + validate the URL exactly like `eval_url_handler` (absolute
    //    http/https + host). The url host selects a configured Site (SSRF-safe).
    let parsed = match reqwest::Url::parse(req.url.trim()) {
        Ok(u) => u,
        Err(_) => {
            return forwarder::EvalFetchError::BadUrl(
                "Enter a valid absolute URL, e.g. https://www.example.com/article/123.".to_string(),
            )
            .into_response();
        }
    };
    if !matches!(parsed.scheme(), "http" | "https") {
        return forwarder::EvalFetchError::BadUrl(
            "The URL scheme must be http or https.".to_string(),
        )
        .into_response();
    }
    let Some(host) = parsed.host_str() else {
        return forwarder::EvalFetchError::BadUrl("The URL must include a host.".to_string())
            .into_response();
    };
    let host_header = match parsed.port() {
        Some(p) => format!("{host}:{p}"),
        None => host.to_string(),
    };
    let is_https = parsed.scheme().eq_ignore_ascii_case("https");
    let path = parsed.path().to_string();
    let mut path_and_query = path.clone();
    if let Some(q) = parsed.query() {
        path_and_query.push('?');
        path_and_query.push_str(q);
    }

    let test_headers = forwarder::headers_to_map(&req.headers);

    // 2. Fetch the real upstream THROUGH the proxy (Site headers applied, SSRF-safe).
    let fetched = match forwarder::fetch_for_eval(
        &state,
        &host_header,
        is_https,
        &path,
        &path_and_query,
        test_headers,
    )
    .await
    {
        Ok(f) => f,
        Err(e) => return e.into_response(),
    };

    let env = match req.env.as_deref() {
        Some(s) if s.eq_ignore_ascii_case("staging") => Env::Staging,
        _ => Env::Live,
    };

    let resp = run_full_journey(&state, &fetched, env, &req.version_overrides).await;
    (
        StatusCode::OK,
        Json(serde_json::to_value(resp).unwrap_or_default()),
    )
        .into_response()
}

/// The resolved rule_graph + applicability + version_number for one feature, from
/// either a `version_overrides` lookup or the active version. Carrying owned data
/// lets the override (`VersionRead`) and active-version (`Arc<ActiveVersionRead>`)
/// sources share the same downstream code, and lets `apply_outcome` resolve
/// against the feature's outcomes.
struct ResolvedVersion {
    version_number: i32,
    rule_graph: RuleGraph,
    applicability: Applicability,
    outcomes: Vec<ActiveOutcome>,
}

/// Run the multi-feature chained journey over the fetched body. Mirrors the
/// production `apply_features_html`/`apply_features_json` semantics: per feature
/// resolve version → selector gate against the CURRENT body → build ctx from the
/// CURRENT body → eval with trace → fold the matched actions, chaining `current`.
async fn run_full_journey(
    state: &AppState,
    fetched: &forwarder::EvalFetch,
    env: Env,
    version_overrides: &HashMap<String, i32>,
) -> FullJourneyResponse {
    let is_json = fetched.is_json;
    let content_kind = if is_json { "json" } else { "html" };

    // Identity resolved ONCE from the real fetched request's cookies/headers
    // (mirrors how `site` is resolved once and threaded through every feature's
    // context below).
    let identity_settings = crate::domain::identity::IdentitySettings {
        user_cookie: state.settings.identity_user_cookie.clone(),
        products_cookie: state.settings.identity_products_cookie.clone(),
        user_header: state.settings.identity_user_header.clone(),
        products_header: state.settings.identity_products_header.clone(),
    };
    let identity = crate::domain::identity::resolve(
        &fetched.request_headers,
        &fetched.request_cookies,
        &identity_settings,
    );

    // 4a. Parse the JSON body ONCE up front. On a parse failure mirror production
    //     (forwarder.rs serves the original untouched and runs ZERO features) —
    //     short-circuit with an empty feature list instead of iterating against
    //     Null, which would let every JSON feature run against a body that never
    //     existed. `total_time_ms` is "0.00" because no feature loop ran.
    let mut current_json: Value = Value::Null;
    if is_json {
        match serde_json::from_str(&fetched.body_string) {
            Ok(v) => current_json = v,
            Err(_) => {
                tracing::warn!("full_journey: json parse failed, running no features");
                return FullJourneyResponse {
                    content_kind: content_kind.to_string(),
                    site: fetched.site.clone(),
                    total_time_ms: features_matched::fmt_ms(0.0),
                    features: Vec::new(),
                };
            }
        }
    }

    // 3. Ordered feature list, KEEP ONLY the matching content type. Preserve the
    //    backend order (execution_order asc within type).
    let features: Vec<FeatureListItem> = state
        .backend
        .feature_list()
        .await
        .into_iter()
        .filter(|f| f.kind.eq_ignore_ascii_case(content_kind))
        .collect();

    // 4b. Running body starts from the fetched body; start the total-time clock.
    let mut current_html: String = if is_json {
        String::new()
    } else {
        fetched.body_string.clone()
    };

    let total_start = Instant::now();
    let mut out_features: Vec<FullJourneyFeature> = Vec::with_capacity(features.len());

    for feat in &features {
        // 5. Resolve the rule_graph + applicability + version_number.
        let resolved = resolve_version(state, &feat.id, env, version_overrides).await;
        let Some(resolved) = resolved else {
            // Unresolved → record a skipped entry; do NOT touch the running body.
            out_features.push(skipped_feature(feat, None));
            continue;
        };

        // 6. Selector gate against the CURRENT running body (shared with production).
        let gate_ok = if is_json {
            selector_matches(&resolved.applicability, true, Some(&current_json), "")
        } else {
            selector_matches(&resolved.applicability, false, None, &current_html)
        };
        if !gate_ok {
            tracing::info!(
                feature_id = %feat.id, apply_status = "skipped",
                reason = "selector_no_match", "full_journey"
            );
            out_features.push(skipped_feature(feat, Some(resolved.version_number)));
            continue;
        }

        // 7. Build ctx from the CURRENT body (JSON re-serialized so later features'
        //    json_expression sees mutations), translate + eval + build journey,
        //    then chain `current` to the journey's final body.
        let canvas = resolved.rule_graph.canvas.clone();
        let ctx_body = if is_json {
            serde_json::to_string(&current_json).unwrap_or_default()
        } else {
            current_html.clone()
        };
        let ctx = EvaluationContextParts::from_request(
            &fetched.request_headers,
            &fetched.request_path,
            &fetched.request_cookies,
            ctx_body,
            is_json,
            identity.clone(),
        )
        .with_site(fetched.site.clone())
        .into_context();

        let inputs = JourneyInputs {
            is_json,
            json_body: current_json.clone(),
            html_body: current_html.clone(),
        };

        match eval_feature(
            state,
            &canvas,
            ctx,
            &inputs,
            &resolved.outcomes,
            resolved.version_number,
        )
        .await
        {
            Some((journey, summary, time_took_ms, final_body)) => {
                // CHAIN: set the running body to the journey's final body.
                if is_json {
                    current_json = final_body;
                } else {
                    current_html = final_body.as_str().map(str::to_string).unwrap_or_default();
                }
                out_features.push(FullJourneyFeature {
                    feature_id: feat.id.clone(),
                    name: feat.name.clone(),
                    r#type: feat.kind.clone(),
                    execution_order: feat.execution_order,
                    version_number: Some(resolved.version_number),
                    matched: true,
                    journey,
                    summary,
                    time_took_ms,
                });
            }
            None => {
                // Translation/eval failure: fail open (skip, body untouched).
                out_features.push(skipped_feature(feat, Some(resolved.version_number)));
            }
        }
    }

    // 8. total_time_ms: the EXACT wall-clock around the whole loop.
    let total_time_ms = features_matched::fmt_ms(total_start.elapsed().as_secs_f64() * 1000.0);

    FullJourneyResponse {
        content_kind: content_kind.to_string(),
        site: fetched.site.clone(),
        total_time_ms,
        features: out_features,
    }
}

/// Resolve a feature's version: a `version_overrides` entry → `version(fid, vnum)`
/// (no outcomes available for an arbitrary saved version, so `apply_outcome` is a
/// no-op there); else the active version (carries outcomes). `None` on any
/// unresolved/error path (fail-open).
async fn resolve_version(
    state: &AppState,
    feature_id: &str,
    env: Env,
    version_overrides: &HashMap<String, i32>,
) -> Option<ResolvedVersion> {
    if let Some(&vnum) = version_overrides.get(feature_id) {
        let v = state.backend.version(feature_id, vnum).await?;
        return Some(ResolvedVersion {
            version_number: v.version_number,
            rule_graph: v.rule_graph,
            applicability: v.applicability,
            outcomes: Vec::new(),
        });
    }
    let av = state.backend.active_version(feature_id, env).await?;
    Some(ResolvedVersion {
        version_number: av.version_number,
        rule_graph: av.rule_graph.clone(),
        applicability: av.applicability.clone(),
        outcomes: av.outcomes.clone(),
    })
}

/// Translate + evaluate one feature's canvas, building its journey + summary +
/// time + final body. Returns `None` on translation/eval failure (fail-open).
async fn eval_feature(
    state: &AppState,
    canvas: &CanvasGraph,
    ctx: crate::domain::context::EvaluationContext,
    inputs: &JourneyInputs,
    outcomes: &[ActiveOutcome],
    version_number: i32,
) -> Option<(Vec<JourneyEntry>, Option<FeatureEntry>, String, Value)> {
    // Translate (catch_unwind, mirroring eval.rs).
    let content =
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| to_decision_content(canvas)))
            .ok()?;

    let evaluator = GraphEvaluator::new(state.registry.clone(), &state.compiled);
    let eval_start = Instant::now();
    let trace = evaluator
        .evaluate_with_trace(canvas, content, Arc::new(ctx))
        .await
        .ok()?;
    let eval_ms = eval_start.elapsed().as_secs_f64() * 1000.0;

    // Pre-resolve component references on the async side so the chained
    // `body_after` reflects the rendered component (design §4.3): apply_component*
    // action refs AND component_ref* components inside the feature's outcomes.
    let components = forwarder::resolve_action_components(state, &trace.actions, outcomes).await;

    let JourneyResult {
        journey,
        timings,
        final_body,
    } = build_journey(state, canvas, inputs, &trace, outcomes, &components);

    // The full-journey path resolves a real saved version, so the summary carries it.
    let summary = features_matched::build_entry(&timings, eval_ms, Some(version_number));
    // time_took_ms = eval_ms + Σ node times — consistent with `build_entry`'s
    // `time_took_ms` (a feature with no expression node still reports its eval_ms).
    let sum_ms: f64 = timings.iter().map(|t| t.time_ms).sum();
    let time_took_ms = features_matched::fmt_ms(eval_ms + sum_ms);

    Some((journey, summary, time_took_ms, final_body))
}

/// A skipped feature entry: no journey, no summary, `time_took_ms = "0.00"`.
fn skipped_feature(feat: &FeatureListItem, version_number: Option<i32>) -> FullJourneyFeature {
    FullJourneyFeature {
        feature_id: feat.id.clone(),
        name: feat.name.clone(),
        r#type: feat.kind.clone(),
        execution_order: feat.execution_order,
        version_number,
        matched: false,
        journey: Vec::new(),
        summary: None,
        time_took_ms: features_matched::fmt_ms(0.0),
    }
}
