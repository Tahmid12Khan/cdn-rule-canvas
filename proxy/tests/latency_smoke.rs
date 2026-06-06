//! Latency smoke: fire 100 concurrent `/__rre/eval` requests through the full
//! `build_app` stack (the pure-eval hot path — no upstream/backend fetch) and
//! assert the p95 round-trip stays under a CI-safe target. Prints p50/p95/p99 so
//! the numbers are visible in `cargo test -- --nocapture`.
//!
//! This guards the hot-path eval-overhead budget (spec/CLAUDE p99 eval < 5ms) end
//! to end. The asserted bound here is the request round-trip under concurrency
//! (HTTP + JSON + translate + evaluate), so it is intentionally looser than the
//! in-process eval-only budget — it catches gross regressions, not microseconds.

use std::sync::Arc;

use rre_proxy::build_app;
use rre_proxy::config::Settings;
use rre_proxy::domain::processors::default_registry;
use rre_proxy::infra::backend_client::BackendClient;
use rre_proxy::infra::compiled_cache::CompiledCache;
use rre_proxy::infra::site_map::SiteMap;
use rre_proxy::state::AppState;
use serde_json::{json, Value};

/// start -> meta_tags(paywall) -> device_type(mobile) -> {regwall|paywall} -> end.
fn canvas() -> Value {
    json!({
        "root_node_id": "start",
        "nodes": [
            { "kind": "start", "id": "start", "position": { "x": 0.0, "y": 0.0 } },
            { "kind": "decision", "id": "n_meta",
              "processor": { "type": "meta_tags", "tag_name": "paywall", "operator": "contains", "value": "true" },
              "position": { "x": 200.0, "y": 0.0 } },
            { "kind": "decision", "id": "n_dev",
              "processor": { "type": "device_type", "operator": "equals", "value": "mobile" },
              "position": { "x": 400.0, "y": 0.0 } },
            { "kind": "expression", "id": "n_regwall",
              "action": { "type": "apply_outcome", "outcome_id": "11111111-1111-1111-1111-111111111111" },
              "position": { "x": 600.0, "y": -80.0 } },
            { "kind": "expression", "id": "n_paywall",
              "action": { "type": "apply_outcome", "outcome_id": "22222222-2222-2222-2222-222222222222" },
              "position": { "x": 600.0, "y": 80.0 } },
            { "kind": "end", "id": "end", "position": { "x": 800.0, "y": 0.0 } }
        ],
        "edges": [
            { "id": "e0", "source_node_id": "start",     "target_node_id": "n_meta",    "branch": "yes" },
            { "id": "e1", "source_node_id": "n_meta",    "target_node_id": "n_dev",     "branch": "yes" },
            { "id": "e2", "source_node_id": "n_meta",    "target_node_id": "end",       "branch": "no"  },
            { "id": "e3", "source_node_id": "n_dev",     "target_node_id": "n_regwall", "branch": "yes" },
            { "id": "e4", "source_node_id": "n_dev",     "target_node_id": "n_paywall", "branch": "no"  },
            { "id": "e5", "source_node_id": "n_regwall", "target_node_id": "end",       "branch": "yes" },
            { "id": "e6", "source_node_id": "n_paywall", "target_node_id": "end",       "branch": "yes" }
        ]
    })
}

async fn spawn_app() -> String {
    let settings = Settings {
        proxy_bind_addr: "127.0.0.1:0".to_string(),
        upstream_base_url: "http://127.0.0.1:1".to_string(),
        backend_base_url: "http://127.0.0.1:1".to_string(),
        app_env: "dev".to_string(),
        active_version_ttl_secs: 30,
        compiled_cache_capacity: 256,
        upstream_connect_timeout_secs: 2,
        upstream_read_timeout_secs: 10,
        max_upstream_body_bytes: 16 * 1024 * 1024,
        max_decompressed_bytes: 16 * 1024 * 1024,
        sanitizer_config_path: "config/sanitizer.yaml".to_string(),
    };
    let http = reqwest::Client::new();
    let site_map = SiteMap::new(http.clone(), settings.backend_base_url.clone(), 30);
    let backend = BackendClient::new(http.clone(), settings.backend_base_url.clone(), 30);
    let sanitizer =
        rre_proxy::domain::applier::html_sanitizer::load_sanitizer("config/sanitizer.yaml")
            .unwrap();
    let state = AppState {
        settings: Arc::new(settings),
        http,
        site_map: Arc::new(site_map),
        backend: Arc::new(backend),
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

fn percentile(sorted: &[f64], p: f64) -> f64 {
    if sorted.is_empty() {
        return 0.0;
    }
    let rank = (p / 100.0 * (sorted.len() - 1) as f64).round() as usize;
    sorted[rank.min(sorted.len() - 1)]
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn eval_p95_under_target_at_100_concurrency() {
    let base = spawn_app().await;
    let body = json!({
        "canvas": canvas(),
        "context": { "device_type": "mobile", "meta_tags": { "paywall": "true" } }
    });

    // Warm up the compiled-graph cache (first eval compiles + caches the canvas).
    let client = reqwest::Client::new();
    let warm = client
        .post(format!("{base}/__rre/eval"))
        .json(&body)
        .send()
        .await
        .unwrap();
    assert_eq!(warm.status(), 200);

    // 100 concurrent requests; record each round-trip latency in ms.
    const N: usize = 100;
    let mut handles = Vec::with_capacity(N);
    for _ in 0..N {
        let client = client.clone();
        let url = format!("{base}/__rre/eval");
        let body = body.clone();
        handles.push(tokio::spawn(async move {
            let t = std::time::Instant::now();
            let res = client.post(&url).json(&body).send().await.unwrap();
            let status = res.status();
            let _ = res.bytes().await.unwrap();
            (status, t.elapsed().as_secs_f64() * 1000.0)
        }));
    }

    let mut latencies = Vec::with_capacity(N);
    for h in handles {
        let (status, ms) = h.await.unwrap();
        assert_eq!(status, 200, "every concurrent eval returns 200");
        latencies.push(ms);
    }
    latencies.sort_by(|a, b| a.partial_cmp(b).unwrap());

    let p50 = percentile(&latencies, 50.0);
    let p95 = percentile(&latencies, 95.0);
    let p99 = percentile(&latencies, 99.0);
    let line =
        format!("eval latency @ {N} concurrency: p50={p50:.2}ms p95={p95:.2}ms p99={p99:.2}ms");
    println!("{line}");
    // Also persist to a temp file so the numbers are recoverable even when the
    // test harness collapses stdout.
    let _ = std::fs::write(std::env::temp_dir().join("rre_eval_latency.txt"), &line);

    // CI-safe bound: the round-trip (HTTP + translate + evaluate) p95 must stay
    // well under 100ms. A regression that blocks the runtime or drops caching
    // blows past this; the in-process eval overhead is a small fraction of it.
    assert!(
        p95 < 100.0,
        "eval p95 {p95:.2}ms exceeded 100ms target (p50={p50:.2} p99={p99:.2})"
    );
}
