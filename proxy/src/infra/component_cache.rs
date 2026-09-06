//! Component-template resolve cache (Component Editor, design §4.1).
//!
//! Mirrors the active-version STALE-WHILE-REVALIDATE cache in
//! [`crate::infra::backend_client`]: a cache hit older than `ttl` is returned
//! IMMEDIATELY (the request never blocks on the refetch) while a background task
//! refreshes the entry. SWR is what gives the spec-item-4 auto-update for a
//! `"default"` reference within the TTL window: changing the component's default
//! in the editor re-points all default-following rules once the entry refreshes.
//!
//! Keyed by `(component_id, VersionSelector)` so a `"default"` reference and a
//! pinned `Version(n)` reference to the same component cache independently. The
//! value is `Arc<ResolvedComponent>` (cheap to clone into the per-request
//! resolved-component map). Any 404 / error on the synchronous (MISS) path
//! returns `None` so the applier fails open (component skipped, body untouched);
//! a background refresh that sees a 404/err INVALIDATES the entry so a deleted
//! component stops being served stale. Never logs bodies.

use std::sync::Arc;
use std::time::{Duration, Instant};

use moka::future::Cache;
use uuid::Uuid;

use crate::infra::backend_client::{ResolvedComponent, VersionSelector};

/// A cached resolve entry stamped with its fetch time. As in `backend_client`,
/// freshness is decided by `fetched_at.elapsed() < ttl` (SWR), not by a moka TTL
/// (the cache has only a long safety cap so abandoned keys are eventually evicted).
#[derive(Clone)]
struct Stamped {
    value: Arc<ResolvedComponent>,
    fetched_at: Instant,
}

/// Three-way fetch outcome so the SWR caller can distinguish "definitively gone"
/// (404 → invalidate stale) from a transient error (keep stale).
enum FetchResult {
    Ok(Arc<ResolvedComponent>),
    /// 404 — the component (or its only resolvable default) is gone.
    NotFound,
    /// Transport / non-404 non-2xx / decode error — transient; keep any stale entry.
    Error,
}

type Key = (Uuid, VersionSelector);

pub struct ComponentCache {
    http: reqwest::Client,
    base: String,
    /// SWR cache keyed by `(component_id, selector)`. No moka TTL (entries are
    /// explicitly refreshed); a long safety cap bounds memory for abandoned keys.
    cache: Cache<Key, Stamped>,
    /// Freshness window: `fetched_at.elapsed() < ttl` ⇒ fresh; else stale (SWR).
    ttl: Duration,
    /// Anti-thundering-herd: at most one background refresh per key.
    refresh_inflight: Cache<Key, ()>,
}

impl ComponentCache {
    pub fn new(http: reqwest::Client, base: String, ttl_secs: u64) -> Self {
        let ttl = Duration::from_secs(ttl_secs);
        // Freshness is decided by the stamp; moka gets a long SAFETY CAP (floored
        // at 5 min) so a tiny/zero `ttl` never evicts an entry before the next
        // request reads + revalidates it — identical to backend_client.
        const SAFETY_FLOOR: Duration = Duration::from_secs(300);
        let safety_cap = ttl.saturating_mul(20).max(SAFETY_FLOOR);
        // Bounded size (design: "size bounded, e.g. 256") so the global component
        // library can't grow proxy memory without limit.
        let cache = Cache::builder()
            .time_to_live(safety_cap)
            .max_capacity(256)
            .build();
        let inflight_ttl = ttl.max(Duration::from_secs(10));
        let refresh_inflight = Cache::builder()
            .time_to_live(inflight_ttl)
            .max_capacity(256)
            .build();
        Self {
            http,
            base,
            cache,
            ttl,
            refresh_inflight,
        }
    }

    /// `GET {base}/api/v1/component-templates/{id}/resolve?version=...`,
    /// STALE-WHILE-REVALIDATE.
    ///
    /// - HIT + fresh → return the cached value.
    /// - HIT + stale → return the STALE value IMMEDIATELY and spawn ONE background
    ///   refresh (in-flight guarded). Ok → re-stamp; 404 → invalidate; transient
    ///   error → keep stale.
    /// - MISS → fetch synchronously, stamp+insert on Ok, return. `None` on 404 /
    ///   any transport or decode error (fail-open; NOT cached).
    pub async fn resolve_component(
        &self,
        component_id: Uuid,
        selector: VersionSelector,
    ) -> Option<Arc<ResolvedComponent>> {
        let key = (component_id, selector);

        if let Some(stamped) = self.cache.get(&key).await {
            if stamped.fetched_at.elapsed() < self.ttl {
                return Some(stamped.value); // fresh.
            }
            self.spawn_refresh(key);
            return Some(stamped.value); // stale: serve immediately.
        }

        match fetch_inner(&self.http, &self.base, component_id, selector).await {
            FetchResult::Ok(rc) => {
                self.cache
                    .insert(
                        key,
                        Stamped {
                            value: rc.clone(),
                            fetched_at: Instant::now(),
                        },
                    )
                    .await;
                Some(rc)
            }
            FetchResult::NotFound | FetchResult::Error => None,
        }
    }

    /// Spawn at most one background refresh per key for a stale entry. All captured
    /// state is cheap (Arc-backed `Cache` handles, cloned `reqwest::Client`).
    fn spawn_refresh(&self, key: Key) {
        let inflight = self.refresh_inflight.clone();
        let cache = self.cache.clone();
        let http = self.http.clone();
        let base = self.base.clone();
        let (component_id, selector) = key;
        tokio::spawn(async move {
            if inflight.get(&key).await.is_some() {
                return;
            }
            inflight.insert(key, ()).await;

            match fetch_inner(&http, &base, component_id, selector).await {
                FetchResult::Ok(rc) => {
                    cache
                        .insert(
                            key,
                            Stamped {
                                value: rc,
                                fetched_at: Instant::now(),
                            },
                        )
                        .await;
                }
                // A deleted/unresolvable component (404) must STOP being served
                // stale: invalidate so the next request MISSes → None → fail-open.
                FetchResult::NotFound => {
                    cache.invalidate(&key).await;
                }
                // Transient error: keep the stale value (already served); the next
                // stale hit retries.
                FetchResult::Error => {}
            }
            inflight.invalidate(&key).await;
        });
    }
}

/// Free-function fetch shared by the synchronous MISS path and the background
/// refresh (so the task captures only cheap clones, not `&self`). Never logs the
/// rendered/template body — only metadata.
async fn fetch_inner(
    http: &reqwest::Client,
    base: &str,
    component_id: Uuid,
    selector: VersionSelector,
) -> FetchResult {
    let url = format!(
        "{}/api/v1/component-templates/{}/resolve?version={}",
        base.trim_end_matches('/'),
        component_id,
        selector.as_query()
    );

    let resp = match http.get(&url).send().await {
        Ok(r) => r,
        Err(e) => {
            tracing::warn!(error = %e, component_id = %component_id, "resolve_component=miss (transport error)");
            return FetchResult::Error;
        }
    };

    let status = resp.status();
    if !status.is_success() {
        tracing::warn!(status = %status, component_id = %component_id, "resolve_component=miss (non-2xx)");
        return if status == reqwest::StatusCode::NOT_FOUND {
            FetchResult::NotFound
        } else {
            FetchResult::Error
        };
    }

    match resp.json::<ResolvedComponent>().await {
        Ok(rc) => FetchResult::Ok(Arc::new(rc)),
        Err(e) => {
            tracing::warn!(error = %e, component_id = %component_id, "resolve_component=miss (decode error)");
            FetchResult::Error
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::matchers::{method, path, query_param};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn body(version_number: i32, html: &str) -> serde_json::Value {
        serde_json::json!({
            "version_number": version_number,
            "html_body": html,
            "variables": []
        })
    }

    /// A fresh resolve hit caches: repeated calls within the TTL hit the backend
    /// exactly once.
    #[tokio::test]
    async fn resolve_within_ttl_hits_backend_once() {
        let server = MockServer::start().await;
        let cid = Uuid::new_v4();
        Mock::given(method("GET"))
            .and(path(format!("/api/v1/component-templates/{cid}/resolve")))
            .and(query_param("version", "default"))
            .respond_with(ResponseTemplate::new(200).set_body_json(body(3, "<p>{{x}}</p>")))
            .expect(1)
            .mount(&server)
            .await;

        let cache = ComponentCache::new(reqwest::Client::new(), server.uri(), 3600);
        for _ in 0..5 {
            let rc = cache
                .resolve_component(cid, VersionSelector::Default)
                .await
                .unwrap();
            assert_eq!(rc.version_number, 3);
            assert_eq!(rc.html_body, "<p>{{x}}</p>");
        }
    }

    /// The resolve endpoint falls back to the current default when a pinned version
    /// is missing (still 200, design §4.1) — the proxy just caches whatever 200 it
    /// gets, so a `Version(n)` that the backend resolves to the default is served.
    #[tokio::test]
    async fn missing_version_falls_back_to_default_via_backend() {
        let server = MockServer::start().await;
        let cid = Uuid::new_v4();
        // version=9 is gone; backend returns the current default (v2) with 200.
        Mock::given(method("GET"))
            .and(path(format!("/api/v1/component-templates/{cid}/resolve")))
            .and(query_param("version", "9"))
            .respond_with(ResponseTemplate::new(200).set_body_json(body(2, "<p>default</p>")))
            .expect(1)
            .mount(&server)
            .await;

        let cache = ComponentCache::new(reqwest::Client::new(), server.uri(), 3600);
        let rc = cache
            .resolve_component(cid, VersionSelector::Version(9))
            .await
            .unwrap();
        assert_eq!(rc.version_number, 2, "backend fell back to current default");
    }

    /// A truly unresolvable default → 404 → `None` (fail-open). Not cached: the
    /// next request retries.
    #[tokio::test]
    async fn unresolvable_returns_none() {
        let server = MockServer::start().await;
        let cid = Uuid::new_v4();
        Mock::given(method("GET"))
            .and(path(format!("/api/v1/component-templates/{cid}/resolve")))
            .respond_with(ResponseTemplate::new(404))
            .mount(&server)
            .await;

        let cache = ComponentCache::new(reqwest::Client::new(), server.uri(), 3600);
        assert!(cache
            .resolve_component(cid, VersionSelector::Default)
            .await
            .is_none());
    }

    /// SWR: after `ttl` a stale hit serves the OLD value immediately, and a
    /// background refresh updates the entry to the new resolved version.
    #[tokio::test]
    async fn stale_serves_immediately_then_background_refreshes() {
        let server = MockServer::start().await;
        let cid = Uuid::new_v4();
        Mock::given(method("GET"))
            .and(path(format!("/api/v1/component-templates/{cid}/resolve")))
            .respond_with(ResponseTemplate::new(200).set_body_json(body(1, "<p>v1</p>")))
            .up_to_n_times(1)
            .expect(1)
            .mount(&server)
            .await;

        let cache = ComponentCache::new(reqwest::Client::new(), server.uri(), 0);
        let first = cache
            .resolve_component(cid, VersionSelector::Default)
            .await
            .unwrap();
        assert_eq!(first.version_number, 1);

        // Default now resolves to v2 (e.g. a new version was created / pinned).
        Mock::given(method("GET"))
            .and(path(format!("/api/v1/component-templates/{cid}/resolve")))
            .respond_with(ResponseTemplate::new(200).set_body_json(body(2, "<p>v2</p>")))
            .mount(&server)
            .await;

        let stale = cache
            .resolve_component(cid, VersionSelector::Default)
            .await
            .unwrap();
        assert_eq!(stale.version_number, 1, "stale served immediately");

        let mut latest = 1;
        for _ in 0..50 {
            tokio::time::sleep(Duration::from_millis(20)).await;
            latest = cache
                .resolve_component(cid, VersionSelector::Default)
                .await
                .unwrap()
                .version_number;
            if latest == 2 {
                break;
            }
        }
        assert_eq!(latest, 2, "background refresh updated the entry");
    }
}
