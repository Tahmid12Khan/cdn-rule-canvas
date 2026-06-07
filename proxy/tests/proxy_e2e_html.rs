//! Full pipeline e2e: a fake upstream (wiremock) serves paywalled HTML, a fake
//! backend (wiremock) serves the active-version with a paywall outcome that
//! injects HTML. The proxy should evaluate the anonymous canvas and inject.

use std::sync::Arc;

use rre_proxy::build_app;
use rre_proxy::config::Settings;
use rre_proxy::domain::processors::default_registry;
use rre_proxy::infra::backend_client::BackendClient;
use rre_proxy::infra::compiled_cache::CompiledCache;
use rre_proxy::infra::site_map::SiteMap;
use rre_proxy::state::AppState;
use serde_json::json;
use wiremock::matchers::{method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

const FEATURE: &str = "demo-article";
const HOST: &str = "rre.test";
const PAYWALL_OUTCOME: &str = "22222222-2222-2222-2222-222222222222";

/// Mount `GET /api/v1/features?page_size=100` returning the single demo feature so
/// the proxy's per-feature pipeline runs it for every request (no feature_map).
/// `type: "html"` so the content-type filter keeps it on an HTML response.
async fn mount_feature_list(backend: &MockServer) {
    Mock::given(method("GET"))
        .and(path("/api/v1/features"))
        .and(query_param("page_size", "100"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "items": [{ "id": FEATURE, "name": FEATURE, "type": "html", "execution_order": 1 }]
        })))
        .mount(backend)
        .await;
}

/// Mount `GET /api/v1/sites?page_size=100` returning a single Site that routes the
/// inbound `host` (`source_host:source_port`) to the wiremock `upstream` authority,
/// carrying the given per-site `headers` map. The site source uses the http default
/// port 80 so an inbound `Host: rre.test` (no port) resolves to `rre.test:80`.
async fn mount_site_list(backend: &MockServer, upstream_uri: &str, headers: serde_json::Value) {
    // `upstream_uri` is `http://127.0.0.1:<port>` (wiremock). Split into host:port
    // for the Site's dest fields so the proxy forwards to the real mock.
    let authority = upstream_uri.trim_start_matches("http://");
    let (dest_host, dest_port) = authority
        .rsplit_once(':')
        .map(|(h, p)| (h.to_string(), p.parse::<u16>().unwrap()))
        .unwrap();
    Mock::given(method("GET"))
        .and(path("/api/v1/sites"))
        .and(query_param("page_size", "100"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "items": [{
                "slug": "demo-site",
                "source_host": HOST,
                "source_port": 80,
                "dest_protocol": "http",
                "dest_host": dest_host,
                "dest_port": dest_port,
                "headers": headers
            }]
        })))
        .mount(backend)
        .await;
}

/// The editor canvas posted to `/__rre/eval-url` (the anonymous canvas of
/// `active_version_body`): start -> meta(paywall?) ; yes -> apply_outcome -> end.
fn eval_canvas() -> serde_json::Value {
    json!({
        "root_node_id": "start",
        "nodes": [
            { "kind": "start", "id": "start", "position": { "x": -200.0, "y": 0.0 } },
            { "kind": "decision", "id": "n_meta",
              "processor": { "type": "meta_tags", "tag_name": "paywall", "operator": "contains", "value": "true" },
              "position": { "x": 0.0, "y": 0.0 } },
            { "kind": "expression", "id": "n_paywall",
              "action": { "type": "apply_outcome", "outcome_id": PAYWALL_OUTCOME },
              "position": { "x": 200.0, "y": 0.0 } },
            { "kind": "end", "id": "end", "position": { "x": 400.0, "y": 100.0 } }
        ],
        "edges": [
            { "id": "e0", "source_node_id": "start",     "target_node_id": "n_meta",    "branch": "yes" },
            { "id": "e1", "source_node_id": "n_meta",    "target_node_id": "n_paywall", "branch": "yes" },
            { "id": "e2", "source_node_id": "n_meta",    "target_node_id": "end",       "branch": "no" },
            { "id": "e3", "source_node_id": "n_paywall", "target_node_id": "end",       "branch": "yes" }
        ]
    })
}

fn active_version_body() -> serde_json::Value {
    // start -> n_meta (paywall?) ; yes -> apply_outcome(paywall) -> end ; no -> end.
    json!({
        "version_number": 1,
        "rule_graph": {
            "anonymous": {
                "root_node_id": "start",
                "nodes": [
                    { "kind": "start", "id": "start", "position": { "x": -200.0, "y": 0.0 } },
                    { "kind": "decision", "id": "n_meta",
                      "processor": { "type": "meta_tags", "tag_name": "paywall", "operator": "contains", "value": "true" },
                      "position": { "x": 0.0, "y": 0.0 } },
                    { "kind": "expression", "id": "n_paywall",
                      "action": { "type": "apply_outcome", "outcome_id": PAYWALL_OUTCOME },
                      "position": { "x": 200.0, "y": 0.0 } },
                    { "kind": "end", "id": "end", "position": { "x": 400.0, "y": 100.0 } }
                ],
                "edges": [
                    { "id": "e0", "source_node_id": "start",     "target_node_id": "n_meta",    "branch": "yes" },
                    { "id": "e1", "source_node_id": "n_meta",    "target_node_id": "n_paywall", "branch": "yes" },
                    { "id": "e2", "source_node_id": "n_meta",    "target_node_id": "end",       "branch": "no" },
                    { "id": "e3", "source_node_id": "n_paywall", "target_node_id": "end",       "branch": "yes" }
                ]
            },
            "registered": { "root_node_id": null, "nodes": [], "edges": [] },
            "customer":   { "root_node_id": null, "nodes": [], "edges": [] }
        },
        "outcomes": [
            {
                "id": PAYWALL_OUTCOME, "title": "Paywall", "is_builtin": false, "order_index": 0,
                "components": [
                    { "id": "44444444-4444-4444-4444-444444444444", "slug": "wall",
                      "type": "html_injection",
                      "config": { "type": "html_injection", "target_selector": "#article-body",
                                  "placement_mode": "append", "html_body": "<div class=\"rre-wall\">Subscribe to continue</div>" },
                      "placement": "inline", "order_index": 0 }
                ]
            },
            { "id": "33333333-3333-3333-3333-333333333333", "title": "ShowContent",
              "is_builtin": true, "order_index": 1, "components": [] }
        ]
    })
}

async fn spawn(upstream: &str, backend: &str) -> String {
    spawn_with_cap(upstream, backend, 16 * 1024 * 1024).await
}

async fn spawn_with_cap(upstream: &str, backend: &str, max_upstream_body_bytes: usize) -> String {
    let settings = Settings {
        proxy_bind_addr: "127.0.0.1:0".to_string(),
        upstream_base_url: upstream.to_string(),
        backend_base_url: backend.to_string(),
        app_env: "dev".to_string(),
        active_version_ttl_secs: 30,
        compiled_cache_capacity: 256,
        upstream_connect_timeout_secs: 2,
        upstream_read_timeout_secs: 10,
        max_upstream_body_bytes,
        max_decompressed_bytes: 16 * 1024 * 1024,
        sanitizer_config_path: "config/sanitizer.yaml".to_string(),
    };
    let http = reqwest::Client::new();
    let site_map = SiteMap::new(http.clone(), backend.to_string(), 30);
    let backend_client = BackendClient::new(http.clone(), backend.to_string(), 30);
    let sanitizer =
        rre_proxy::domain::applier::html_sanitizer::load_sanitizer("config/sanitizer.yaml")
            .unwrap();

    let state = AppState {
        settings: Arc::new(settings),
        http,
        site_map: Arc::new(site_map),
        backend: Arc::new(backend_client),
        compiled: Arc::new(CompiledCache::new(256)),
        registry: Arc::new(default_registry()),
        sanitizer: Arc::new(sanitizer),
    };

    let app = build_app(state);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    format!("http://{addr}")
}

#[tokio::test]
async fn injects_paywall_for_paywalled_article() {
    let upstream = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/article/1"))
        .respond_with(
            // Use `set_body_raw` so body + a SINGLE `text/html` content-type are
            // set atomically. `set_body_string` would force `text/plain` (and a
            // later `insert_header` can leave a duplicate/ordered content-type,
            // making the proxy's HTML gate flaky).
            ResponseTemplate::new(200).set_body_raw(
                r#"<html><head><meta name="paywall" content="true"></head><body><div id="article-body"><p>Body</p></div></body></html>"#
                    .as_bytes(),
                "text/html; charset=utf-8",
            ),
        )
        .mount(&upstream)
        .await;

    let backend = MockServer::start().await;
    mount_feature_list(&backend).await;
    Mock::given(method("GET"))
        .and(path(format!("/api/v1/features/{FEATURE}/active-version")))
        .respond_with(ResponseTemplate::new(200).set_body_json(active_version_body()))
        .mount(&backend)
        .await;

    let base = spawn(&upstream.uri(), &backend.uri()).await;

    let res = reqwest::Client::new()
        .get(format!("{base}/article/1"))
        .header("host", HOST)
        .send()
        .await
        .unwrap();

    assert_eq!(
        res.headers()
            .get("x-rre-apply-status")
            .and_then(|v| v.to_str().ok()),
        Some("ok")
    );
    let body = res.text().await.unwrap();
    assert!(body.contains("Subscribe to continue"), "body: {body}");
}

/// Spec §1/§2 + v2.2: a matched HTML feature stamps `x-rre-feature-<id>: true`
/// and appends a trusted `<script>window.rre.feature_expressions=…</script>`
/// immediately before `</body>`. The serialized JSON must contain NO raw `<`
/// (every `<` is `<`) so an embedded `</script>` cannot break out.
#[tokio::test]
async fn matched_html_feature_injects_script_before_body_close() {
    let upstream = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/article/3"))
        .respond_with(
            ResponseTemplate::new(200).set_body_raw(
                r#"<html><head><meta name="paywall" content="true"></head><body><div id="article-body"><p>Body</p></div></body></html>"#
                    .as_bytes(),
                "text/html; charset=utf-8",
            ),
        )
        .mount(&upstream)
        .await;

    let backend = MockServer::start().await;
    mount_feature_list(&backend).await;
    Mock::given(method("GET"))
        .and(path(format!("/api/v1/features/{FEATURE}/active-version")))
        .respond_with(ResponseTemplate::new(200).set_body_json(active_version_body()))
        .mount(&backend)
        .await;

    let base = spawn(&upstream.uri(), &backend.uri()).await;

    let res = reqwest::Client::new()
        .get(format!("{base}/article/3"))
        .header("host", HOST)
        .send()
        .await
        .unwrap();

    // §1: match-marker header.
    assert_eq!(
        res.headers()
            .get(format!("x-rre-feature-{FEATURE}"))
            .and_then(|v| v.to_str().ok()),
        Some("true"),
    );

    let body = res.text().await.unwrap();

    // The script appears immediately before </body> (final step, after the
    // sanitized component transform that injected the paywall).
    assert!(body.contains("Subscribe to continue"), "body: {body}");
    let marker = "<script>window.rre=window.rre||{};window.rre.feature_expressions=";
    let script_start = body
        .find(marker)
        .expect("feature_expressions script present");
    let body_close = body.rfind("</body>").expect("</body> present");
    assert!(
        script_start < body_close,
        "script must precede </body>; body: {body}"
    );

    // Extract the feature_expressions JSON between `=` and the next assignment
    // (`;window.rre.total_time_ms=`, a sibling of feature_expressions per spec item 7).
    let after = &body[script_start + marker.len()..];
    let json_end = after
        .find(";window.rre.total_time_ms=")
        .expect("total_time_ms sibling assignment");
    let json_part = &after[..json_end];
    // §2 escape: the serialized JSON carries NO raw `<` (so no `</script>` breakout).
    assert!(
        !json_part.contains('<'),
        "feature_expressions JSON must escape every `<`; got {json_part}"
    );
    // It still parses as JSON once `<` escapes are decoded by serde_json.
    let parsed: serde_json::Value = serde_json::from_str(json_part).unwrap();
    assert!(
        parsed.get(FEATURE).is_some(),
        "feature_expressions keyed by feature_id: {parsed}"
    );
    // v2.2: the entry carries an `expressions` array (the apply_outcome node).
    assert!(
        parsed[FEATURE]["expressions"].is_array(),
        "entry has an expressions array: {parsed}"
    );
    // The entry carries the served active `version` (== 1, the mounted version).
    assert_eq!(
        parsed[FEATURE]["version"],
        serde_json::json!(1),
        "entry carries the served active version_number: {parsed}"
    );
    // spec item 7: `window.rre.total_time_ms` is a top-level `d.dd` STRING sibling
    // of feature_expressions, assigned right after it (then compute_time_ms).
    let total_after = &after[json_end + ";window.rre.total_time_ms=".len()..];
    let total_end = total_after
        .find(";window.rre.compute_time_ms=")
        .expect("compute_time_ms sibling assignment");
    let total_part = &total_after[..total_end];
    let total: serde_json::Value = serde_json::from_str(total_part).unwrap();
    let total_str = total.as_str().expect("total_time_ms is a string");
    assert!(
        total_str.contains('.') && total_str.split('.').nth(1).is_some_and(|d| d.len() == 2),
        "total_time_ms is `d.dd`-formatted; got {total_str}"
    );
    // compute_time_ms: a `d.dd` STRING sibling, <= total_time_ms (excludes fetch I/O).
    let compute_after = &total_after[total_end + ";window.rre.compute_time_ms=".len()..];
    let compute_end = compute_after.find(";</script>").expect("script terminator");
    let compute_part = &compute_after[..compute_end];
    let compute: serde_json::Value = serde_json::from_str(compute_part).unwrap();
    let compute_str = compute.as_str().expect("compute_time_ms is a string");
    assert!(
        compute_str.contains('.') && compute_str.split('.').nth(1).is_some_and(|d| d.len() == 2),
        "compute_time_ms is `d.dd`-formatted; got {compute_str}"
    );
    let total_f: f64 = total_str.parse().unwrap();
    let compute_f: f64 = compute_str.parse().unwrap();
    assert!(
        compute_f <= total_f + 1e-9,
        "compute_time_ms ({compute_f}) <= total_time_ms ({total_f})"
    );
}

/// No host/path gating any more: the feature runs on every request and
/// self-gates. `/other` has no `paywall` meta, so the meta_tags decision branches
/// `no` -> no actions -> the response is served untouched (`skipped`).
#[tokio::test]
async fn passes_through_when_feature_self_gates() {
    let upstream = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/other"))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("content-type", "text/html")
                .set_body_string("<html><body>untouched</body></html>"),
        )
        .mount(&upstream)
        .await;

    let backend = MockServer::start().await;
    mount_feature_list(&backend).await;
    Mock::given(method("GET"))
        .and(path(format!("/api/v1/features/{FEATURE}/active-version")))
        .respond_with(ResponseTemplate::new(200).set_body_json(active_version_body()))
        .mount(&backend)
        .await;
    let base = spawn(&upstream.uri(), &backend.uri()).await;

    let res = reqwest::Client::new()
        .get(format!("{base}/other"))
        .header("host", HOST)
        .send()
        .await
        .unwrap();

    assert_eq!(
        res.headers()
            .get("x-rre-apply-status")
            .and_then(|v| v.to_str().ok()),
        Some("skipped")
    );
    let body = res.text().await.unwrap();
    assert!(body.contains("untouched"));
}

#[tokio::test]
async fn fails_open_when_backend_unavailable() {
    let upstream = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/article/2"))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("content-type", "text/html")
                .set_body_string(
                    r#"<html><head><meta name="paywall" content="true"></head><body>original</body></html>"#,
                ),
        )
        .mount(&upstream)
        .await;

    // Backend returns 404 for active-version -> proxy fails open (pass-through).
    let backend = MockServer::start().await;
    mount_feature_list(&backend).await;
    Mock::given(method("GET"))
        .and(path(format!("/api/v1/features/{FEATURE}/active-version")))
        .respond_with(ResponseTemplate::new(404))
        .mount(&backend)
        .await;

    let base = spawn(&upstream.uri(), &backend.uri()).await;

    let res = reqwest::Client::new()
        .get(format!("{base}/article/2"))
        .header("host", HOST)
        .send()
        .await
        .unwrap();

    assert_eq!(
        res.headers()
            .get("x-rre-apply-status")
            .and_then(|v| v.to_str().ok()),
        Some("skipped")
    );
    let body = res.text().await.unwrap();
    assert!(body.contains("original"));
}

/// Per-site header injection: when a request matches a Site, each configured header
/// OVERRIDES any client-supplied same-named header (the client cannot spoof a header
/// the site sets), and a configured header absent from the client request is added.
/// The client sends `X-Api-Key: client-value`; the matched Site configures
/// `X-Api-Key: site-value` + `X-Site-Only: present`. The upstream mock matches only
/// when it RECEIVES `X-Api-Key: site-value` and `X-Site-Only` exists.
#[tokio::test]
async fn matched_site_injects_and_overrides_upstream_headers() {
    let upstream = MockServer::start().await;
    // This mock matches only on the OVERRIDDEN api key + the added site-only header.
    // If the client value leaked through (no override) this mock would not match and
    // the request would 404 (no matching mock) -> the assertions below fail.
    Mock::given(method("GET"))
        .and(path("/article/site"))
        .and(wiremock::matchers::header("x-api-key", "site-value"))
        .and(wiremock::matchers::header_exists("x-site-only"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(
            r#"<html><head></head><body>routed</body></html>"#.as_bytes(),
            "text/html; charset=utf-8",
        ))
        .expect(1)
        .mount(&upstream)
        .await;

    let backend = MockServer::start().await;
    mount_feature_list(&backend).await;
    // The Site routes `rre.test:80` -> the wiremock upstream, with custom headers.
    mount_site_list(
        &backend,
        &upstream.uri(),
        json!({ "X-Api-Key": "site-value", "X-Site-Only": "present" }),
    )
    .await;
    Mock::given(method("GET"))
        .and(path(format!("/api/v1/features/{FEATURE}/active-version")))
        .respond_with(ResponseTemplate::new(200).set_body_json(active_version_body()))
        .mount(&backend)
        .await;

    // `upstream_base_url` is intentionally bogus so the test only passes when the
    // matched Site (not the fallback) routes the request to the real upstream.
    let base = spawn("http://127.0.0.1:1", &backend.uri()).await;

    let res = reqwest::Client::new()
        .get(format!("{base}/article/site"))
        .header("host", HOST)
        // Client tries to spoof the api key — the Site value must win.
        .header("x-api-key", "client-value")
        .send()
        .await
        .unwrap();

    assert_eq!(
        res.status().as_u16(),
        200,
        "matched Site routes to upstream"
    );
    let body = res.text().await.unwrap();
    assert!(
        body.contains("routed"),
        "served the matched upstream body: {body}"
    );

    // Precise assertion on what the upstream actually received.
    let requests = upstream
        .received_requests()
        .await
        .expect("recording enabled");
    let req = requests
        .iter()
        .find(|r| r.url.path() == "/article/site")
        .expect("upstream received the proxied request");
    let api_keys: Vec<&str> = req
        .headers
        .get_all("x-api-key")
        .iter()
        .filter_map(|v| v.to_str().ok())
        .collect();
    assert_eq!(
        api_keys,
        vec!["site-value"],
        "configured header REPLACES the client value (override, single value)"
    );
    assert_eq!(
        req.headers.get("x-site-only").and_then(|v| v.to_str().ok()),
        Some("present"),
        "a site header absent from the client request is added"
    );
}

/// Defense-in-depth denylist: a configured Site header on the forbidden list
/// (`Host`, `Content-Length`, hop-by-hop) is NOT forwarded and can never override
/// the proxy-managed value. Here the Site config tries to set `Host: evil.test`;
/// the upstream must still receive the proxy's destination Host (`127.0.0.1`), and
/// an allowed sibling header (`X-Ok`) must still be applied (the forbidden entry is
/// skipped, not the whole map).
#[tokio::test]
async fn matched_site_cannot_override_forbidden_headers() {
    let upstream = MockServer::start().await;
    // Match only on the allowed header to confirm the request still routed; the
    // Host assertion below is checked against the recorded request, not matched on
    // (so a leaked `evil.test` Host would be caught precisely, not silently 404).
    Mock::given(method("GET"))
        .and(path("/article/forbidden"))
        .and(wiremock::matchers::header("x-ok", "yes"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(
            r#"<html><head></head><body>routed</body></html>"#.as_bytes(),
            "text/html; charset=utf-8",
        ))
        .expect(1)
        .mount(&upstream)
        .await;

    let backend = MockServer::start().await;
    mount_feature_list(&backend).await;
    // The Site config attempts to set proxy-managed `Host` + `Content-Length`
    // (must be skipped) alongside an allowed `X-Ok` (must be applied).
    mount_site_list(
        &backend,
        &upstream.uri(),
        json!({ "Host": "evil.test", "Content-Length": "999", "X-Ok": "yes" }),
    )
    .await;
    Mock::given(method("GET"))
        .and(path(format!("/api/v1/features/{FEATURE}/active-version")))
        .respond_with(ResponseTemplate::new(200).set_body_json(active_version_body()))
        .mount(&backend)
        .await;

    let base = spawn("http://127.0.0.1:1", &backend.uri()).await;

    let res = reqwest::Client::new()
        .get(format!("{base}/article/forbidden"))
        .header("host", HOST)
        .send()
        .await
        .unwrap();

    assert_eq!(res.status().as_u16(), 200, "allowed header still routes");

    let requests = upstream
        .received_requests()
        .await
        .expect("recording enabled");
    let req = requests
        .iter()
        .find(|r| r.url.path() == "/article/forbidden")
        .expect("upstream received the proxied request");

    // The proxy-managed Host (the dest host = 127.0.0.1) must win; the config's
    // `evil.test` must never reach upstream.
    let host = req.headers.get("host").and_then(|v| v.to_str().ok());
    assert_eq!(
        host,
        Some("127.0.0.1"),
        "forbidden `Host` config must NOT override the proxy-managed Host: {host:?}"
    );
    // The allowed sibling header IS applied (denylist skips only the bad entry).
    assert_eq!(
        req.headers.get("x-ok").and_then(|v| v.to_str().ok()),
        Some("yes"),
        "an allowed header alongside a forbidden one is still applied"
    );
}

/// H2: an upstream body larger than `max_upstream_body_bytes` is rejected with a
/// 502 (clean error) rather than buffered unbounded — the proxy never OOMs on a
/// large/streamed upstream body.
#[tokio::test]
async fn rejects_oversized_upstream_body() {
    let big = "x".repeat(64 * 1024); // 64 KiB body, cap set to 1 KiB below.
    let upstream = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/article/big"))
        .respond_with(
            ResponseTemplate::new(200).set_body_raw(big.into_bytes(), "text/html; charset=utf-8"),
        )
        .mount(&upstream)
        .await;

    let backend = MockServer::start().await;
    mount_feature_list(&backend).await;
    Mock::given(method("GET"))
        .and(path(format!("/api/v1/features/{FEATURE}/active-version")))
        .respond_with(ResponseTemplate::new(200).set_body_json(active_version_body()))
        .mount(&backend)
        .await;

    let base = spawn_with_cap(&upstream.uri(), &backend.uri(), 1024).await;

    let res = reqwest::Client::new()
        .get(format!("{base}/article/big"))
        .header("host", HOST)
        .send()
        .await
        .unwrap();

    assert_eq!(res.status().as_u16(), 502, "oversized body -> 502 (H2)");
}

// ---------------------------------------------------------------------------
// `POST /__rre/eval-url`: fetch a REAL upstream response THROUGH the proxy and
// run the EDITOR canvas (request body) against it. Unlike the live forwarder it
// does not consult the feature list / active version — it evaluates the posted
// canvas — but it reuses the same Site routing + header injection.
// ---------------------------------------------------------------------------

/// eval-url fetches the matched Site's upstream with the Site headers applied, runs
/// the posted canvas against the REAL response, and returns the same journey shape
/// as `/__rre/eval`. A test header that collides with a Site default must NOT
/// override it (the upstream mock matches only on the Site value); a non-colliding
/// test header is forwarded.
#[tokio::test]
async fn eval_url_runs_editor_canvas_against_live_upstream() {
    let upstream = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/article/1"))
        // Matches only on the OVERRIDDEN site api key + the added site-only header.
        .and(wiremock::matchers::header("x-api-key", "site-value"))
        .and(wiremock::matchers::header_exists("x-site-only"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(
            r#"<html><head><meta name="paywall" content="true"></head><body><div id="article-body"><p>Body</p></div></body></html>"#
                .as_bytes(),
            "text/html; charset=utf-8",
        ))
        .expect(1)
        .mount(&upstream)
        .await;

    let backend = MockServer::start().await;
    // Only the Site list is needed — eval-url evaluates the request-body canvas.
    mount_site_list(
        &backend,
        &upstream.uri(),
        json!({ "X-Api-Key": "site-value", "X-Site-Only": "present" }),
    )
    .await;

    // Bogus `upstream_base_url` so ONLY the matched Site can route the request.
    let base = spawn("http://127.0.0.1:1", &backend.uri()).await;

    let res = reqwest::Client::new()
        .post(format!("{base}/__rre/eval-url"))
        .json(&json!({
            "canvas": eval_canvas(),
            "url": format!("http://{HOST}/article/1"),
            // A test header tries to spoof the Site api key (Site must win) + a sibling.
            "headers": { "X-Api-Key": "client-value", "X-Extra": "from-test" }
        }))
        .send()
        .await
        .unwrap();

    assert_eq!(res.status().as_u16(), 200, "eval-url succeeds");
    let body: serde_json::Value = res.json().await.unwrap();

    // The matched terminal node is the paywall expression (meta paywall=true -> yes).
    assert_eq!(body["matched_node_id"], json!("n_paywall"));
    // The journey ran start -> ... -> end against the REAL fetched HTML.
    let journey = body["journey"].as_array().expect("journey array");
    assert_eq!(journey.first().unwrap()["kind"], json!("start"));
    assert_eq!(journey.last().unwrap()["kind"], json!("end"));
    let start_body = journey.first().unwrap()["body_after"]
        .as_str()
        .unwrap_or("");
    assert!(
        start_body.contains("article-body"),
        "journey ran against the fetched upstream HTML: {start_body}"
    );

    // The upstream received the SITE api key (override), not the test value, and the
    // non-colliding test header was forwarded.
    let requests = upstream
        .received_requests()
        .await
        .expect("recording enabled");
    let req = requests
        .iter()
        .find(|r| r.url.path() == "/article/1")
        .expect("upstream received the proxied request");
    let api_keys: Vec<&str> = req
        .headers
        .get_all("x-api-key")
        .iter()
        .filter_map(|v| v.to_str().ok())
        .collect();
    assert_eq!(
        api_keys,
        vec!["site-value"],
        "Site header WINS over the colliding test header (no override)"
    );
    assert_eq!(
        req.headers.get("x-extra").and_then(|v| v.to_str().ok()),
        Some("from-test"),
        "a non-colliding test header is forwarded to the upstream"
    );
}

/// SSRF guard (configured sites only): a URL whose host matches no configured Site
/// is rejected with 400 `NO_SITE` — the proxy never fetches an arbitrary host.
#[tokio::test]
async fn eval_url_rejects_unconfigured_host() {
    let backend = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/v1/sites"))
        .and(query_param("page_size", "100"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "items": [] })))
        .mount(&backend)
        .await;
    let base = spawn("http://127.0.0.1:1", &backend.uri()).await;

    let res = reqwest::Client::new()
        .post(format!("{base}/__rre/eval-url"))
        .json(&json!({
            "canvas": eval_canvas(),
            "url": "http://unknown.test/article/1",
            "headers": {}
        }))
        .send()
        .await
        .unwrap();

    assert_eq!(
        res.status().as_u16(),
        400,
        "unconfigured host -> 400 (configured sites only)"
    );
    let body: serde_json::Value = res.json().await.unwrap();
    assert_eq!(body["error"]["code"], json!("NO_SITE"));
}

/// A malformed / non-http(s) URL is rejected with 400 `BAD_URL` before any fetch.
#[tokio::test]
async fn eval_url_rejects_bad_url() {
    let backend = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/v1/sites"))
        .and(query_param("page_size", "100"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "items": [] })))
        .mount(&backend)
        .await;
    let base = spawn("http://127.0.0.1:1", &backend.uri()).await;

    let res = reqwest::Client::new()
        .post(format!("{base}/__rre/eval-url"))
        .json(&json!({ "canvas": eval_canvas(), "url": "ftp://nope", "headers": {} }))
        .send()
        .await
        .unwrap();

    assert_eq!(res.status().as_u16(), 400, "non-http(s) URL -> 400");
    let body: serde_json::Value = res.json().await.unwrap();
    assert_eq!(body["error"]["code"], json!("BAD_URL"));
}

/// A JSON editor canvas for eval-url: json_expression(api==demo-article) -> trim_json
/// -> add_attribute -> end. Uses body-mutating actions (no outcomes needed), so the
/// journey shows REAL JSON body changes through a live fetch.
fn json_eval_canvas() -> serde_json::Value {
    json!({
        "root_node_id": "start",
        "nodes": [
            { "kind": "start", "id": "start", "position": { "x": -200.0, "y": 0.0 } },
            { "kind": "decision", "id": "d_api",
              "processor": { "type": "json_expression", "json_path": "$.api", "operator": "equals", "value": "demo-article" },
              "position": { "x": 0.0, "y": 0.0 } },
            { "kind": "expression", "id": "t_body",
              "action": { "type": "trim_json", "json_path": "$.body", "length": 0 },
              "position": { "x": 200.0, "y": 0.0 } },
            { "kind": "expression", "id": "a_pw",
              "action": { "type": "add_attribute", "json_path": "$.paywall_show", "value": "<html>paywall_showed</html>" },
              "position": { "x": 400.0, "y": 0.0 } },
            { "kind": "end", "id": "end", "position": { "x": 600.0, "y": 100.0 } }
        ],
        "edges": [
            { "id": "e0", "source_node_id": "start",  "target_node_id": "d_api",  "branch": "yes" },
            { "id": "e1", "source_node_id": "d_api",  "target_node_id": "t_body", "branch": "yes" },
            { "id": "e2", "source_node_id": "d_api",  "target_node_id": "end",    "branch": "no"  },
            { "id": "e3", "source_node_id": "t_body", "target_node_id": "a_pw",   "branch": "yes" },
            { "id": "e4", "source_node_id": "a_pw",   "target_node_id": "end",    "branch": "yes" }
        ]
    })
}

/// eval-url against a REAL JSON upstream: the journey replays the JSON body
/// mutations (trim then add_attribute) on the fetched body — exercising the JSON
/// branch of EvalFetch/build_journey end to end.
#[tokio::test]
async fn eval_url_runs_json_canvas_against_json_upstream() {
    let upstream = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/article/json"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(json!({ "api": "demo-article", "body": [1, 2, 3] })),
        )
        .expect(1)
        .mount(&upstream)
        .await;

    let backend = MockServer::start().await;
    mount_site_list(&backend, &upstream.uri(), json!({})).await;
    let base = spawn("http://127.0.0.1:1", &backend.uri()).await;

    let res = reqwest::Client::new()
        .post(format!("{base}/__rre/eval-url"))
        .json(&json!({
            "canvas": json_eval_canvas(),
            "url": format!("http://{HOST}/article/json"),
            "headers": {}
        }))
        .send()
        .await
        .unwrap();

    assert_eq!(res.status().as_u16(), 200, "eval-url (json) succeeds");
    let body: serde_json::Value = res.json().await.unwrap();

    assert_eq!(body["matched_node_id"], json!("a_pw"));
    let journey = body["journey"].as_array().expect("journey array");
    // start, d_api, t_body, a_pw, end.
    assert_eq!(journey.len(), 5, "journey: {journey:?}");
    // Started from the REAL fetched JSON body.
    assert_eq!(journey[0]["body_after"]["body"], json!([1, 2, 3]));
    // trim_json emptied the array...
    assert_eq!(journey[2]["node_id"], json!("t_body"));
    assert_eq!(journey[2]["body_after"]["body"], json!([]));
    // ...and add_attribute set the new field on the fetched body.
    assert_eq!(journey[3]["node_id"], json!("a_pw"));
    assert_eq!(
        journey[3]["body_after"]["paywall_show"],
        json!("<html>paywall_showed</html>")
    );
}

/// eval-url with NO Site headers configured + an INVALID test-header name: the
/// invalid header is silently dropped (never reaches the upstream), the valid one
/// is forwarded, and the eval still succeeds. Covers the empty-Site-headers
/// baseline and the defensive `headers_to_map` drop.
#[tokio::test]
async fn eval_url_drops_invalid_test_headers() {
    let upstream = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/article/h"))
        .and(wiremock::matchers::header_exists("x-good"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(
            r#"<html><head><meta name="paywall" content="true"></head><body><div id="article-body"><p>Body</p></div></body></html>"#
                .as_bytes(),
            "text/html; charset=utf-8",
        ))
        .expect(1)
        .mount(&upstream)
        .await;

    let backend = MockServer::start().await;
    // Empty Site headers — baseline (the injection loop is a no-op).
    mount_site_list(&backend, &upstream.uri(), json!({})).await;
    let base = spawn("http://127.0.0.1:1", &backend.uri()).await;

    let res = reqwest::Client::new()
        .post(format!("{base}/__rre/eval-url"))
        .json(&json!({
            "canvas": eval_canvas(),
            "url": format!("http://{HOST}/article/h"),
            // "X-Bad Header" has a space -> not a valid HTTP token -> dropped.
            "headers": { "X-Bad Header": "leak", "X-Good": "ok" }
        }))
        .send()
        .await
        .unwrap();

    assert_eq!(res.status().as_u16(), 200, "valid headers still route");

    let requests = upstream
        .received_requests()
        .await
        .expect("recording enabled");
    let req = requests
        .iter()
        .find(|r| r.url.path() == "/article/h")
        .expect("upstream received the proxied request");
    assert_eq!(
        req.headers.get("x-good").and_then(|v| v.to_str().ok()),
        Some("ok"),
        "a valid test header is forwarded"
    );
    assert!(
        req.headers.get("x-bad header").is_none(),
        "an invalid test-header name is dropped, never forwarded"
    );
}

/// Content-type filter: on an HTML response the proxy evaluates ONLY html-type
/// features — a json-type feature's active-version endpoint is NEVER fetched.
/// The reverse of `json_response_does_not_fetch_or_eval_html_features`.
#[tokio::test]
async fn html_response_does_not_fetch_or_eval_json_features() {
    const JSON_FEATURE: &str = "demo-json-feature";

    let upstream = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/article/ct"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(
            r#"<html><head><meta name="paywall" content="true"></head><body><div id="article-body"><p>Body</p></div></body></html>"#
                .as_bytes(),
            "text/html; charset=utf-8",
        ))
        .mount(&upstream)
        .await;

    let backend = MockServer::start().await;
    // Feature list with BOTH an html and a json feature.
    Mock::given(method("GET"))
        .and(path("/api/v1/features"))
        .and(query_param("page_size", "100"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "items": [
                { "id": FEATURE,      "name": FEATURE,      "type": "html", "execution_order": 1 },
                { "id": JSON_FEATURE, "name": JSON_FEATURE, "type": "json", "execution_order": 1 }
            ]
        })))
        .mount(&backend)
        .await;
    // The html feature IS fetched + applied.
    Mock::given(method("GET"))
        .and(path(format!("/api/v1/features/{FEATURE}/active-version")))
        .respond_with(ResponseTemplate::new(200).set_body_json(active_version_body()))
        .mount(&backend)
        .await;
    // The json feature's active-version MUST NOT be fetched on an HTML response.
    Mock::given(method("GET"))
        .and(path(format!(
            "/api/v1/features/{JSON_FEATURE}/active-version"
        )))
        .respond_with(ResponseTemplate::new(200).set_body_json(active_version_body()))
        .expect(0)
        .mount(&backend)
        .await;

    let base = spawn(&upstream.uri(), &backend.uri()).await;
    let res = reqwest::Client::new()
        .get(format!("{base}/article/ct"))
        .header("host", HOST)
        .send()
        .await
        .unwrap();

    assert_eq!(res.status().as_u16(), 200);
    // The html feature applied (paywall injected); the json feature never ran.
    assert_eq!(
        res.headers()
            .get(format!("x-rre-feature-{FEATURE}"))
            .and_then(|v| v.to_str().ok()),
        Some("true"),
        "html feature applied (filter kept it)"
    );
    assert!(
        res.headers()
            .get(format!("x-rre-feature-{JSON_FEATURE}"))
            .is_none(),
        "json feature must not run on an HTML response"
    );
    let body = res.text().await.unwrap();
    assert!(body.contains("Subscribe to continue"), "body: {body}");
    // The `expect(0)` mock is verified on drop (no json active-version fetch).
}
