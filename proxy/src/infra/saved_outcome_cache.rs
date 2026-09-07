//! Saved-outcome resolve cache (Outcomes Library design). Mirrors
//! `component_cache.rs`'s stale-while-revalidate shape exactly, keyed by the
//! saved outcome's own id (no version selector — the backend `resolve` route
//! already picked the version and rendered the HTML).

use std::sync::Arc;
use std::time::{Duration, Instant};

use moka::future::Cache;
use uuid::Uuid;

pub use rre_core::bundle::ResolvedSavedOutcome;

#[derive(Clone)]
struct Stamped {
    value: Arc<ResolvedSavedOutcome>,
    fetched_at: Instant,
}

enum FetchResult {
    Ok(Arc<ResolvedSavedOutcome>),
    NotFound,
    Error,
}

pub struct SavedOutcomeCache {
    http: reqwest::Client,
    base: String,
    cache: Cache<Uuid, Stamped>,
    ttl: Duration,
    refresh_inflight: Cache<Uuid, ()>,
}

impl SavedOutcomeCache {
    pub fn new(http: reqwest::Client, base: String, ttl_secs: u64) -> Self {
        let ttl = Duration::from_secs(ttl_secs);
        const SAFETY_FLOOR: Duration = Duration::from_secs(300);
        let safety_cap = ttl.saturating_mul(20).max(SAFETY_FLOOR);
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

    pub async fn resolve_saved_outcome(&self, id: Uuid) -> Option<Arc<ResolvedSavedOutcome>> {
        if let Some(stamped) = self.cache.get(&id).await {
            if stamped.fetched_at.elapsed() < self.ttl {
                return Some(stamped.value);
            }
            self.spawn_refresh(id);
            return Some(stamped.value);
        }
        match fetch_inner(&self.http, &self.base, id).await {
            FetchResult::Ok(rso) => {
                self.cache
                    .insert(
                        id,
                        Stamped {
                            value: rso.clone(),
                            fetched_at: Instant::now(),
                        },
                    )
                    .await;
                Some(rso)
            }
            FetchResult::NotFound | FetchResult::Error => None,
        }
    }

    fn spawn_refresh(&self, id: Uuid) {
        let inflight = self.refresh_inflight.clone();
        let cache = self.cache.clone();
        let http = self.http.clone();
        let base = self.base.clone();
        tokio::spawn(async move {
            if inflight.get(&id).await.is_some() {
                return;
            }
            inflight.insert(id, ()).await;
            match fetch_inner(&http, &base, id).await {
                FetchResult::Ok(rso) => {
                    cache
                        .insert(
                            id,
                            Stamped {
                                value: rso,
                                fetched_at: Instant::now(),
                            },
                        )
                        .await;
                }
                FetchResult::NotFound => {
                    cache.invalidate(&id).await;
                }
                FetchResult::Error => {}
            }
            inflight.invalidate(&id).await;
        });
    }
}

async fn fetch_inner(http: &reqwest::Client, base: &str, id: Uuid) -> FetchResult {
    let url = format!(
        "{}/api/v1/saved-outcomes/{}/resolve",
        base.trim_end_matches('/'),
        id
    );
    let resp = match http.get(&url).send().await {
        Ok(r) => r,
        Err(e) => {
            tracing::warn!(error = %e, saved_outcome_id = %id, "resolve_saved_outcome=miss (transport error)");
            return FetchResult::Error;
        }
    };
    let status = resp.status();
    if !status.is_success() {
        tracing::warn!(status = %status, saved_outcome_id = %id, "resolve_saved_outcome=miss (non-2xx)");
        return if status == reqwest::StatusCode::NOT_FOUND {
            FetchResult::NotFound
        } else {
            FetchResult::Error
        };
    }
    match resp.json::<ResolvedSavedOutcome>().await {
        Ok(rso) => FetchResult::Ok(Arc::new(rso)),
        Err(e) => {
            tracing::warn!(error = %e, saved_outcome_id = %id, "resolve_saved_outcome=miss (decode error)");
            FetchResult::Error
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[tokio::test]
    async fn resolve_within_ttl_hits_backend_once() {
        let server = MockServer::start().await;
        let id = Uuid::new_v4();
        Mock::given(method("GET"))
            .and(path(format!("/api/v1/saved-outcomes/{id}/resolve")))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_json(serde_json::json!({ "html_body": "<p>hi</p>" })),
            )
            .expect(1)
            .mount(&server)
            .await;
        let cache = SavedOutcomeCache::new(reqwest::Client::new(), server.uri(), 3600);
        for _ in 0..5 {
            let rso = cache.resolve_saved_outcome(id).await.unwrap();
            assert_eq!(rso.html_body, "<p>hi</p>");
        }
    }

    #[tokio::test]
    async fn unresolvable_returns_none() {
        let server = MockServer::start().await;
        let id = Uuid::new_v4();
        Mock::given(method("GET"))
            .and(path(format!("/api/v1/saved-outcomes/{id}/resolve")))
            .respond_with(ResponseTemplate::new(404))
            .mount(&server)
            .await;
        let cache = SavedOutcomeCache::new(reqwest::Client::new(), server.uri(), 3600);
        assert!(cache.resolve_saved_outcome(id).await.is_none());
    }
}
