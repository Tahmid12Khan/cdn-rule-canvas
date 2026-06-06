//! Backend HTTP client + the `ActiveVersionRead` DTO mirror.
//!
//! The active-version payload is fetched from the RRE backend and cached in a
//! `moka::future::Cache` (TTL configured at startup). Any error / 404 /
//! NO_LIVE_VERSION returns `None` so the forwarder fails open (pass-through).

use std::sync::Arc;
use std::time::Duration;

use moka::future::Cache;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::domain::graph::{Canvas, CanvasGraph, RuleGraph};

/// Placement mirror (BACKEND CONTRACT §2). snake_case wire form.
#[derive(Deserialize, Serialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Placement {
    Inline,
    StickyFooter,
    Popup,
}

/// Version-level applicability gate (BACKEND CONTRACT §5). Mirrors the backend
/// `Applicability` DTO. serde defaults, no `deny_unknown_fields` — an older
/// backend that omits the field deserializes to the empty (apply-always) gate.
#[derive(Deserialize, Clone, Debug, Default)]
pub struct Applicability {
    /// CSS selector that must match >=1 element for an HTML response's rules to
    /// apply. None/empty = apply whenever the response content-type is HTML.
    #[serde(default)]
    pub html_selector: Option<String>,
    /// JSONPath that must match >=1 node for a JSON response's rules to apply.
    /// None/empty = apply whenever the response content-type is JSON.
    #[serde(default)]
    pub json_selector: Option<String>,
}

/// Active-version payload — EXACT shape from BACKEND CONTRACT §5.
#[derive(Deserialize, Clone, Debug)]
pub struct ActiveVersionRead {
    pub version_number: i32,
    pub rule_graph: RuleGraph,
    #[serde(default)]
    pub applicability: Applicability,
    pub outcomes: Vec<ActiveOutcome>,
}

#[derive(Deserialize, Clone, Debug)]
pub struct ActiveOutcome {
    pub id: Uuid,
    pub title: String,
    pub is_builtin: bool,
    pub order_index: i32,
    pub components: Vec<ActiveComponent>,
}

#[derive(Deserialize, Clone, Debug)]
pub struct ActiveComponent {
    pub id: Uuid,
    pub slug: String,
    pub r#type: String,
    pub config: serde_json::Value,
    pub placement: Placement,
    pub order_index: i32,
}

impl ActiveVersionRead {
    /// The `CanvasGraph` for the classified user class (canvas isolation).
    pub fn canvas(&self, c: Canvas) -> &CanvasGraph {
        self.rule_graph.canvas(c)
    }

    /// Find an outcome by id (the eval result `outcomeId`).
    pub fn find_outcome(&self, id: Uuid) -> Option<&ActiveOutcome> {
        self.outcomes.iter().find(|o| o.id == id)
    }
}

impl ActiveOutcome {
    /// True if this is the builtin "ShowContent" outcome (a no-op terminal).
    pub fn is_builtin_show_content(&self) -> bool {
        self.is_builtin
    }
}

/// Environment selector for the active-version fetch.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Env {
    Live,
    Staging,
}

impl Env {
    fn as_query(&self) -> &'static str {
        match self {
            Env::Live => "live",
            Env::Staging => "staging",
        }
    }
}

/// The `Page<FeatureRead>` envelope (BACKEND CONTRACT §5). The proxy only needs
/// each feature's `id`; the rest of the DTO is ignored.
#[derive(Deserialize)]
struct FeaturePage {
    items: Vec<FeatureRef>,
}

#[derive(Deserialize)]
struct FeatureRef {
    id: String,
}

pub struct BackendClient {
    http: reqwest::Client,
    base: String,
    cache: Cache<(String, Env), Arc<ActiveVersionRead>>,
    /// TTL-cached list of ALL feature ids (single keyless entry). Mirrors the
    /// active-version cache TTL. Every request runs the per-feature pipeline for
    /// each id; an active-version 404 skips that feature (as today).
    feature_ids: Cache<(), Arc<Vec<String>>>,
}

impl BackendClient {
    pub fn new(http: reqwest::Client, base: String, ttl_secs: u64) -> Self {
        let cache = Cache::builder()
            .time_to_live(Duration::from_secs(ttl_secs))
            .max_capacity(1024)
            .build();
        let feature_ids = Cache::builder()
            .time_to_live(Duration::from_secs(ttl_secs))
            .max_capacity(1)
            .build();
        Self {
            http,
            base,
            cache,
            feature_ids,
        }
    }

    /// `GET {base}/api/v1/features?page_size=100`, TTL-cached. Returns ALL feature
    /// ids. A successful fetch (including a genuinely empty 200) is cached for the
    /// TTL. Any transport / non-2xx / decode error yields a FRESH empty list that
    /// is NOT cached — the forwarder still passes through, but the next request
    /// re-fetches once the backend recovers instead of serving a sticky empty list
    /// for the full TTL. Never panics.
    pub async fn feature_ids(&self) -> Arc<Vec<String>> {
        if let Some(hit) = self.feature_ids.get(&()).await {
            return hit;
        }
        match self.fetch_feature_ids().await {
            Ok(ids) => {
                let ids = Arc::new(ids);
                self.feature_ids.insert((), ids.clone()).await;
                ids
            }
            // Do NOT cache transient failures: return an uncached empty list so a
            // subsequent request retries the backend immediately on recovery.
            Err(()) => Arc::new(Vec::new()),
        }
    }

    /// `Ok(ids)` on a successful 2xx (cacheable). `Err(())` on a transport /
    /// non-2xx / decode error (fail-open, but must NOT be cached).
    async fn fetch_feature_ids(&self) -> Result<Vec<String>, ()> {
        let url = format!(
            "{}/api/v1/features?page_size=100",
            self.base.trim_end_matches('/')
        );
        let resp = match self.http.get(&url).send().await {
            Ok(r) => r,
            Err(e) => {
                tracing::warn!(error = %e, "feature_list=miss (transport error)");
                return Err(());
            }
        };
        if !resp.status().is_success() {
            tracing::warn!(status = %resp.status(), "feature_list=miss (non-2xx)");
            return Err(());
        }
        match resp.json::<FeaturePage>().await {
            Ok(page) => Ok(page.items.into_iter().map(|f| f.id).collect()),
            Err(e) => {
                tracing::warn!(error = %e, "feature_list=miss (decode error)");
                Err(())
            }
        }
    }

    /// `GET {base}/api/v1/features/{id}/active-version?env=...`, TTL-cached.
    /// Returns `None` on 404 / NO_LIVE_VERSION / any transport or decode error
    /// (fail-open per the proxy contract).
    pub async fn active_version(
        &self,
        feature_id: &str,
        env: Env,
    ) -> Option<Arc<ActiveVersionRead>> {
        let key = (feature_id.to_string(), env);
        if let Some(hit) = self.cache.get(&key).await {
            return Some(hit);
        }

        let url = format!(
            "{}/api/v1/features/{}/active-version?env={}",
            self.base.trim_end_matches('/'),
            feature_id,
            env.as_query()
        );

        let resp = match self.http.get(&url).send().await {
            Ok(r) => r,
            Err(e) => {
                tracing::warn!(error = %e, feature_id, "active_version=miss (transport error)");
                return None;
            }
        };

        if !resp.status().is_success() {
            tracing::warn!(status = %resp.status(), feature_id, "active_version=miss (non-2xx)");
            return None;
        }

        match resp.json::<ActiveVersionRead>().await {
            Ok(av) => {
                let arc = Arc::new(av);
                self.cache.insert(key, arc.clone()).await;
                Some(arc)
            }
            Err(e) => {
                tracing::warn!(error = %e, feature_id, "active_version=miss (decode error)");
                None
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    /// A transient backend error on the feature-list fetch must NOT be cached: the
    /// first request hits a 500 (fail-open => empty list, uncached), and the very
    /// next request — still within the TTL — re-fetches and sees the now-healthy
    /// backend's data, without waiting for the TTL to expire.
    #[tokio::test]
    async fn feature_ids_error_response_is_not_cached() {
        let server = MockServer::start().await;

        // First call: backend is down (500). `expect(1)` pins it to one hit.
        Mock::given(method("GET"))
            .and(path("/api/v1/features"))
            .respond_with(ResponseTemplate::new(500))
            .up_to_n_times(1)
            .expect(1)
            .mount(&server)
            .await;

        // A long TTL proves recovery is NOT gated on TTL expiry: a cached empty
        // result would prevent the second call from ever reaching the backend.
        let client = BackendClient::new(reqwest::Client::new(), server.uri(), 3600);

        let first = client.feature_ids().await;
        assert!(first.is_empty(), "error fails open to an empty list");

        // Backend recovers (200 with one feature). Mounted after the 500 mock is
        // exhausted, so the next fetch sees it.
        Mock::given(method("GET"))
            .and(path("/api/v1/features"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "items": [{ "id": "demo-article" }]
            })))
            .expect(1)
            .mount(&server)
            .await;

        let second = client.feature_ids().await;
        assert_eq!(
            second.as_slice(),
            ["demo-article".to_string()],
            "the error result was not cached, so the recovered data is served immediately"
        );

        // Successful result IS cached: a third call within the TTL serves from cache
        // (the 200 mock's `expect(1)` would fail on a second backend hit).
        let third = client.feature_ids().await;
        assert_eq!(third.as_slice(), ["demo-article".to_string()]);
    }
}
