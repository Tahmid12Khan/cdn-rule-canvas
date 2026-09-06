//! Backend HTTP client + the `ActiveVersionRead` DTO mirror.
//!
//! The active-version payload is fetched from the RRE backend and cached in a
//! `moka::future::Cache` using STALE-WHILE-REVALIDATE: a cache hit that is older
//! than `ttl` is returned IMMEDIATELY (the request never blocks on the refetch)
//! while a background task refreshes the entry. Any error / 404 / NO_LIVE_VERSION
//! on the synchronous (MISS) path returns `None` so the forwarder fails open
//! (pass-through); a background refresh that sees a 404/err INVALIDATES the entry
//! so an unpublished version stops being served stale.

use std::sync::Arc;
use std::time::{Duration, Instant};

use moka::future::Cache;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::domain::graph::{CanvasGraph, RuleGraph};

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
    /// The single Rule Canvas.
    pub fn canvas(&self) -> &CanvasGraph {
        &self.rule_graph.canvas
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

/// Which version of a Component template a rule node references (design §2/§4.1).
/// Parsed from a rule action's `version` field: the string `"default"` → `Default`
/// (resolved per the component's movable default pointer), a positive integer → the
/// pinned `Version(n)` (with proxy-side fall-back to the current default if `n` is
/// gone). Used as the second half of the component-cache key so a `"default"` and a
/// pinned reference cache independently.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum VersionSelector {
    Default,
    Version(i32),
}

impl VersionSelector {
    /// Parse the rule action's `version` value: `"default"` (or absent/Null) →
    /// `Default`; a JSON integer or numeric string → `Version(n)`. Anything else
    /// (non-positive, non-numeric, structured) falls back to `Default` — the
    /// proxy never trusts the value blindly and `Default` is always resolvable.
    pub fn from_action_value(v: Option<&serde_json::Value>) -> Self {
        match v {
            None | Some(serde_json::Value::Null) => VersionSelector::Default,
            Some(serde_json::Value::Number(n)) => match n.as_i64() {
                Some(i) if i > 0 && i <= i64::from(i32::MAX) => VersionSelector::Version(i as i32),
                _ => VersionSelector::Default,
            },
            Some(serde_json::Value::String(s)) => {
                let s = s.trim();
                if s.eq_ignore_ascii_case("default") {
                    VersionSelector::Default
                } else {
                    match s.parse::<i32>() {
                        Ok(i) if i > 0 => VersionSelector::Version(i),
                        _ => VersionSelector::Default,
                    }
                }
            }
            _ => VersionSelector::Default,
        }
    }

    /// The `?version=` query value the resolve endpoint expects.
    pub fn as_query(&self) -> String {
        match self {
            VersionSelector::Default => "default".to_string(),
            VersionSelector::Version(n) => n.to_string(),
        }
    }
}

/// Proxy-facing resolved Component payload — EXACT shape from the backend
/// `GET /api/v1/component-templates/{cid}/resolve` endpoint (design §3.2
/// `ResolvedComponentRead`). `variables` is carried as a raw JSON value (the
/// proxy renders against the rule action's `variables`, not these declared ones,
/// so it never inspects the shape here). No `deny_unknown_fields` so extra
/// backend fields are ignored.
#[derive(Deserialize, Clone, Debug)]
pub struct ResolvedComponent {
    pub version_number: i32,
    pub html_body: String,
    #[serde(default)]
    pub variables: serde_json::Value,
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

/// One feature's list-row metadata (BACKEND CONTRACT §5 `Page<FeatureRead>`
/// item): `id`, display `name`, content `type` (`html`/`json`), and
/// `execution_order`. The hot-path forwarder uses `id` + `kind` to filter
/// features to the response content kind; the full-journey endpoint also reads
/// `name` + `execution_order`. No `deny_unknown_fields` so extra backend columns
/// are ignored. Defaults keep the fetch fail-open if an older backend omits the
/// newer fields.
#[derive(Deserialize, Clone, Debug)]
pub struct FeatureListItem {
    pub id: String,
    #[serde(default)]
    pub name: String,
    #[serde(default, rename = "type")]
    pub kind: String,
    #[serde(default)]
    pub execution_order: i32,
}

/// The `Page<FeatureListItem>` envelope used by the feature-list fetchers.
#[derive(Deserialize)]
struct FeatureListPage {
    items: Vec<FeatureListItem>,
}

/// Minimal version payload for the full-journey override path
/// (`GET /api/v1/features/{fid}/versions/{vnum}`): the subset of `VersionRead`
/// the journey needs (rule_graph + applicability + version_number). `outcomes` are
/// NOT included — the journey replays expression actions, not full outcome applies.
/// No `deny_unknown_fields` so the rest of `VersionRead` is ignored.
#[derive(Deserialize, Clone, Debug)]
pub struct VersionRead {
    pub version_number: i32,
    pub rule_graph: RuleGraph,
    #[serde(default)]
    pub applicability: Applicability,
}

/// A cached active-version entry stamped with its fetch time. The cache itself has
/// NO `time_to_live` (only a long safety cap), so freshness is decided by
/// `fetched_at.elapsed() < ttl`: within `ttl` the entry is fresh; beyond it the
/// entry is STALE and served while a background task refreshes it (SWR).
#[derive(Clone)]
struct Stamped {
    value: Arc<ActiveVersionRead>,
    fetched_at: Instant,
}

pub struct BackendClient {
    http: reqwest::Client,
    base: String,
    /// Stale-while-revalidate cache: keyed by `(feature_id, Env)`, value stamped
    /// with its fetch time. No moka TTL (entries are explicitly refreshed); a long
    /// safety cap (`ttl * 20`) bounds memory for keys that stop being requested.
    cache: Cache<(String, Env), Stamped>,
    /// Freshness window: `fetched_at.elapsed() < ttl` ⇒ fresh; else stale (SWR).
    ttl: Duration,
    /// In-flight refresh guard: a short-TTL keyless-per-key marker so concurrent
    /// stale hits spawn at most one background refresh per key (anti-thundering-herd).
    /// Duplicate refreshes are harmless regardless — this just avoids waste.
    refresh_inflight: Cache<(String, Env), ()>,
    /// TTL-cached typed feature list (single keyless entry) for the hot path:
    /// `id` + `type`/`kind` so the forwarder filters features to the response
    /// content kind. A transient error is NOT cached (re-fetch on recovery).
    feature_list: Cache<(), Arc<Vec<FeatureListItem>>>,
}

impl BackendClient {
    pub fn new(http: reqwest::Client, base: String, ttl_secs: u64) -> Self {
        let ttl = Duration::from_secs(ttl_secs);
        // The SWR cache decides FRESHNESS by the stamp (`fetched_at.elapsed() <
        // ttl`), NOT by a moka TTL. moka still gets a long SAFETY CAP so a key that
        // stops being requested is eventually evicted (memory bound) — floored at 5
        // minutes so a tiny/zero `ttl` (tests, aggressive configs) never makes moka
        // evict the entry before the next request reads + revalidates it.
        const SAFETY_FLOOR: Duration = Duration::from_secs(300);
        let safety_cap = ttl.saturating_mul(20).max(SAFETY_FLOOR);
        let cache = Cache::builder()
            .time_to_live(safety_cap)
            .max_capacity(1024)
            .build();
        // In-flight markers: floored at 10s so a tiny/zero `ttl` still collapses a
        // burst of concurrent stale hits into one refresh; the refresh task also
        // removes its own marker on completion (this is just the safety expiry).
        let inflight_ttl = ttl.max(Duration::from_secs(10));
        let refresh_inflight = Cache::builder()
            .time_to_live(inflight_ttl)
            .max_capacity(1024)
            .build();
        let feature_list = Cache::builder().time_to_live(ttl).max_capacity(1).build();
        Self {
            http,
            base,
            cache,
            ttl,
            refresh_inflight,
            feature_list,
        }
    }

    /// `GET {base}/api/v1/features?page_size=100`, TTL-cached. Returns the typed
    /// feature list (`id` + `type`/`kind`) the hot-path forwarder filters by
    /// response content kind. A successful fetch (including a genuinely empty 200)
    /// is cached for the TTL. Any transport / non-2xx / decode error yields a FRESH
    /// empty list that is NOT cached — the forwarder still passes through, but the
    /// next request re-fetches once the backend recovers instead of serving a
    /// sticky empty list for the full TTL. Never panics.
    pub async fn feature_list_cached(&self) -> Arc<Vec<FeatureListItem>> {
        if let Some(hit) = self.feature_list.get(&()).await {
            return hit;
        }
        match self.fetch_feature_list().await {
            Some(items) => {
                let items = Arc::new(items);
                self.feature_list.insert((), items.clone()).await;
                items
            }
            // Do NOT cache transient failures: return an uncached empty list so a
            // subsequent request retries the backend immediately on recovery.
            None => Arc::new(Vec::new()),
        }
    }

    /// `GET {base}/api/v1/features?page_size=100` (NOT cached). Returns the full
    /// ordered feature list (`id`, `name`, `type`, `execution_order`) for the
    /// full-journey endpoint, preserving the backend order (`type ASC,
    /// execution_order ASC`). Any transport / non-2xx / decode error yields an
    /// empty list (fail-open). Not cached: the full-journey endpoint is an
    /// off-hot-path test tool, so each call re-reads the live feature list.
    pub async fn feature_list(&self) -> Vec<FeatureListItem> {
        self.fetch_feature_list().await.unwrap_or_default()
    }

    /// `Some(items)` on a successful 2xx (cacheable). `None` on a transport /
    /// non-2xx / decode error (fail-open, but must NOT be cached).
    async fn fetch_feature_list(&self) -> Option<Vec<FeatureListItem>> {
        let url = format!(
            "{}/api/v1/features?page_size=100",
            self.base.trim_end_matches('/')
        );
        let resp = match self.http.get(&url).send().await {
            Ok(r) => r,
            Err(e) => {
                tracing::warn!(error = %e, "feature_list=miss (transport error)");
                return None;
            }
        };
        if !resp.status().is_success() {
            tracing::warn!(status = %resp.status(), "feature_list=miss (non-2xx)");
            return None;
        }
        match resp.json::<FeatureListPage>().await {
            Ok(page) => Some(page.items),
            Err(e) => {
                tracing::warn!(error = %e, "feature_list=miss (decode error)");
                None
            }
        }
    }

    /// `GET {base}/api/v1/features/{fid}/versions/{vnum}` (NOT cached). Returns the
    /// minimal [`VersionRead`] (rule_graph + applicability + version_number) for the
    /// full-journey `version_overrides` path. Returns `None` on 404 / any transport
    /// or decode error (fail-open: the caller skips the feature).
    pub async fn version(&self, feature_id: &str, version_number: i32) -> Option<VersionRead> {
        let url = format!(
            "{}/api/v1/features/{}/versions/{}",
            self.base.trim_end_matches('/'),
            feature_id,
            version_number
        );
        let resp = match self.http.get(&url).send().await {
            Ok(r) => r,
            Err(e) => {
                tracing::warn!(error = %e, feature_id, "version=miss (transport error)");
                return None;
            }
        };
        if !resp.status().is_success() {
            tracing::warn!(status = %resp.status(), feature_id, "version=miss (non-2xx)");
            return None;
        }
        match resp.json::<VersionRead>().await {
            Ok(v) => Some(v),
            Err(e) => {
                tracing::warn!(error = %e, feature_id, "version=miss (decode error)");
                None
            }
        }
    }

    /// `GET {base}/api/v1/features/{id}/active-version?env=...`, STALE-WHILE-REVALIDATE.
    ///
    /// - HIT + fresh (`fetched_at.elapsed() < ttl`) → return the cached value.
    /// - HIT + stale → return the STALE value IMMEDIATELY (never block the request)
    ///   and spawn ONE background refresh (in-flight guarded). A background refresh
    ///   that sees Ok re-stamps the entry; a 404/NO_LIVE_VERSION/error INVALIDATES
    ///   the key so an unpublished version stops being served stale (the next
    ///   request MISSes → syncs → None → fail-open).
    /// - MISS → fetch synchronously, stamp+insert on Ok, return. `None` on 404 /
    ///   NO_LIVE_VERSION / any transport or decode error (fail-open; NOT cached).
    pub async fn active_version(
        &self,
        feature_id: &str,
        env: Env,
    ) -> Option<Arc<ActiveVersionRead>> {
        let key = (feature_id.to_string(), env);

        if let Some(stamped) = self.cache.get(&key).await {
            if stamped.fetched_at.elapsed() < self.ttl {
                return Some(stamped.value); // fresh.
            }
            // Stale: serve immediately, refresh in the background (guarded).
            self.spawn_refresh(key, feature_id);
            return Some(stamped.value);
        }

        // MISS: synchronous fetch. Stamp+insert on Ok; fail-open (None, uncached)
        // on 404 / NO_LIVE_VERSION / error.
        match self.fetch_active_version(feature_id, env).await {
            FetchResult::Ok(av) => {
                self.cache
                    .insert(
                        key,
                        Stamped {
                            value: av.clone(),
                            fetched_at: Instant::now(),
                        },
                    )
                    .await;
                Some(av)
            }
            FetchResult::NotFound | FetchResult::Error => None,
        }
    }

    /// Spawn at most one background refresh per `(feature_id, env)` for a stale
    /// entry. The in-flight guard short-circuits a concurrent refresh; duplicate
    /// refreshes are harmless regardless (idempotent insert/invalidate). On Ok the
    /// entry is re-stamped; on 404/error the key is INVALIDATED. All captured state
    /// is cheap (Arc-backed `Cache` handles, cloned `reqwest::Client`, `String`).
    fn spawn_refresh(&self, key: (String, Env), feature_id: &str) {
        // All captured state is cheap (Arc-backed moka handles, cloned reqwest
        // client, owned strings) so the spawned task is `'static`.
        let inflight = self.refresh_inflight.clone();
        let cache = self.cache.clone();
        let http = self.http.clone();
        let base = self.base.clone();
        let fid = feature_id.to_string();
        let env = key.1;
        tokio::spawn(async move {
            // Anti-thundering-herd: claim the in-flight marker inside the task so
            // concurrent stale hits collapse to ONE refresh per key. Duplicate
            // refreshes are harmless regardless (idempotent insert/invalidate).
            if inflight.get(&key).await.is_some() {
                return;
            }
            inflight.insert(key.clone(), ()).await;

            let result = fetch_active_version_inner(&http, &base, &fid, env).await;
            match result {
                FetchResult::Ok(av) => {
                    cache
                        .insert(
                            key.clone(),
                            Stamped {
                                value: av,
                                fetched_at: Instant::now(),
                            },
                        )
                        .await;
                }
                // An unpublished/deleted version (404/NO_LIVE_VERSION) or a hard
                // error must STOP the stale value from being served: invalidate so
                // the next request MISSes → syncs → None → fail-open.
                FetchResult::NotFound => {
                    cache.invalidate(&key).await;
                }
                // A transient error during a background refresh leaves the stale
                // value in place (we already served it); the NEXT stale hit retries.
                FetchResult::Error => {}
            }
            inflight.invalidate(&key).await;
        });
    }

    /// Synchronous-path fetch of one active version (method form for the MISS path).
    async fn fetch_active_version(&self, feature_id: &str, env: Env) -> FetchResult {
        fetch_active_version_inner(&self.http, &self.base, feature_id, env).await
    }
}

/// Three-way fetch outcome so the SWR caller can distinguish "definitively gone"
/// (404 / NO_LIVE_VERSION → invalidate stale) from a transient error (keep stale).
enum FetchResult {
    Ok(Arc<ActiveVersionRead>),
    /// 404 / NO_LIVE_VERSION — the version is gone; invalidate any stale entry.
    NotFound,
    /// Transport / non-404 non-2xx / decode error — transient; keep any stale entry.
    Error,
}

/// Free-function fetch shared by the synchronous MISS path and the spawned
/// background refresh (so the task captures only cheap clones, not `&self`).
async fn fetch_active_version_inner(
    http: &reqwest::Client,
    base: &str,
    feature_id: &str,
    env: Env,
) -> FetchResult {
    let url = format!(
        "{}/api/v1/features/{}/active-version?env={}",
        base.trim_end_matches('/'),
        feature_id,
        env.as_query()
    );

    let resp = match http.get(&url).send().await {
        Ok(r) => r,
        Err(e) => {
            tracing::warn!(error = %e, feature_id, "active_version=miss (transport error)");
            return FetchResult::Error;
        }
    };

    let status = resp.status();
    if !status.is_success() {
        tracing::warn!(status = %status, feature_id, "active_version=miss (non-2xx)");
        // 404 == NO_LIVE_VERSION (unpublished/deleted) → definitively gone.
        return if status == reqwest::StatusCode::NOT_FOUND {
            FetchResult::NotFound
        } else {
            FetchResult::Error
        };
    }

    match resp.json::<ActiveVersionRead>().await {
        Ok(av) => FetchResult::Ok(Arc::new(av)),
        Err(e) => {
            tracing::warn!(error = %e, feature_id, "active_version=miss (decode error)");
            FetchResult::Error
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
    async fn feature_list_error_response_is_not_cached() {
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

        let first = client.feature_list_cached().await;
        assert!(first.is_empty(), "error fails open to an empty list");

        // Backend recovers (200 with one feature). Mounted after the 500 mock is
        // exhausted, so the next fetch sees it.
        Mock::given(method("GET"))
            .and(path("/api/v1/features"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "items": [{ "id": "demo-article", "name": "Demo", "type": "html", "execution_order": 1 }]
            })))
            .expect(1)
            .mount(&server)
            .await;

        let second = client.feature_list_cached().await;
        let second_ids: Vec<&str> = second.iter().map(|f| f.id.as_str()).collect();
        assert_eq!(
            second_ids,
            ["demo-article"],
            "the error result was not cached, so the recovered data is served immediately"
        );

        // Successful result IS cached: a third call within the TTL serves from cache
        // (the 200 mock's `expect(1)` would fail on a second backend hit).
        let third = client.feature_list_cached().await;
        let third_ids: Vec<&str> = third.iter().map(|f| f.id.as_str()).collect();
        assert_eq!(third_ids, ["demo-article"]);
    }

    /// SWR: within `ttl` the cache is fresh → exactly ONE backend hit for repeated
    /// `active_version` calls (no synchronous refetch, no background refresh).
    #[tokio::test]
    async fn active_version_within_ttl_hits_backend_once() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/v1/features/f1/active-version"))
            .respond_with(ResponseTemplate::new(200).set_body_json(av_body(1)))
            .expect(1) // exactly one fetch for the whole test window.
            .mount(&server)
            .await;

        // Long ttl so every call after the first is FRESH.
        let client = BackendClient::new(reqwest::Client::new(), server.uri(), 3600);

        let a = client.active_version("f1", Env::Live).await.unwrap();
        assert_eq!(a.version_number, 1);
        for _ in 0..5 {
            let v = client.active_version("f1", Env::Live).await.unwrap();
            assert_eq!(v.version_number, 1, "served fresh from cache");
        }
    }

    /// SWR: after `ttl` a stale hit returns the STALE value WITHOUT a synchronous
    /// backend call, and a background refresh updates the entry to the new value.
    #[tokio::test]
    async fn active_version_stale_serves_immediately_then_background_refreshes() {
        let server = MockServer::start().await;
        // First fetch (the synchronous MISS) -> v1, single-use.
        Mock::given(method("GET"))
            .and(path("/api/v1/features/f1/active-version"))
            .respond_with(ResponseTemplate::new(200).set_body_json(av_body(1)))
            .up_to_n_times(1)
            .expect(1)
            .mount(&server)
            .await;

        // ttl = 0 so the entry is stale on the very next call (elapsed >= 0).
        let client = BackendClient::new(reqwest::Client::new(), server.uri(), 0);

        let first = client.active_version("f1", Env::Live).await.unwrap();
        assert_eq!(first.version_number, 1, "MISS fetches v1");

        // Backend now serves v2; mounted AFTER the v1 mock is exhausted.
        Mock::given(method("GET"))
            .and(path("/api/v1/features/f1/active-version"))
            .respond_with(ResponseTemplate::new(200).set_body_json(av_body(2)))
            .mount(&server)
            .await;

        // The next call is a STALE hit: it returns v1 IMMEDIATELY (no sync refetch)
        // and spawns a background refresh.
        let stale = client.active_version("f1", Env::Live).await.unwrap();
        assert_eq!(stale.version_number, 1, "stale value served immediately");

        // The background refresh eventually re-stamps the entry to v2. Poll briefly.
        let mut latest = 1;
        for _ in 0..50 {
            tokio::time::sleep(Duration::from_millis(20)).await;
            latest = client
                .active_version("f1", Env::Live)
                .await
                .unwrap()
                .version_number;
            if latest == 2 {
                break;
            }
        }
        assert_eq!(latest, 2, "background refresh updated the entry to v2");
    }

    /// SWR: a background refresh that sees a 404 (NO_LIVE_VERSION — unpublished)
    /// INVALIDATES the entry, so the next request MISSes → syncs → None (fail-open),
    /// no longer serving the stale value.
    #[tokio::test]
    async fn active_version_background_404_invalidates_entry() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/v1/features/f1/active-version"))
            .respond_with(ResponseTemplate::new(200).set_body_json(av_body(1)))
            .up_to_n_times(1)
            .expect(1)
            .mount(&server)
            .await;

        let client = BackendClient::new(reqwest::Client::new(), server.uri(), 0);

        let first = client.active_version("f1", Env::Live).await.unwrap();
        assert_eq!(first.version_number, 1);

        // Backend now 404s (version unpublished). Stale hit serves v1 + spawns the
        // refresh, which sees the 404 and invalidates the key.
        Mock::given(method("GET"))
            .and(path("/api/v1/features/f1/active-version"))
            .respond_with(ResponseTemplate::new(404).set_body_json(serde_json::json!({
                "error": { "code": "NO_LIVE_VERSION", "message": "none" }
            })))
            .mount(&server)
            .await;

        let stale = client.active_version("f1", Env::Live).await;
        assert_eq!(
            stale.unwrap().version_number,
            1,
            "stale v1 served while the refresh runs"
        );

        // Once the background refresh invalidates the key, subsequent calls MISS →
        // sync fetch → 404 → None (fail-open). Poll until the invalidation lands.
        let mut gone = false;
        for _ in 0..50 {
            tokio::time::sleep(Duration::from_millis(20)).await;
            if client.active_version("f1", Env::Live).await.is_none() {
                gone = true;
                break;
            }
        }
        assert!(gone, "404 background refresh invalidated the stale entry");
    }

    /// A minimal active-version body with the given version_number and an empty
    /// rule_graph (enough to deserialize `ActiveVersionRead`).
    fn av_body(version_number: i32) -> serde_json::Value {
        serde_json::json!({
            "version_number": version_number,
            "rule_graph": {
                "canvas": { "root_node_id": null, "nodes": [], "edges": [] }
            },
            "applicability": {},
            "outcomes": []
        })
    }
}
